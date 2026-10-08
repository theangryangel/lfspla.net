# Development quick start

You need stable Rust, Node.js 24+, Docker Compose, `just`, and `tmux`.

For local development with synthetic data, run `just setup`, `just demo`, then
`just dev` and open <http://localhost:5173/hotlaps/demo>. Demo generation inserts
missing built-in vehicles and tracks automatically; you do not need to run
`just seed` or configure LFS API credentials. See [demo data](#demo-data).

To enable LFS sign-in or sync the full catalogue, including vehicle mods,
configure LFS API credentials as described below. Both sign-in and the vehicle
mod API used by `just seed` require these credentials:

1. Run `just setup`. It checks for dependencies, starts PostgreSQL, and
   migrates the database. It creates `planet.yaml` if missing and never
   overwrites it.
2. [Register an LFS API app](https://www.lfs.net/account/api)

- Set its callback URL to `http://localhost:5173/auth/lfs/callback`
- Turn off **Single page app**
- Replace both `REPLACE_ME` values in `planet.yaml` with the client ID and secret from
  LFS.net.

3. Run `just seed` to seed the database with vehicles, tracks and mods.
4. Run `just dev` (or `just start`)
5. Open <http://localhost:5173>

`just dev` opens tmux panes for PostgreSQL, Vite, and the API. To restart the
API automatically after Rust changes, install Bacon with
`cargo install --locked bacon` and run `just dev --watch` (or `just dev -w`).
Stop the tmux session before switching between watched and regular mode.
Detach with `Ctrl-b`, then `d`; reattach with `just dev`.
Stop the app with `tmux kill-session -t lfs-planet`; stop PostgreSQL with
`docker compose stop postgres`.

Run `just migrate` after a schema change. Run `just seed` to refresh the
catalogue and eras. PostgreSQL data persists between runs; remove it with
`docker compose down --volumes`.

Having a local copy of LFS for validation is not required.
For ranking work without the replay validator, use **Force validate** under
**Account > Hotlaps**. It accepts the upload without checking the replay.
See [replay validation](validator.md) to run the real validator.

Generate track outlines with `just track-gen ~/LFS/data/pth/*.pth`. See the
[track generator guide](../crates/lfsplanet_track_gen/README.md).

See [configuration](configuration.md) for config details and
[contributing](../CONTRIBUTING.md) for checks.

## API SDKs

After changing API routes or schemas, run `just generate-sdks` and commit the
generated source with the Rust changes. This requires Docker; see [SDK usage](../sdks/README.md).

## Demo data

After `just setup` (or `just migrate` for an existing setup), run `just demo` to
generate synthetic players and hotlaps in the configured development database.
The command seeds built-in vehicles and tracks itself, inserting any that are
missing, and creates its own demo era. You do not need to run `just seed` first.
It does not fetch vehicle mods, so generating and browsing demo data requires
neither LFS API credentials nor an LFS installation. To enable LFS sign-in or
full vehicle mod sync with `just seed`, register an LFS API app and configure
its client ID and secret in `planet.yaml` as described in the quick start above.

```sh
just demo
# Optional: change population size, combination coverage, or random seed.
cargo run -- demo --yes --players 200 --coverage 95 --seed 123
```

Each run chooses a random seed and prints it; pass `--seed` to repeat a run.
Stage progress, combination counts, and elapsed time use the configured logging
level. Long database stages report that they are still running every five seconds.

The command seeds built-in tracks and cars locally and creates a closed **Demo**
era at `/hotlaps/demo`, with one ranking over all closed track configurations and
built-in vehicles; mods are excluded. It uses the synthetic
version `0.0A`; existing era version ranges must not include that version. Normal
eras are not required or modified. Each run defaults to generating 100 player
profiles and populating 90% of the demo combinations (rounded up). Occasional drivers, regulars, and
completionists have different entry counts, with consistent relative pace and
randomized lap dates.
Every selected combination gets at least one lap. Times and splits are synthetic,
not simulations of the cars or circuits. The seed repeats profiles, participation,
and times for the same catalogue; dates are relative to the time of the run.

Players get fake usernames and display names, with varied countries. Authentication and uploads are disabled for these accounts. Laps have
`source = 'demo'`, no replay files, and no claimed validator result. The command
uses direct batched inserts, then rebuilds personal bests and badges; it does not
call an importer. Catalogue, era, player/lap inserts and personal bests commit
together. Repeated runs accumulate players and hotlaps in the existing `demo`
era, then rebuild rankings and badges across all its laps. Existing usernames
reuse their player accounts; duplicate demo laps for the same player, track,
and vehicle are skipped. With the same options and catalogue, repeating a seed
adds no new data. Run `just demo` again to add another random batch. The final
lap count reports newly inserted laps only. To start over, delete the demo era
and hotlaps or recreate your disposable development database.

The command is compiled only when `debug_assertions` is enabled, as it is for
ordinary `cargo run` builds. Normal `cargo build --release` builds omit the entire
demo command and generator. Custom profiles that enable debug assertions also
include the command. Cargo cannot make dependencies conditional on this setting,
so Cargo still builds the `fake` dependency for release builds. The schema
migration supporting demo provenance is shared by all builds.

## Troubleshooting

- **Port in use:** free port `5173`, `8000`, or `5432`.
- **Login fails:** check the OAuth credentials and callback URL in `planet.yaml`.
- **Frontend dependencies changed:** run `tmux kill-session -t lfs-planet`, then
  `just dev`.
