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
lfsplanet --config /etc/lfsplanet/config.yaml maintenance sync-mods
lfsplanet --config /etc/lfsplanet/config.yaml seed /etc/lfsplanet/tracks/*.yaml /etc/lfsplanet/vehicles/*.yaml /etc/lfsplanet/eras/*.yaml
```

`sync-mods` requires LFS OAuth credentials and refreshes Vehicle Mods and
their covers. `seed` imports local track and standard vehicle artwork plus era
policy; it needs no LFS OAuth credentials. Canonical tracks and standard vehicles
are populated when their definitions are seeded. Apply these files explicitly.
PostgreSQL serves requests; startup does not apply catalogue files. See
[catalogue seeding](catalogue.md) for document formats and artwork handling.

## Importing LFSWorld hotlaps

Run migrations and install the required era definitions, then import one or more
CSV exports:

```sh
lfsplanet hotlap import-lfsworld tmp/s2hotlaps.csv --spr-dir tmp/sprs
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

```sh
lfsplanet maintenance sessions-gc
lfsplanet maintenance access-tokens-gc
lfsplanet maintenance sync-mods
lfsplanet maintenance storage-gc
lfsplanet maintenance run-all
```

`sessions-gc` removes expired browser sessions. `access-tokens-gc` retains
expired or revoked personal access tokens for 90 days before removal.
`sync-mods` requires LFS OAuth credentials. It also resolves hotlaps
waiting on Vehicle Mods metadata and caches covers for new Mods and new Mod
revisions in local object storage. Failed cover downloads leave a previously
cached image available and do not fail the catalogue refresh. The cached image
revision is tracked separately, so a failed download is retried on the next
sync.

`run-all` retries persisted badge refresh requests left after interrupted
or failed publication, deletion, imports, or era edits. Successful publication
and deletion still attempt an immediate refresh. Calculation and replacement
hold the catalogue rebuild lock, briefly pausing admissions and other published
result changes while badges are calculated. Use `hotlap rebadge` to force a
manual rebuild for selected eras.

`storage-gc` reports unreferenced SPRs, RAFs, vehicle images, and other objects
in the configured attachment store that are at least 24 hours old. Files shared
by several records remain retained until every reference is removed. It
streams object listings in bounded batches, so it does not load every stored
key into memory. It never deletes unless asked:

```sh
lfsplanet maintenance storage-gc --older-than-hours 24
lfsplanet maintenance storage-gc --delete
```

`run-all` runs the routine maintenance tasks once. Its `--delete` flag affects only
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

## Player roles

Accounts have one fixed role: `player` (the default) or `moderator`. Set or
remove moderator access using an existing LFS account:

```sh
lfsplanet --config /etc/lfsplanet/config.yaml player set-role LFS_USERNAME moderator
lfsplanet --config /etc/lfsplanet/config.yaml player set-role LFS_USERNAME player
lfsplanet --config /etc/lfsplanet/config.yaml player show LFS_USERNAME
```

Role changes apply on the next authenticated request, including existing browser
sessions and personal access tokens. Signing in again preserves the assigned
role. Authentication bans and CSRF protection still apply to moderators.
`GET /api/v1/me` exposes `role` as `"player"` or `"moderator"`, or `null`
when signed out. The frontend session exposes `session.me.role` and
`session.isModerator`; existing background refreshes update these values.
These values control UI visibility; the backend extractor enforces access.
Role assignment is CLI-only; there is no web endpoint to change a role.

The Rust `Role` enum maps to `player` and `moderator` in a `VARCHAR(16)` column.
There is no database enum or check constraint; unknown values fail decoding.
Moderation handlers can require the `Moderator` extractor alongside the existing
`AuthenticatedPlayer` extractor for ordinary authenticated operations. It returns
401 for unauthenticated callers and 403 (`moderator_required`) for players.
Moderators can resolve and reopen reports from the hotlap’s Conversation tab.

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

### Pausing uploads for individual rankings

Set `hotlaps.blocked_rankings` in the server configuration and restart the API:

```yaml
hotlaps:
  blocked_rankings:
    - era: "2026-08-26"
      ranking: drag
```

This pauses uploads for every track/vehicle combination in the current 0.8 drag
ranking. Add more entries to block additional rankings. Any blocked membership
rejects an upload, including combinations shared with an unblocked ranking.
The global `allow_uploads`, player `deny_uploads`, and closed-era restrictions
still apply independently. The replay header determines the combination; clients
cannot bypass the check by selecting another ranking. Rejection happens before
replay storage or queue admission. Existing laps and validation jobs are unaffected.
Remove an entry and restart the API to reopen that ranking.

### Public hotlap conversations and reports

A hotlap has one public conversation. Ordinary comments and report comments use
one `hotlap_comment` table and one API response schema. A non-null reason marks
an entry as a report, with its own open/resolved status. Reports require a reason
and a 1–4000 character plain-text explanation. Both kinds of entry can receive
replies. Reading is public; posting and managing your own content require login.

Reasons are the Rust `ReportReason` enum: `suspected_cheating`, `invalid_lap`,
`incorrect_classification`, and `other_problem`. OpenAPI generates the matching
SDK type; the frontend supplies display labels. Reasons are stored as text with
no database enum or check constraint. A partial unique index on hotlap/author
allows one active report per player per lap, alongside any number of comments.
Changing a report's explanation or reason reopens it; saving unchanged content
preserves its review. Moderators resolve/reopen individual reports. Resolution
does not close replies, invalidate laps, or change rankings.

Parents are immutable and must belong to the same lap. Replies are limited to
eight levels. Open report roots sort first; ordinary comments, withdrawn reports
and resolved reports follow in chronological order. Replies stay chronological
under their parent. Ordering applies before pagination, and `around` uses the
same priority when finding a comment’s page, including a reply’s root.
Pagination selects root conversations, and the server returns
nested `replies` for their descendants. Four batched queries load a non-empty
page: count roots, fetch roots with authors, fetch descendants with authors, and
group reply counts. The hotlap endpoint adds one published-lap lookup. The optional `around` comment parameter adds two fixed queries to locate the
normal root page containing a comment or reply, preserving surrounding context. Query
count does not grow with root count or nesting depth. A `(root_id, id)` index
supports the descendant fetch.

Responses include at most 1,000 additional descendants. Each entry's `reply_count`
identifies incomplete branches; those offer an explicit Load more replies action.
The client expands included replies by default and performs no per-entry fetch
while rendering. The direct-replies endpoint pages additional children with their
nested descendants using the same batching. Deleting your comment removes its text
and any report flag, retaining a deleted placeholder and its replies. Withdrawing
a report only clears its flag and review state: its explanation and replies
remain as an ordinary conversation. Both actions use AlertDialog confirmations.
A stable original hotlap ID preserves scoped access after the nullable lap
reference is cleared. Lap removal preserves conversations as read-only archives; archived entries
remain readable and manageable through their original hotlap-scoped API paths.

Reported hotlaps are available in the public Activity views on the home page
(`/?view=reported`) and each era overview (`/hotlaps/{era}?view=reported`). Both
use `GET /api/v1/hotlaps?reported=true`; era views also supply `era_id`. Each lap
appears once and leaves the view when its last open report is resolved.
`reported=false` selects laps with no open reports, including those with only
resolved reports. Lap-time links open the conversation directly. There is no
standalone frontend `/reports` page or Reports navigation item. Past reports
remain visible in the individual lap conversation. Individual report
operations also belong to their hotlap: GET/PUT `/api/v1/hotlaps/{hotlap}/report`
reads or updates your report. DELETE/PATCH
`/api/v1/hotlaps/{hotlap}/comments/{comment}/report` withdraws or moderates a flag.
GET `/api/v1/hotlaps/reports/summaries?hotlap_ids=...` provides batched chart flags.
There are no standalone `/api/v1/comments` or `/api/v1/reports` routes.
GET/POST `/api/v1/hotlaps/{hotlap}/comments` list/create root conversations; creation
accepts optional `parent_id` for replies. GET `/api/v1/hotlaps/{hotlap}/comments/{comment}` reads
an entry with nested replies, GET `/api/v1/hotlaps/{hotlap}/comments/{comment}/replies` paginates direct replies, and
DELETE `/api/v1/hotlaps/{hotlap}/comments/{comment}` removes your text while retaining the entry.
All lists use the usual page parameters. Cookie-authenticated writes require
CSRF tokens; conversation endpoints use `Cache-Control: no-store`.

The WIP report migration is edited in place to create the unified table; the
separate report and thread tables are gone. Recreate development databases that
ran an earlier version before migrating. No production database was changed.

### Individual hotlap pages

`/hotlap/{id}` is the permanent frontend address for an individual lap. Chart,
activity, submission, comparison, and ranking-combination tables expose an info button linking
to it, alongside available SPR and RAF downloads. Report entries also link back
to the lap. Published
laps are public; unpublished or rejected uploads are visible only to their owner,
as enforced by the existing hotlap detail API.

The page shows the lap time, splits and sectors, driver, era, game version,
controller and driving aids, plus available replay and telemetry downloads.
The layout follows driver profiles, with an identity header and lap-time highlight.
Available SPR/RAF downloads, a labelled report button, and owner-only
removal in the right-hand breadcrumb action area. Actions wrap below the trail
on smaller screens; removal requires confirmation. The title links the driver and
era/track/vehicle combination without a duplicate metadata row. Lap details are
shown under Overview, with technical metadata and split times retained there.
Conversation has its own tab and contains both ordinary comments and reports.
Entries use horizontal separators; report entries have a yellow side accent,
tinted background and prominent review status. Reply indentation is capped on
narrow screens. New posts navigate to their normal page and scroll into position,
with surrounding conversations still visible. A link
`/hotlap/{id}?tab=conversation&around={id}#comment-{id}` opens that context.

The shared Empty component handles an empty conversation and empty report lists.

Comment deletion, report withdrawal, hotlap/submission removal, token revocation
and webhook deletion use the shared Bits UI AlertDialog confirmation. It stays
open during requests and displays errors without dismissing the confirmation.

Owners also see their private submission information and can remove their own
upload through a confirmation dialog. Existing test validation is available only
when enabled by server configuration. Removing a lap preserves its reports.
