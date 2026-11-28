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
builds the frontend, deploys the application, then deploys catalogue definitions
and artwork. It expects `uv`, SSH access to the host, and the deployment
environment variables
described in [deployment](../deploy/README.md).

The deploy puts the site into maintenance mode, stops the web process,
validator, and maintenance timer, then replaces the application and runs
migrations. It starts the processes again only after the migration succeeds.
Maintenance mode stays on if the deploy fails, so an old process cannot run
against a partly changed database.

The application deployment does not by itself apply changed era definitions.
`just deploy` follows it with `deploy/catalogue.py`. Run `just deploy-catalogue`
to deploy catalogue changes alone. A new era can be applied normally. To replace
an existing era, first review the change and set
`replace_existing_eras = True` in the deployment inventory for that release.
Turn it off again afterwards.

After a release, check the public site, sign-in flow, an API request, and the
validator service. Read the service logs and confirm that queued hotlaps are
being processed. If a migration fails, fix the cause and rerun the deploy;
the site remains in maintenance mode until it succeeds.

Provision a new host separately. This installs host packages and creates
persistent paths. The application deploy creates the database. Follow [first deployment](../deploy/README.md#first-deployment)
before using `just deploy` on a new host.

Caddy validates its configuration before downtime. Frontend releases are
published atomically; only the current release is kept. The API binary is
replaced in place, so releases have no automatic rollback. Security headers are
configured in `deploy/group_data/all.py` and applied by Caddy.

For backup setup, checks and disaster recovery, see
[PostgreSQL backups](../deploy/runbooks/backups.md). Restore a replacement host before
enabling backups or deploying the application.

## Initialisation

For a new database:

```sh
lfsplanet --config /etc/lfsplanet/config.yaml migrate
lfsplanet --config /etc/lfsplanet/config.yaml vehicles sync-mods
lfsplanet --config /etc/lfsplanet/config.yaml seed /etc/lfsplanet/tracks/*.yaml /etc/lfsplanet/vehicles/*.yaml /etc/lfsplanet/eras/*.yaml
```

`vehicles sync-mods` requires LFS OAuth credentials and refreshes Vehicle Mods and
their covers. `seed` imports local track and standard vehicle artwork plus era
policy; it needs no LFS OAuth credentials. Canonical tracks and standard vehicles
are populated when their definitions are seeded. Apply these files explicitly.
PostgreSQL serves requests; startup does not apply catalogue files. See
[catalogue seeding](catalogue.md) for document formats and artwork handling.

## Importing LFSWorld hotlaps

Run migrations and install the required era definitions, then import one or more
CSV exports:

```sh
lfsplanet hotlaps import-lfsworld tmp/s2hotlaps.csv --spr-dir tmp/sprs
```

`--spr-dir` is optional. When supplied, the importer scans its subdirectories
once and matches the CSV's SPR filenames case-insensitively. On Unix it also
recognises Windows-1252 and Latin-1 filenames extracted from older archives,
without renaming the original files. Directory symlinks are not followed.

Matching replay headers must agree with the CSV's username, track, car and lap
time. Missing files leave the lap without an attachment; ambiguous filenames,
unreadable files, invalid headers and metadata mismatches are skipped with a
warning. The final attachment summary reports attached, missing, skipped and
already attached laps. Replays go into the configured attachment store.

Imported laps retain the CSV import's published state and do not enter the HLVC
queue. Rerun the command to attach files added later. Existing attachments are
preserved, including when rerunning without `--spr-dir`. Attachment writes commit
individually after the CSV import; a storage or database failure stops the pass,
and rerunning resumes with laps that still lack attachments.

## Routine maintenance

The daily `lfsplanet-maintenance.timer` runs these in order:

```sh
lfsplanet gc --all --yes
lfsplanet hotlaps rebadge --pending
lfsplanet race-calendar sync
lfsplanet vehicles sync-mods
```

A failing step stops the steps after it for that run; the next run retries them.

`gc` deletes expired browser sessions (`--sessions`), personal access tokens
revoked or expired more than `--access-token-max-age-days` ago (`--access-tokens`,
default 90), and unreferenced storage objects (`--storage`, below). Select targets
individually or with `--all`, and pass either `--yes` to delete or `--dry-run` to
report what is eligible. Every selected target runs even if an earlier one fails;
failures are reported together.

`vehicles sync-mods` requires LFS OAuth credentials. It also resolves hotlaps
waiting on Vehicle Mods metadata and caches covers for new Mods and new Mod
revisions in local object storage. Failed cover downloads leave a previously
cached image available and do not fail the catalogue refresh. The cached image
revision is tracked separately, so a failed download is retried on the next
sync.

`hotlaps rebadge --pending` retries persisted badge refresh requests left after interrupted
or failed publication, deletion, imports, or era edits. Successful publication
and deletion still attempt an immediate refresh. Calculation and replacement
hold the catalogue rebuild lock, briefly pausing admissions and other published
result changes while badges are calculated. Use `hotlaps rebadge` to force a
manual rebuild for selected eras.

`gc --storage` covers unreferenced SPRs, RAFs, vehicle images, and other objects
in the configured attachment store that are at least `--storage-max-age-hours`
old (default 24). Files shared by several records remain retained until every
reference is removed. It streams object listings in bounded batches, so it does
not load every stored key into memory:

```sh
lfsplanet gc --storage --dry-run --storage-max-age-hours 24
lfsplanet gc --storage --yes
```

Run `lfsplanet gc --all --dry-run` to see everything a scheduled run would delete.

## Player restrictions

Use `player` to deny or allow authentication and new hotlap uploads for an
existing LFS account. These commands use the exact LFS username.

Check a player's current restrictions:

```sh
lfsplanet --config /etc/lfsplanet/config.yaml players show LFS_USERNAME
```

To ban new uploads, deny `uploads`:

```sh
lfsplanet --config /etc/lfsplanet/config.yaml players deny uploads LFS_USERNAME
```

To stop a player from signing in or using a personal access token, deny `auth`:

```sh
lfsplanet --config /etc/lfsplanet/config.yaml players deny auth LFS_USERNAME
```

Allow the restriction again to unban the player. For example:

```sh
lfsplanet --config /etc/lfsplanet/config.yaml players allow uploads LFS_USERNAME
lfsplanet --config /etc/lfsplanet/config.yaml players allow auth LFS_USERNAME
```

## Recovery commands

After changing an era's replay-version rules, run `lfsplanet hotlaps fix-eras`.
Rebuild badges after rule changes or validator failures. Choose the eras:

```sh
lfsplanet hotlaps rebadge --era 2007-12-21
lfsplanet hotlaps rebadge --open
lfsplanet hotlaps rebadge --all
```

See [eras and rankings](eras.md) for policy changes and
[replay validation](validator.md) for host requirements.

To audit stored SPRs without modifying hotlaps, select an `era`, `player` or single `hotlap` scope.
For an era, optional `--player` narrows the selection before replay downloads:

```sh
lfsplanet hotlaps verify-sprs era 0.8
lfsplanet hotlaps verify-sprs era 0.8 --player mgxpower
lfsplanet hotlaps verify-sprs player mgxpower
lfsplanet hotlaps verify-sprs hotlap 123
```

Structured tracing events report hotlap/player/era IDs, track, vehicle, state
and rejection reason, followed by a verification summary. Test-drive detection requires both `Test Drive\0` and `TEST DRV` in replay
data. Other parser failures are reported too. Rows without SPR attachments are
counted separately; storage read failures are reported and make the command exit
unsuccessfully after finishing the selected scope. No HLVC process is run.

Add `--delete-invalid` to delete parser-rejected hotlaps in the selected scope:

```sh
lfsplanet hotlaps verify-sprs era 0.8 --delete-invalid
lfsplanet hotlaps verify-sprs player mgxpower --delete-invalid
lfsplanet hotlaps verify-sprs hotlap 123 --delete-invalid
```

Without the flag, verification is read-only. Storage read failures and hotlaps
without SPR attachments are never deleted. Deletions restore personal bests and
chart positions; badges are rebuilt once per affected era. Deletion or badge
rebuild failures are logged and cause an unsuccessful exit. Attachments remain
for storage GC.

After reviewing a result, remove a hotlap by ID:

```sh
lfsplanet hotlaps delete 123
```

This immediately deletes the row, including imported laps. For published laps it
restores the player's next valid personal best, reranks the chart and rebuilds
badges for the affected era. Unreferenced SPR/RAF attachments remain for storage
GC.
