# Attachment storage

SPR, RAF, vehicle images, and track artwork share one store. New files use
`<first-two-hash-characters>/<sha256>.<extension>`.

For existing database, paths may stay unchanged.
Each attachment has its own UUID and filename; paths can be shared.
Anyone with the UUID can download it.

All attachments are streamed through the API for every storage backend and
cached in the browser for 24 hours (`Cache-Control: private, max-age=86400`).
Content types and download filenames come from the attachment record, so existing
objects need no metadata repair. Replacing a file creates a new attachment UUID
and URL. A previously cached file can remain available for up to 24 hours after
its attachment is deleted. Missing attachments and objects return uncached 404s.

## Configuration

Local development needs no changes to `planet.yaml`.

For S3, set these on the deployment machine:

```sh
export LFSPLANET_BUCKET_NAME=lfsplanet
export LFSPLANET_BUCKET_HOST=https://uk-lon-1.katapultobjects.com:443
export LFSPLANET_BUCKET_ACCESS_KEY_ID='your-access-key'
export LFSPLANET_BUCKET_SECRET_ACCESS_KEY='your-secret-key'
unset LFSPLANET_BUCKET_REGION
```

## Migration between stores

1. Stop web, workers, maintenance, and catalogue sync.
2. Copy the **contents** of `/var/lib/lfsplanet/spr/` to the bucket root. Preserve
   relative paths: `spr/ab/file.spr` stays `spr/ab/file.spr`. No renaming.
3. Deploy. Deployment runs migrations and restarts services. The old binary
   cannot use the migrated schema.
4. Check existing SPR/RAF downloads, images, and a new upload.
5. Run GC dry-run before resuming scheduled deletion. Keep the local backup
   until verified.

Manual S3 CLI commands need the service credentials:

```sh
export AWS_ACCESS_KEY_ID="$LFSPLANET_BUCKET_ACCESS_KEY_ID"
export AWS_SECRET_ACCESS_KEY="$LFSPLANET_BUCKET_SECRET_ACCESS_KEY"
export AWS_ENDPOINT="$LFSPLANET_BUCKET_HOST"
export AWS_REGION=us-east-1
lfsplanet --config /etc/lfsplanet/config.yaml maintenance storage-gc
```

## Cleanup & Safety

Use a dedicated bucket or prefix. Do not mix and match.

GC scans the whole store and deletes unreferenced files after 24 hours by default.
Deleting a hotlap removes its references; shared files remain until unused.

GC discovers foreign keys to `attachment.id` from PostgreSQL rather than keeping
an asset-type list. Any referenced attachment retains its object, including
when other attachment rows share that object key. Unreferenced attachment rows
and objects remain eligible under the normal minimum-age rules.
