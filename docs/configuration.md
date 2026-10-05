# Configuration

The config file is `planet.yaml`. Use `--config PATH` (or `-c`) to change it.
For local development:

```sh
just setup
```

This uses database host `localhost`, public URL `http://localhost:5173`, HTTP
cookies, and enables Force validate. Existing files are never overwritten.
Replace the generated `REPLACE_ME` OAuth values with credentials from
[developer setup](development.md).

The task selects `--development`. The CLI requires `--production` or
`--no-production` (also called `--development`); they cannot be combined.
Production mode enables secure cookies and disables Force validate.
Set its public URL and database URL before use.

Deployment uses HTTPS and secure cookies.

`worker.hlvc.poll_interval_seconds` and `worker.hlvc.timeout_seconds` control
hotlap validation. `worker.webhooks.workers` sets concurrent deliveries per
worker process and defaults to 3. Deliveries to the same destination are
serialized. Keep the count within the worker's database connection limit. The
deployment inventory sets it with `webhook_workers` in
`deploy/group_data/all.py`.

## Object storage

`storage.object_store_root` accepts a filesystem path or an object-store URL.
The existing local configuration remains valid:

```yaml
storage:
  object_store_root: data/storage
```

Relative filesystem paths resolve against the configuration file's directory.
The directory is created when the store is opened. Absolute paths and local
`file://` URLs are also supported.

S3 support is enabled by default. Compile with `cargo build --release` and configure:

```yaml
storage:
  object_store_root: s3://my-bucket/lfsplanet
```

The URL selects the bucket and prefix. Replay and image keys remain relative to
that prefix, including during listing and garbage collection. Configure the
backend through environment variables such as `AWS_REGION`,
`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and `AWS_SESSION_TOKEN` when needed.
S3-compatible services can also supply `AWS_ENDPOINT`. The bucket must already
exist. Web, worker and maintenance processes must use the same location and
backend environment.

Other supported remote backends require their corresponding Cargo feature
(`azure`, `gcp`, or `http`). An unavailable backend fails when the store is
constructed. Changing the location does not migrate existing objects; copy them
to the new location while preserving their relative keys.

## Local LFS runtime

All LFS settings live under `lfs`. Platform runtime fields are flattened within
that section alongside installation locations, OAuth credentials and HTTP limits:

```yaml
lfs:
  installation_root: /srv/lfs
  wine_prefix: /srv/lfs/.wine
  wine_executable: /usr/bin/wine
  bubblewrap_executable: /usr/bin/bwrap
  outbound_http_timeout_seconds: 15
```

Move existing top-level `lfs_installation_root` to `lfs.installation_root`.
Remove `installer_download_root`; installers are cached automatically under
`installation_root/.cache/installers`. Move fields from `lfs_runtime` directly
beneath `lfs`. The old top-level names are rejected. Relative installation and
Wine prefix paths resolve against the configuration file's directory;
executable paths are used as given.

Linux uses Wine and Bubblewrap for installation, updates and replay validation.
Other platforms ignore unrecognized fields in `lfs` and return an unsupported
error for those operations. Linux rejects unknown fields in `lfs`; other sections
retain their existing validation on every platform. Installation trimming uses portable filesystem code.
The unsupported implementation is also compiled and tested on Linux.
