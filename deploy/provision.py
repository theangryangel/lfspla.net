"""Provision the long-lived lfsplanet host state.

Run before the first deploy and whenever host dependencies change:
    pyinfra inventory.py provision.py
"""

from object_storage import backup_configuration

from pyinfra import host
from pyinfra.operations import apt, files, server, systemd


app_user = host.data.app_user
app_group = host.data.app_group
app_root = host.data.app_root
config_dir = host.data.config_dir
state_dir = host.data.state_dir
maintenance_dir = f"{app_root}/maintenance"
frontend_dir = host.data.frontend_dir
backup = backup_configuration(host.data)

apt.update(name="Update APT package index before provisioning dependencies")

apt.packages(
    name="Install PostgreSQL repository prerequisites",
    packages=["ca-certificates", "postgresql-common"],
)

server.shell(
    name="Configure the PostgreSQL Global Development Group APT repository",
    commands=["YES=yes /usr/share/postgresql-common/pgdg/apt.postgresql.org.sh"],
)

apt.packages(
    name="Install lfsplanet host dependencies",
    packages=["bubblewrap", "caddy", "postgresql-18", "pgbackrest", "wine"],
    update=True,
)

systemd.service(
    name="Enable and start PostgreSQL",
    service="postgresql.service",
    running=True,
    enabled=True,
)

server.group(
    name="Create the lfsplanet service group",
    group=app_group,
    system=True,
)

server.user(
    name="Create the lfsplanet service account",
    user=app_user,
    group=app_group,
    system=True,
    shell="/usr/sbin/nologin",
    ensure_home=False,
)

for path, owner, mode in (
    (app_root, "root", "0755"),
    (maintenance_dir, "root", "0755"),
    (frontend_dir, "root", "0755"),
    (f"{frontend_dir}/releases", "root", "0755"),
    (config_dir, "root", "0750"),
    (f"{config_dir}/eras", "root", "0750"),
    (state_dir, app_user, "0750"),
    (f"{state_dir}/games", app_user, "0750"),
    (f"{state_dir}/wine", app_user, "0750"),
    (f"{state_dir}/spr", app_user, "0750"),
):
    files.directory(
        name=f"Ensure {path} exists",
        path=path,
        user=owner,
        group=app_group,
        mode=mode,
    )

# Prepare the client for backups or a manual restore. Do not contact the
# repository, initialize a stanza, change archiving, or enable backup timers.
if backup:
    for path, owner in (
        ("/etc/pgbackrest", "root"),
        ("/var/lib/pgbackrest", "postgres"),
    ):
        files.directory(
            name=f"Ensure {path} exists",
            path=path,
            user=owner,
            group="postgres",
            mode="0750",
        )
    files.template(
        name="Configure encrypted pgBackRest S3 repository",
        src="templates/pgbackrest.conf.j2",
        dest="/etc/pgbackrest/pgbackrest.conf",
        user="root",
        group="postgres",
        mode="0640",
        backup=backup,
    )
