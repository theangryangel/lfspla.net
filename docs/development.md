# Local development

## Prerequisites

- Node.js 24 or newer, for the frontend
- Docker and Docker Compose, for the development PostgreSQL instance
- `just`, for the tasks in the repository's [Justfile](../Justfile)
- `tmux`, for `just dev` (optional if you run the processes separately)

The Rust backend serves the API and authentication routes. The browser
application lives in `frontend2/` and runs through Vite during development.

Run the commands below from the repository root. Run `just` (or `just --list`)
to see all available tasks.

## First-time setup

```sh
npm --prefix frontend2 ci
cargo run --locked -- generate-config > planet.yaml
docker compose up -d
just seed
```

`just seed` runs database migrations, syncs the track and vehicle catalogue,
and applies the definitions in `assets/eras/*.yaml` without a confirmation prompt.
PostgreSQL must be ready before it runs.

Generate `planet.yaml` only on first setup, since the command overwrites an
existing file. It is ignored by Git because it contains a cookie-encryption
key and may contain OAuth credentials. See [configuration](configuration.md)
for configuration options.

## Daily development

```sh
just dev
```

This starts a tmux session named `lfs-planet` with panes for PostgreSQL,
the Vite frontend, and the Rust backend. If the session already exists, it
attaches to it. Complete the first-time setup before running it.

Open the frontend URL printed by Vite (normally <http://localhost:5173>).
Vite proxies `/api` and `/auth` to the backend at <http://localhost:8000>.
The backend also serves the [OpenAPI document](http://localhost:8000/api/openapi.json)
and [Swagger UI](http://localhost:8000/api/docs).

Detach from tmux with `Ctrl-b`, then `d`; the processes keep running. Run
`just dev` again to reattach. To stop a process, press `Ctrl-c` in its pane.
Stop PostgreSQL with `docker compose down`; add `--volumes` only when you
want to discard its development data.

Run `just migrate` whenever new schema migrations are pending. Run `just seed`
when you also need to refresh the catalogue and apply era definitions.

## Generating the track outlines

Track outlines are generated from the path files in an LFS installation, which
ships one per configuration in `LFS/data/pth`:

```sh
just track-gen ~/LFS/data/pth/*.pth
```

Each output is named after the file it came from, so this lands
`assets/tracks/BL1.svg` for `BL1.pth` and needs no mapping. Existing files are
overwritten, and the whole directory can be regenerated at any time: the
drawings are deterministic, so a regeneration that changes nothing produces no
diff.

Re-run it when LFS ships a new configuration, or when the drawing itself
changes. Commit the result.
[crates/lfsplanet_track_gen/README.md](../crates/lfsplanet_track_gen/README.md)
covers what it draws, how its colours reach the theme, and the flags for size
and detail.

Built-in vehicle images live in `assets/builtin-vehicles/`, named after their
vehicle codes. `just seed` uploads them to local object storage and associates
them with the built-in vehicle rows; the frontend gets all vehicle images from
the API.

## Tests and checks

The repository uses [prek](https://prek.j178.dev/) for Git hooks that format
files and run Rust checks. Install the hooks or run them manually with:

```sh
prek install --prepare-hooks
prek run --all-files
```

PostgreSQL regression tests are ignored by the ordinary test command. Run them
against a disposable PostgreSQL server using a role with permission to create
databases:

```sh
DATABASE_URL=postgres://user:password@localhost/test cargo test --locked -p lfsplanet database_tests -- --ignored
```

SQLx creates and migrates a separate database for each test. CI runs these tests
against its own PostgreSQL service; no LFS installation or OAuth credentials are
needed.

## Logging and validator work

The normal filter is `warn,lfsplanet=info`. For SQL query logging while keeping
that baseline:

```sh
RUST_LOG='warn,lfsplanet=info,sqlx::query=info' cargo run --locked -- web
```

`worker` requires an LFS installation, Wine, and Bubblewrap. It is
separate from the ordinary web-development loop; see
[replay validation](validator.md).

## Project updates

Project changes are tracked in the repository [CHANGELOG](../CHANGELOG.md).
