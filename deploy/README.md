# Deployment

pyinfra deploys the Rust API and frontend to a Debian host using PostgreSQL 18,
systemd and Caddy. Requires SSH, sudo and [uv](https://docs.astral.sh/uv/).

## Configuration

Set these on your workstation. Keep secrets outside Git.

| Variable                        | Value                                                                  |
| ------------------------------- | ---------------------------------------------------------------------- |
| `LFSPLANET_HOST`                | SSH host                                                               |
| `LFSPLANET_SESSION_KEY`         | Saved 128-character hex key; generate once with `openssl rand -hex 64` |
| `LFSPLANET_OAUTH_CLIENT_ID`     | LFS OAuth client ID                                                    |
| `LFSPLANET_OAUTH_CLIENT_SECRET` | LFS OAuth secret                                                       |

Set the public domain and redirects in `inventory.py`; defaults are in
`group_data/all.py`. Point DNS at the host and allow ports 80 and 443.

For S3 attachments, see [storage configuration](../docs/attachments.md#configuration).
Leave all four bucket settings unset for local storage. Changing stores requires
[copying existing attachments](../docs/attachments.md#migration-between-stores).

For PostgreSQL backups, set the variables in [backup runbook](runbooks/backups.md) before
provisioning. Use a separate bucket from attachments.

## First deployment

From the repository root:

```sh
uv run --directory deploy --with-requirements requirements.txt pyinfra inventory.py provision.py
```

Provisioning installs packages, creates host paths and prepares pgBackRest
configuration. On a new database, activate backups:

```sh
uv run --directory deploy --with-requirements requirements.txt pyinfra inventory.py pgbackrest.py
```

This briefly restarts PostgreSQL. On a replacement host, [restore first](runbooks/backups.md#restore).

Build and deploy the application and eras:

```sh
just deploy
```

## Releases

Run `just deploy` from the repository root. It builds the API and frontend,
deploys both, then syncs eras. Reprovision only for host or backup configuration
changes.

Deploy era changes alone with `just deploy-eras`. To replace an existing era,
review the change and set `replace_existing_eras = True` in `inventory.py`;
reset it afterwards. If an era apply fails after syncing, retry `era apply`
manually on the host.

See [operations](../docs/operations.md#deployment-and-releases) for release checks
and failures, and [backups](runbooks/backups.md) for backup checks and recovery.
