# PostgreSQL backups

> [!WARNING]
> The attachment store is not backed up, and currently trusting the S3 storage.
> This is dumb.
> This should be resolved at a later date, ideally with a combined snapshot.
>
> It is arguable that we should build this into lfsplanet itself perhaps given we're
> using object_store storage...

pgBackRest backs up PostgreSQL `18/main` to a separate S3 bucket.
`provision.py` installs the client and configuration; `pgbackrest.py` enables
archiving and timers. Recovery is manual.

## Setup

Create the bucket. Give a separate key list/read/write/delete access; keep the
application key out of it. Set these on the deployment workstation:

```sh
export LFSPLANET_BACKUP_BUCKET_NAME="your-bucket-name"
export LFSPLANET_BACKUP_BUCKET_HOST="your-provider"
export LFSPLANET_BACKUP_BUCKET_ACCESS_KEY_ID="your-backup-key"
export LFSPLANET_BACKUP_BUCKET_SECRET_ACCESS_KEY="your-backup-secret"
export LFSPLANET_BACKUP_CIPHER_PASS="your-saved-repository-passphrase"
```

Generate the passphrase once with `openssl rand -hex 32`. Save it outside Krystal
and reuse it. Changing it does not rotate repository encryption.

Follow [first deployment](../README.md#first-deployment). Rerun provisioning when
changing credentials or repository settings. Configuration contains secrets;
avoid pyinfra's `--diff` with production values.

Full backups run Sundays; differential backups Monday–Saturday at 02:00 host
time, plus up to 15 minutes of jitter. Missed runs catch up after boot. WAL
archiving runs continuously, with a five-minute segment timeout. Failed uploads
delay recovery coverage and accumulate WAL on the host.

pgBackRest manages retention. Do not expire repository objects with bucket
lifecycle rules. Activation verifies cluster identity and takes an initial full
backup only when none exists, without expiring backups. Scheduled backups apply
retention normally.

## Checks

On the host:

```sh
sudo -u postgres pgbackrest --stanza=lfsplanet info
sudo -u postgres pgbackrest --stanza=lfsplanet check
systemctl list-timers 'pgbackrest-backup-*'
journalctl -u pgbackrest-backup@full.service -u pgbackrest-backup@diff.service
sudo -u postgres psql -d postgres -c 'SELECT * FROM pg_stat_archiver;'
```

Monitor backup age, failed jobs, archiving failures and disk space. Notifications
are not configured. Test a restore before relying on these backups.

## Restore

These steps assume a replacement Debian/Ubuntu host with PostgreSQL 18.
Keep the old host stopped or fenced throughout recovery.

### Prepare

On the workstation, set the same backup variables and target the replacement
host with `LFSPLANET_HOST`. Keep the other inventory variables set too.

```sh
uv run --directory deploy --with-requirements requirements.txt pyinfra inventory.py provision.py
```

Do not run `pgbackrest.py` or deploy the application yet. Keep any existing
application services, maintenance and backup timers stopped.

### Recover

On the replacement host, stop PostgreSQL and preserve its initialized database.
Use an unused name for `main.before-restore`:

```sh
sudo pg_ctlcluster 18 main stop
sudo mv -T /var/lib/postgresql/18/main /var/lib/postgresql/18/main.before-restore
sudo install -d -o postgres -g postgres -m 0700 /var/lib/postgresql/18/main
sudo -u postgres pgbackrest --stanza=lfsplanet info
sudo -u postgres pgbackrest --stanza=lfsplanet --type=default restore
```

This restores the latest backup and replays available WAL. For an application
mistake, choose a backup and time target instead: `--set`, `--type=time`,
`--target` and `--target-action=pause`. See [pgBackRest recovery options](https://pgbackrest.org/command.html#command-restore).
Map user-defined tablespaces to empty destinations with `--tablespace-map-all`
or `--tablespace-map`. Preserve a failed partial restore before retrying.

Keep `archive_mode=off` during verification. Review `/etc/postgresql/18/main/`
and restored `postgresql.auto.conf`; remove any persistent archive-mode override
from `postgresql.auto.conf` before later enabling backups.

### Verify and resume

```sh
sudo pg_ctlcluster 18 main start
sudo -u postgres psql -d postgres -c 'SELECT pg_is_in_recovery();'
```

Wait for `false`. For a paused time-targeted restore, verify the database first,
then call `pg_wal_replay_resume()` and wait for recovery to finish.
Check recovered data and attachment references.

On the workstation, enable backups, then deploy:

```sh
uv run --directory deploy --with-requirements requirements.txt pyinfra inventory.py pgbackrest.py
just deploy
```

## Scope

Backups include the PostgreSQL cluster and roles. They exclude host/application
configuration, release artefacts and attachments. An older database can reference
attachments already deleted by GC. Both buckets share Krystal's provider risk.

Keep the bucket, `/lfsplanet` repository prefix, `lfsplanet` stanza and passphrase
stable during replacement. A fresh database has a different system identifier;
activation refuses it until restored. Never reset the repository to bypass that
check. Use a new bucket for an independent database.
