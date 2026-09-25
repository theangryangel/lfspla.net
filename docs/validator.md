# Replay validation

The validator runs LFS HLVC against queued SPR replays. It requires Linux,
Wine, and Bubblewrap.

## Host requirements

- Wine
- Bubblewrap 0.11.0 or newer, not setuid
- Unprivileged user namespaces enabled on the host
- A persistent LFS installation root, installer-download directory, and the
  parent of a shared Wine prefix

These requirements are not verified at startup.

## Installing LFS

`lfsplanet lfs` manages named installations beneath `lfs_installation_root`. A name is a
safe path component and is not derived from an era.

```sh
lfsplanet lfs install 0.8 \
  --download-url 'https://www.lfs.net/file_lfs.php?name=LFS_S3_8C20_setup.exe' \
  --username YOUR_USERNAME \
  --unlock-code "$LFS_UNLOCK_CODE"

lfsplanet lfs update 0.8
lfsplanet lfs path 0.8
```

To diagnose one local replay without uploading or storing it, run it through
the same HLVC sandbox directly. The command prints Bubblewrap and Wine exit
codes plus captured stdout and stderr:

```sh
lfsplanet hotlap validate 0.8 /path/to/replay.spr
```

Downloads are cached in `installer_download_root`, outside the destination,
using the requested URL as the cache key. This directory must be on disk rather
than a size-constrained `/tmp` tmpfs. Delete a cached file to force a fresh
download. The installer is extracted without network access, prepared with
`LFS.exe /nogfx=extract`, then unlocked with network access. Existing
installations are never replaced; a failed install remains available for
inspection.

Remove expendable version-dependent data after installation or update:

```sh
lfsplanet lfs trim 0.7 --dds --yes
lfsplanet lfs trim 0.8 --dds --lgh --yes
```

`--dds` clears `data/dds` while retaining the directory. `--lgh` removes only
top-level `*.lgh` files in `data/wld`; both commands report targets and require
`--yes`.

It is your responsibility to ensure that this is correctly applied to the right versions.

Later down the line we may automate this.

## Running the worker

```sh
lfsplanet worker
```

The worker selects pending hotlaps with `FOR UPDATE SKIP LOCKED`, holding the
transaction until validation and its result are saved. Each worker needs a
database connection. Shutdown on SIGINT or SIGTERM lets the current attempt finish.
`hotlap watch` remains an alias for `worker`.

Infrastructure failures retry after 30 seconds, up to five completed attempts.
Exhausted hotlaps remain pending with `error_detail` populated, and still count
against the upload allowance. Crashes roll back the attempt, including its
counter. To retry an exhausted record, reset its `attempt_count` to zero and
`next_attempt_at` to NULL. Invalid replay verdicts are final, not retries.

Stop the old validator before applying the record-processor cutover migration.
It removes unresolved-vehicle hotlaps (object GC handles their replays), resets
outstanding validation work, and removes the unused shared jobs table.

The worker validates replays in an ephemeral Bubblewrap overlay, except for the
installation's `mods/` and `cache/` directories. LFS may retain downloaded
Vehicle Mods and cached data there for later validations. Other installation
and Wine-prefix writes disappear with the mount namespace.

`hotlaps.allow_test_validation` exposes an owner-scoped test endpoint that
publishes a replay without HLVC. It is useful for ranking development but must
remain disabled in production.

## Webhook notifications

Users manage webhook subscriptions at `/account/webhooks`. Each enabled
subscription can receive every newly validated upload, including laps that do
not improve a personal best, or only events where a validated upload becomes
first on its chart (`WorldRecordSet`). Imported historical results do not emit
notifications. The test-validation endpoint queues notifications too.

`WebhookEvent` owns the event snapshot; Strum generates `WebhookEventKind` for
subscription selection. `WebhookFormat` selects the delivery implementation,
currently Discord. These models and rendering live in `src/models/webhooks`. A world
record event is emitted only after the chart has been reranked and the new lap's
stored chart position is exactly one.

HLVC inserts `webhook_notification` rows in the same transaction as validation
and ranking. A unique subscription/hotlap/event constraint prevents duplicate
queue entries when a hotlap is revalidated. Deleting a hotlap or subscription
also removes its notifications. Pausing a subscription holds queued work and
prevents new work; resuming does not backfill events missed while paused.

`lfsplanet worker` runs validation and webhook delivery as separate Tokio tasks.
The delivery processor in `src/jobs/webhooks.rs` locks both the notification and
subscription with `FOR UPDATE SKIP LOCKED`. This serializes sends for each
subscription across processes. Discord rate-limit cooldowns are stored on the
subscription. Each request has a 15-second timeout. Network errors and server
errors retry with exponential delays starting at 30 seconds, up to eight
attempts. Other permanent failures terminate that notification; HTTP 401, 403,
and 404 also pause the subscription. Failed records retain `error_detail` for
operator inspection. Resuming a subscription does not retry terminal failures.

Delivery is at least once: a crash after Discord accepts a message but before
commit can cause a duplicate. Webhook URLs contain credentials and are not
returned by the API or included in delivery errors. Only Discord HTTPS webhook
endpoints are accepted, and HTTP redirects are disabled. Discord sends use
`wait=true`, suppress mentions, and follow the documented rate-limit headers:
[execute webhook](https://docs.discord.com/developers/resources/webhook#execute-webhook)
and [rate limits](https://docs.discord.com/developers/topics/rate-limits).
The configured `web.public_base_url` supplies chart links.

The account API exposes GET/POST `/api/v1/me/webhooks` and PATCH/DELETE
`/api/v1/me/webhooks/{id}`, using browser authentication and CSRF protection for
mutations. PATCH accepts `enabled`; POST accepts `name`, `url`, `format`
(`discord`), and `event_kind` (`hotlap_validated`).
