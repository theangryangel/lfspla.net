# Deployment

This directory deploys lfsplanet with pyinfra, systemd, Caddy, and PostgreSQL.
A release is two artefacts deployed in one pass: the API binary, which runs
under systemd on loopback, and the built frontend, which Caddy serves from
disk. Caddy proxies `/api`, `/auth` and `/health` to the binary and answers
everything else from the frontend release.

It assumes a Debian-family host with systemd. Run `provision.py` once to
configure the PostgreSQL Global Development Group (PGDG) APT repository and
install PostgreSQL 18, Wine, Bubblewrap, and other durable host state. Run `deploy.py` for each application release.

`provision.py` sets `YES=yes` for the PGDG repository setup script so it can
run non-interactively.

Provisioning enables PostgreSQL and ensures the `lfsplanet` login role and its
owned `lfsplanet` database exist. The rendered configuration connects over
the local Unix socket using Debian's default peer authentication.

It installs these persistent paths:

```text
/etc/lfsplanet/config.yaml    rendered application configuration (contains secrets)
/etc/lfsplanet/eras/          deployed version-controlled era inputs
/etc/lfsplanet/builtin-vehicles/  deployed built-in vehicle image inputs
/var/lib/lfsplanet/games/     named LFS installations
/var/lib/lfsplanet/wine/      shared Wine prefix
/var/lib/lfsplanet/spr/       local replay and cached vehicle-image storage
/opt/lfsplanet/frontend/      frontend releases and the `current` symlink
```

## Secrets

No production secrets belong in this repository. The checked-in
`inventory.py` reads the target host and secrets from the environment. The
deployment renders `/etc/lfsplanet/config.yaml` from a Jinja template as
`root:lfsplanet`, mode `0640`.

Generate a fresh session key and obtain OAuth values from your secret manager.
The inventory requires `LFSPLANET_HOST` and `LFSPLANET_SESSION_KEY`; when
OAuth is enabled, it also reads `LFSPLANET_OAUTH_CLIENT_ID` and
`LFSPLANET_OAUTH_CLIENT_SECRET`. `site_domain` is the canonical public domain;
set `alternate_domains` in the inventory to domains that should permanently
redirect to it (for example, `www.lfspla.net`). Do not put secret values in Git.

`deploy/secrets/` is ignored by Git. SSH credentials should stay in SSH
configuration, an agent, or your secret manager.

`release_binary` and `frontend_dist` are relative to this `deploy/` directory;
after the documented release build, use `../target/release/lfsplanet` and
`../frontend2/dist`.

## First deployment

Build both artefacts locally:

```sh
cargo build --locked --release
npm --prefix ../frontend2 ci
npm --prefix ../frontend2 run build
```

Provision a new host, export the required session key from your secret manager,
then deploy:

```sh
export LFSPLANET_SESSION_KEY="$(openssl rand -hex 64)"
export LFSPLANET_HOST="lfsplanet.example.com"
cd deploy
uv run --with-requirements requirements.txt pyinfra inventory.py provision.py
uv run --with-requirements requirements.txt pyinfra inventory.py deploy.py
```

This requires [uv](https://docs.astral.sh/uv/) on the deployment workstation.
It resolves and runs the pyinfra dependency in an isolated environment; no
`.venv` needs to be created or activated.

For routine releases, run only `deploy.py`. Re-run `provision.py` only when
changing host-level dependencies or persistent host state.

The Caddyfile obtains and renews TLS certificates for `site_domain` and every
`alternate_domains` entry; ensure all of their DNS records point to the host
and ports 80 and 443 are reachable before deploying.

## Frontend releases

The deploy uploads `frontend_dist` to
`/opt/lfsplanet/frontend/releases/<digest>`, where `<digest>` is taken from the
bytes of the build. Content addressing makes re-uploading the same build a
no-op and keeps a published release immutable, then `current` is moved onto the
new directory with `rename(2)` so no request sees a partial document root.

Superseded releases are then removed; only the live one is kept. The API binary
is replaced in place with no old copies retained, and an old frontend has no
matching API to talk to once migrations have run, so keeping one alive would
preserve exactly the mismatch a reload fixes.

Discarding them is safe because SvelteKit already handles it. Caddy serves
`_app/immutable/` with a one-year `immutable` lifetime, since those paths are
content-hashed, and everything else with `no-cache`. Nothing under `/_app`
falls back to `index.html`: a request for a chunk a superseded release owned
answers 404 rather than returning HTML the browser would try to parse as
JavaScript. On that 404 the client re-reads `_app/version.json`, sees a version
it does not recognise and reloads onto the new release. That recovery is why
`version.json` must stay `no-cache`.

Compression is left to Caddy's `encode zstd gzip`, per request. Precompressing
at deploy time was measured and dropped: against Caddy's on-the-fly output it
saved 0.6% for gzip and 9% for zstd across 622 KiB of text assets. Brotli would
have saved 12%, around 24 KiB on a cold load, and is the only encoding Caddy
cannot produce on the fly. If that becomes worth having, generate the `.br`
files in the frontend build and add `precompressed br` to the `file_server`
directives, rather than compressing on the host.

## Response security headers

The Caddyfile is the only place the site's Content-Security-Policy, HSTS,
`Referrer-Policy`, `X-Frame-Options`, `X-Content-Type-Options` and
`Permissions-Policy` are set, and it applies them to proxied API responses as
well as to static files. The application sets none of them: it listens on
loopback only, so nothing reaches it without passing through Caddy, and the
policy governing a document belongs with whatever serves that document. Edit
`content_security_policy` in `group_data/all.py` to change it; a frontend that
needs a new origin no longer needs the binary rebuilt.

Before anything is stopped the deploy runs `caddy validate` against the
rendered file, so a broken site configuration fails while the running site is
still untouched.

## Database migrations

The deploy stops the web process, validator, and maintenance timer before
replacing `/opt/lfsplanet/lfsplanet` and running `lfsplanet migrate` as the
service account. It restarts those units only after migration succeeds. A
migration failure leaves them stopped so an older process cannot run against a
partially changed schema; resolve the migration failure and rerun the deploy.

Before stopping the application, Caddy switches to a static maintenance page
with HTTP status `503 Service Unavailable` and `Retry-After: 300`. The page
is removed only after every application unit has started successfully, so it
also remains visible if deployment fails.

That page covers the whole site, static frontend included, rather than only the
API. The frontend can do nothing useful while the API is migrating, and one
unambiguous 503 reads better than the SPA's generic error screen. Enabling it
before `current` moves also means the new frontend is never served against the
old API.

## Eras and initial database data

Run the separate era-policy deploy after the application deploy:

```sh
uv run --with-requirements requirements.txt pyinfra inventory.py eras.py
```

It synchronizes the catalogue every time and applies era definitions only when
pyinfra's file sync changes the deployed era files or their permissions. If an
apply fails after the files have synced, retry `era apply` manually on the host;
an unchanged sync will not trigger another apply.

New eras are created normally. For reviewed changes to an existing
era, set `replace_existing_eras = True` in inventory; this is required before
the deploy passes `--yes` to `era apply`. Do not enable it for ordinary
deploys.
