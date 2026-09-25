"""Deploy an lfsplanet release to a provisioned host.

Run from this directory, for example:
    pyinfra inventory.py deploy.py

The inventory must supply deploy-directory-relative release_binary and
frontend_dist paths, a site_domain, and a session_key. OAuth credentials are
optional, but must be supplied as a pair.

The binary and the frontend are separate artefacts deployed in one pass. Caddy
serves the frontend from a content-addressed release directory and proxies the
API, so a frontend change no longer needs the binary rebuilt and swapped.
"""

import hashlib
from pathlib import Path

from pyinfra import host
from pyinfra.operations import files, server, systemd, postgres


DEPLOY_DIR = Path(__file__).resolve().parent


def required(name):
    value = host.data.get(name)
    if not value:
        raise ValueError(f"inventory data must set {name!r}")
    return value


def release_id(dist):
    """Names a frontend release after the bytes it contains.

    Content addressing makes the upload idempotent and keeps published releases
    immutable, so a browser still running a previous index.html keeps finding
    the hashed chunks that document references.
    """
    digest = hashlib.sha256()
    for path in sorted(p for p in dist.rglob("*") if p.is_file()):
        digest.update(str(path.relative_to(dist)).encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()[:16]


app_user = host.data.app_user
app_group = host.data.app_group
app_root = host.data.app_root
config_dir = host.data.config_dir
state_dir = host.data.state_dir
release_binary = required("release_binary")
site_domain = required("site_domain")
alternate_domains = host.data.get("alternate_domains", [])
session_key = required("session_key")
oauth_client_id = host.data.get("oauth_client_id")
oauth_client_secret = host.data.get("oauth_client_secret")
if not isinstance(alternate_domains, (list, tuple)) or not all(
    isinstance(domain, str) and domain for domain in alternate_domains
):
    raise ValueError("inventory data 'alternate_domains' must be a list of non-empty domains")
if site_domain in alternate_domains:
    raise ValueError("inventory data 'alternate_domains' must not include 'site_domain'")
if bool(oauth_client_id) != bool(oauth_client_secret):
    raise ValueError(
        "inventory data must set both 'oauth_client_id' and 'oauth_client_secret', or neither"
    )

frontend_dir = host.data.frontend_dir
frontend_dist = (DEPLOY_DIR / required("frontend_dist")).resolve()
if not (frontend_dist / "index.html").is_file():
    raise ValueError(
        f"inventory data 'frontend_dist' ({frontend_dist}) holds no index.html; "
        "build the frontend with `npm ci && npm run build` in frontend2 first"
    )
frontend_current = f"{frontend_dir}/current"
frontend_release = f"{frontend_dir}/releases/{release_id(frontend_dist)}"

postgres.role(
    name="Ensure the lfsplanet PostgreSQL role exists",
    role="lfsplanet",
    login=True,
    inherit=True,
    psql_database="postgres",
    _sudo_user="postgres",
)

postgres.database(
    name="Ensure the lfsplanet PostgreSQL database exists",
    database="lfsplanet",
    owner="lfsplanet",
    psql_database="postgres",
    _sudo_user="postgres",
)

files.template(
    name="Render the lfsplanet configuration",
    src="templates/config.yaml.j2",
    dest=f"{config_dir}/config.yaml",
    user="root",
    group=app_group,
    mode="0640",
    session_key=session_key,
    site_domain=site_domain,
    state_dir=state_dir,
    max_spr_upload_bytes=host.data.max_spr_upload_bytes,
    webhook_workers=host.data.webhook_workers,
    oauth_client_id=oauth_client_id,
    oauth_client_secret=oauth_client_secret,
)

for unit in (
    "lfsplanet-web.service",
    "lfsplanet-validator.service",
    "lfsplanet-maintenance.service",
    "lfsplanet-maintenance.timer",
):
    files.template(
        name=f"Install {unit}",
        src=f"templates/{unit}.j2",
        dest=f"/etc/systemd/system/{unit}",
        user="root",
        group="root",
        mode="0644",
    )

files.template(
    name="Install the lfsplanet Caddy site",
    src="templates/Caddyfile.j2",
    dest="/etc/caddy/Caddyfile",
    user="root",
    group="root",
    mode="0644",
    site_domain=site_domain,
    alternate_domains=alternate_domains,
    maintenance_dir=f"{app_root}/maintenance",
    frontend_dir=frontend_dir,
    content_security_policy=host.data.content_security_policy,
)

files.template(
    name="Install the lfsplanet maintenance page",
    src="templates/maintenance.html.j2",
    dest=f"{app_root}/maintenance/maintenance.html",
    user="root",
    group="root",
    mode="0644",
)

# Reject a bad site configuration while the running site is still untouched,
# rather than discovering it after the application has been stopped.
server.shell(
    name="Validate the rendered Caddy configuration",
    commands=["caddy validate --adapter caddyfile --config /etc/caddy/Caddyfile"],
)

# Upload into the release directory before anything goes down. Nothing serves it
# until `current` moves, so this is invisible to visitors and safe to repeat.
files.sync(
    name="Upload the frontend release",
    src=str(frontend_dist),
    dest=frontend_release,
    user="root",
    group="root",
    mode="0644",
    dir_mode="0755",
)

systemd.daemon_reload(name="Reload systemd after lfsplanet unit changes")

# The maintenance page covers the whole site, static frontend included: the
# frontend cannot do anything useful while the API is migrating. Enabling it
# before the frontend symlink moves also means the new frontend is never served
# against the old API.
files.file(
    name="Enable the lfsplanet maintenance page",
    path=f"{app_root}/maintenance/enabled",
    user="root",
    group="root",
    mode="0644",
    touch=True,
)

systemd.service(
    name="Enable and restart Caddy with maintenance routing",
    service="caddy.service",
    running=True,
    restarted=True,
    enabled=True,
)

# rename(2) over the symlink, so no request can observe a missing document root.
server.shell(
    name="Publish the frontend release",
    commands=[
        f"""
        if [ "$(readlink {frontend_current} 2>/dev/null)" != "{frontend_release}" ]; then
            ln -sfn {frontend_release} {frontend_current}.new
            mv -Tf {frontend_current}.new {frontend_current}
        fi
        """
    ],
)

for service in (
    "lfsplanet-maintenance.timer",
    "lfsplanet-maintenance.service",
    "lfsplanet-validator.service",
    "lfsplanet-web.service",
):
    systemd.service(
        name=f"Stop {service} before migrating the database",
        service=service,
        running=False,
    )

files.put(
    name="Replace the lfsplanet binary",
    src=release_binary,
    dest=f"{app_root}/lfsplanet",
    user="root",
    group="root",
    mode="0755",
)

server.shell(
    name="Apply lfsplanet database migrations",
    commands=[
        f"{app_root}/lfsplanet --config {config_dir}/config.yaml migrate"
    ],
    _sudo_user=app_user,
)

systemd.service(
    name="Enable and restart the lfsplanet web service",
    service="lfsplanet-web.service",
    running=True,
    restarted=True,
    enabled=True,
)

systemd.service(
    name="Enable and restart the lfsplanet validator",
    service="lfsplanet-validator.service",
    running=True,
    restarted=True,
    enabled=True,
)

systemd.service(
    name="Enable the lfsplanet maintenance timer",
    service="lfsplanet-maintenance.timer",
    running=True,
    enabled=True,
)

files.file(
    name="Disable the lfsplanet maintenance page",
    path=f"{app_root}/maintenance/enabled",
    present=False,
)

# Only once the new release is serving. Nothing is retained: a chunk request
# against a removed release answers 404, and SvelteKit responds to that by
# re-checking `_app/version.json`, seeing a new version and reloading onto the
# new release. Keeping old releases would instead let a stale frontend keep
# running against an API that has already migrated.
server.shell(
    name="Remove superseded frontend releases",
    commands=[
        f"""
        cd {frontend_dir}/releases || exit 0
        live="$(readlink -f {frontend_current} 2>/dev/null || true)"
        for old in */; do
            old="${{old%/}}"
            [ -d "$old" ] || continue
            [ "$(readlink -f "$old")" = "$live" ] || rm -rf -- "$old"
        done
        """
    ],
)
