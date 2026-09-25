# Operations - Running lfspla.net

`lfsplanet` is one binary with distinct process roles. Deployment should run
them independently so a validator restart does not interrupt the web service.

| Role        | Command                                                         | Behaviour                                            |
| ----------- | --------------------------------------------------------------- | ---------------------------------------------------- |
| Web         | `lfsplanet --config /etc/lfsplanet/config.yaml web`             | Serves HTML and the JSON API until stopped.          |
| Validator   | `lfsplanet --config /etc/lfsplanet/config.yaml worker`          | Continuously validates queued replays until stopped. |
| Maintenance | `lfsplanet --config /etc/lfsplanet/config.yaml maintenance ...` | Runs one task and exits; schedule externally.        |
| Migrate     | `lfsplanet --config /etc/lfsplanet/config.yaml migrate`         | Applies pending schema migrations and exits.         |

Run `migrate` before starting database-using commands after a new release.
Migrations create and evolve schema only; they do not synchronise catalogue
data or automatically apply eras.

## Deployment and releases

The checked-in deployment uses pyinfra, systemd, Caddy, and PostgreSQL on a
Debian-family host. Run `just deploy` from the repository root for a normal
release:

```sh
just deploy
```

It builds the locked Rust release, installs the locked frontend dependencies,
builds the frontend, deploys the application, then deploys the era files. It
expects `uv`, SSH access to the host, and the deployment environment variables
described in [deployment](../deploy/README.md).

The deploy puts the site into maintenance mode, stops the web process,
validator, and maintenance timer, then replaces the application and runs
migrations. It starts the processes again only after the migration succeeds.
Maintenance mode stays on if the deploy fails, so an old process cannot run
against a partly changed database.

The application deployment does not by itself apply changed era definitions.
`just deploy` follows it with the era deployment. A new era can be applied
normally. To replace an existing era, first review the change and set
`replace_existing_eras = True` in the deployment inventory for that release.
Turn it off again afterwards.

After a release, check the public site, sign-in flow, an API request, and the
validator service. Read the service logs and confirm that queued hotlaps are
being processed. If a migration fails, fix the cause and rerun the deploy;
the site remains in maintenance mode until it succeeds.

Provision a new host separately. This installs host packages and creates the
database and persistent paths. Follow [first deployment](../deploy/README.md#first-deployment)
before using `just deploy` on a new host.

## Initialisation

For a new database:

```sh
lfsplanet --config /etc/lfsplanet/config.yaml migrate
lfsplanet --config /etc/lfsplanet/config.yaml maintenance catalogue-sync --standard-vehicle-images-dir /etc/lfsplanet/builtin-vehicles
lfsplanet --config /etc/lfsplanet/config.yaml era apply /etc/lfsplanet/eras/*.yaml
```

`catalogue-sync` adds tracks and built-in vehicles. Pass
`--standard-vehicle-images-dir` to seed built-in image files into object
storage. With LFS OAuth, it also refreshes Vehicle Mods. Apply `assets/eras/`
files yourself. PostgreSQL serves
requests; startup does not apply era files.

## Routine maintenance

```sh
lfsplanet maintenance sessions-gc
lfsplanet maintenance access-tokens-gc
lfsplanet maintenance badges-refresh
lfsplanet maintenance catalogue-sync
lfsplanet maintenance storage-gc
lfsplanet maintenance run-all
```

`sessions-gc` removes expired browser sessions. `access-tokens-gc` retains
expired or revoked personal access tokens for 90 days before removal.
`catalogue-sync` also resolves hotlaps waiting on Vehicle Mods metadata and,
when OAuth is configured, caches covers for new Mods and new Mod revisions in
local object storage. Failed cover downloads leave a previously cached image
available and do not fail the catalogue refresh. The cached image revision is
tracked separately, so a failed download is retried on the next sync.

`badges-refresh` retries persisted badge refresh requests left after interrupted
or failed publication, deletion, imports, or era edits. Successful publication
and deletion still attempt an immediate refresh. Calculation and replacement
hold the catalogue rebuild lock, briefly pausing admissions and other published
result changes while badges are calculated. `run-all` includes this retry task.

`storage-gc` reports replay objects and cached Vehicle Mod covers that are no
longer retained by their database records and are at least 24 hours old. It
streams object listings in bounded batches, so it does not load every stored
key into memory. It never deletes unless asked:

```sh
lfsplanet maintenance storage-gc --older-than-hours 24
lfsplanet maintenance storage-gc --delete
```

`run-all` runs every maintenance task once. Its `--delete` flag affects only
eligible storage objects; it attempts all tasks and reports failures together.

## Player restrictions

Use `player` to deny or allow authentication and new hotlap uploads for an
existing LFS account. These commands use the exact LFS username.

Check a player's current restrictions:

```sh
lfsplanet --config /etc/lfsplanet/config.yaml player show LFS_USERNAME
```

To ban new uploads, deny `uploads`:

```sh
lfsplanet --config /etc/lfsplanet/config.yaml player deny uploads LFS_USERNAME
```

To stop a player from signing in or using a personal access token, deny `auth`:

```sh
lfsplanet --config /etc/lfsplanet/config.yaml player deny auth LFS_USERNAME
```

Allow the restriction again to unban the player. For example:

```sh
lfsplanet --config /etc/lfsplanet/config.yaml player allow uploads LFS_USERNAME
lfsplanet --config /etc/lfsplanet/config.yaml player allow auth LFS_USERNAME
```

## Recovery commands

After changing an era's replay-version rules, run `lfsplanet hotlap fix-eras`.
Rebuild badges after rule changes or validator failures. Choose the eras:

```sh
lfsplanet hotlap rebadge --era 2007-12-21
lfsplanet hotlap rebadge --open
lfsplanet hotlap rebadge --all
```

See [eras and rankings](eras.md) for policy changes and
[replay validation](validator.md) for host requirements.
