# Scheduled mini-games

LFS Planet can run Bomb, Metronome, Shortcut and Land Speed Record events on one LFS server.
The LFS dedicated server must already be running with InSim enabled. The
application connects to it; it does not launch or install the dedicated server.

## Configure the connection

Add to `planet.yaml` alongside the existing database/web/storage configuration:

```yaml
game_server:
  insim_address: 127.0.0.1:29999
  admin_password: replace-me
  poll_interval_seconds: 2

# Optional: avoid importing our own LFS.net advertisements.
event_import:
  ignore_league_ids: []
  ignore_title_regex: '^\[LFS Planet\]'
```

The address must be a socket address, not a hostname. The password is optional
when the LFS host allows that. Poll interval must be between 1 and 60 seconds. Keep credentials
out of catalogue definitions and version control.

## Define an event

Create an event YAML alongside its LYT file:

```yaml
kind: Event
id: shortcut-weekend-2027-01
league: LFS Planet
season: Specials 2027
title: Shortcut weekend
description: Find the fastest route through the layout. Your best attempt counts.
starts_at: "2027-01-09T12:00:00Z"
ends_at: "2027-01-11T12:00:00Z"
layout: layouts/BL1_shortcut.lyt
join_url: "https://www.lfs.net/join/your-server-name"
game:
  source:
    type: shortcut
  track: BL1
  vehicles:
    - vehicle: XFG
      added_mass_kg: 0
      intake_restriction: 0
```

Supply your actual layout file, schedule and join URL. Paths resolve relative to
the YAML. Layouts must be nonempty LFS 0.8+ LYT files, up to 2 MiB. The layout is
uploaded as an immutable attachment and loaded through InSim; it need not be
installed manually on the LFS host. An empty/omitted vehicles list is unrestricted.
Vehicle ballast is 0-200kg and intake restriction is 0-50%.

Other source settings:

```yaml
# Metronome: smallest absolute deviation from the target wins.
source:
  type: metronome
  target_ms: 30000

# Land Speed Record: highest speed through InSim checkpoint 1 wins.
source:
  type: land_speed_record

# Bomb: most checkpoints wins, then longest survival time.
source:
  type: bomb
  checkpoint_timeout_secs: 30
  checkpoint_penalty_ms: 250
  collision_max_penalty_ms: 500
```

Local leagues and seasons are created by name; imported leagues/seasons are not
modified. Catalogue IDs are stable: keep the ID when editing an event. A missing
YAML file does not delete the stored event or its results. `hidden: true` can be
used to keep an upcoming definition out of the schedule and public pages.

## Apply and run

```sh
cargo run --locked -- migrate
cargo run --locked -- seed path/to/event.yaml --dry-run
cargo run --locked -- seed path/to/event.yaml
cargo run --locked -- game-server
```

Run `game-server` independently of `web` and `worker`, using the same database
and attachment storage. One process owns the game-server advisory lock. Events
cannot overlap on the single connection. Touching end/start times are allowed;
the end is exclusive. The game process waits between events without a game
connection. It reconnects with backoff after disconnection.

After an event starts (or execution has been recorded), its schedule, gameplay,
layout and visibility cannot be changed by seeding. Title, description, grouping
and join link may still be updated. To stop a running competition operationally,
stop the game-server process. SIGINT and SIGTERM cancel the runtime and wait for
it to stop. Restarting resumes the currently scheduled competition; unfinished
attempts may be lost, while recorded results remain.

Players are identified by case-insensitive LFS usernames and do not need to have
signed into the website. Results are accepted only during the event window.
Bomb, Metronome, Shortcut and Land Speed Record retain separate result storage and classification;
ties use the earliest result, then its ID. These results do not enter validated
hotlap charts.

## Public pages and imports

The calendar and homepage link to `/events/{id}`. Imported LFS.net events redirect
to the upstream page; our competitions show rules, a join
link while live, and paginated standings. Standings refresh every ten seconds
until the event completes. The public page's Live status describes the scheduled
window, not a server health check.

Import exclusions apply to normal sync and dry-run sync. `--source-only` remains
an unfiltered source preview and intentionally does not read application config.
There is no publishing integration or identity linking with LFS.net.

## Validation limits

Database tests cover catalogue application, schedule exclusion, result ordering,
player identity, visibility and runtime ownership/cancellation. A real LFS host
is needed to check layout placement, game mechanics, reconnects and event
switching end to end. The standalone `lfsplanet_*` game runners remain available
for developing a mode independently of the application.

## Land Speed Record

Use `game.source.type: land_speed_record` with a layout containing InSim
checkpoint 1 as the speed trap. Forward crossings arm a measurement for that
player; the first subsequent MCI update for their car supplies the speed.
Telemetry is requested every 100ms. This measures just beyond the gate rather
than interpolating an exact crossing speed. Crossings expire after two seconds
and are cleared on leaving the track, pitting, disconnecting, driver swaps and
session changes.

Players can run simultaneously and slipstreaming is allowed. A crossing scores
once, without spectating the player, so the layout should provide a safe return
route for repeated attempts. Existing vehicle, ballast and intake restrictions
apply. Each player's highest speed is classified; equal speeds favour the
earliest recorded result. Speed is stored before display rounding and shown in
km/h to two decimal places on the in-game HUD and event page.

The mode also has a standalone runner:

```sh
cargo run --locked -p lfsplanet_land_speed_record -- --help
```
