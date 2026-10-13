"""Initialize or verify pgBackRest backups after provision.py.

Run from deploy/: pyinfra inventory.py pgbackrest.py
After a disaster, restore and verify PostgreSQL before running this file.
"""

import json

from object_storage import backup_configuration
from pyinfra import host
from pyinfra.facts.server import Command
from pyinfra.operations import files, server, systemd


def initial_backup_required(info):
    """Only bootstrap a healthy stanza with no backups; fail closed on errors."""
    stanzas = json.loads(info)
    if not isinstance(stanzas, list) or len(stanzas) != 1:
        raise ValueError("expected pgBackRest info for exactly one lfsplanet stanza")
    stanza = stanzas[0]
    if (
        stanza.get("name") != "lfsplanet"
        or stanza.get("status", {}).get("code") not in (0, 2)
        or not isinstance(stanza.get("backup"), list)
    ):
        raise ValueError("pgBackRest repository is not healthy; refusing bootstrap backup")
    if stanza["backup"]:
        return False
    if stanza["status"]["code"] != 2:
        raise ValueError("unexpected empty pgBackRest backup list; refusing bootstrap backup")
    return True


backup = backup_configuration(host.data)
if backup is None:
    raise ValueError("pgbackrest.py requires all five LFSPLANET_BACKUP_* settings")

# This verifies the database system identifier against an existing stanza.
# A freshly initialized replacement cluster must fail here, before enabling
# archiving, taking any backup, or enabling timers. Restore it explicitly.
server.shell(
    name="Create or verify the lfsplanet pgBackRest stanza",
    commands=["pgbackrest --stanza=lfsplanet stanza-create"],
    _sudo_user="postgres",
)

files.template(
    name="Configure PostgreSQL WAL archiving",
    src="templates/postgresql-pgbackrest.conf.j2",
    dest="/etc/postgresql/18/main/conf.d/pgbackrest.conf",
    user="postgres",
    group="postgres",
    mode="0640",
)

# Reload first so pending_restart reflects the new settings. Only the
# initial enablement (or another restart-only change) needs downtime.
server.shell(
    name="Apply PostgreSQL archiving settings, restarting only when required",
    commands=[
        """
        set -eu
        pg_ctlcluster 18 main reload
        pending_restart="$(psql -p 5432 -d postgres -Atqc "SELECT count(*) FROM pg_settings WHERE pending_restart AND name IN ('archive_mode', 'wal_level')")"
        archive_mode="$(psql -p 5432 -d postgres -Atqc 'SHOW archive_mode')"
        if [ "$pending_restart" != "0" ] || [ "$archive_mode" != "on" ]; then
            pg_ctlcluster 18 main restart
        fi
        """
    ],
    _sudo_user="postgres",
)

server.shell(
    name="Verify pgBackRest repository and WAL archiving",
    commands=["pgbackrest --stanza=lfsplanet check"],
    _sudo_user="postgres",
)

# Query repository state at execution time, after stanza-create/check.
# No local marker: a new host or changed bucket must not look like a reset.
# Initialization never runs expiration; normal scheduled backups manage it.
server.shell(
    name="Take the initial full PostgreSQL backup if the stanza has none",
    commands=["pgbackrest --stanza=lfsplanet --type=full --no-expire-auto backup"],
    _sudo_user="postgres",
    _if=lambda: initial_backup_required(
        host.get_fact(
            Command,
            command="pgbackrest --stanza=lfsplanet --output=json info",
            _sudo=True,
            _sudo_user="postgres",
        )
    ),
)

files.template(
    name="Install the pgBackRest backup service",
    src="templates/pgbackrest-backup@.service.j2",
    dest="/etc/systemd/system/pgbackrest-backup@.service",
    user="root",
    group="root",
    mode="0644",
)

for backup_type, calendar in (
    ("full", "Sun *-*-* 02:00:00"),
    ("diff", "Mon..Sat *-*-* 02:00:00"),
):
    files.template(
        name=f"Install the pgBackRest {backup_type} backup timer",
        src="templates/pgbackrest-backup.timer.j2",
        dest=f"/etc/systemd/system/pgbackrest-backup-{backup_type}.timer",
        user="root",
        group="root",
        mode="0644",
        backup_type=backup_type,
        calendar=calendar,
    )

systemd.daemon_reload(name="Reload systemd after pgBackRest unit changes")
for backup_type in ("full", "diff"):
    systemd.service(
        name=f"Enable pgBackRest {backup_type} backup scheduling",
        service=f"pgbackrest-backup-{backup_type}.timer",
        running=True,
        enabled=True,
    )
