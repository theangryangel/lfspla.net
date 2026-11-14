# lfsplanet_land_speed_record

Land Speed Record is a multiplayer speed trap mini-game built on Kitcar and
`lfsplanet_game`. It has no database or web dependency; the caller supplies a
leaderboard implementation.

Place InSim checkpoint 1 in the layout as the speed trap. A forward crossing
arms that player, and their first subsequent MCI update supplies the speed.
The runner requests updates every 100ms. Each crossing scores once; stale
crossings expire after two seconds and are cleared when the driver leaves,
pits, disconnects, swaps drivers or the session changes.

Players remain on track after scoring and can retry together. Slipstreaming is
allowed. Each player's fastest speed counts, displayed in km/h to two decimal
places. Vehicle restrictions use the shared game infrastructure.

For standalone development against a local LFS server:

```sh
cargo run --locked -p lfsplanet_land_speed_record -- --help
cargo run --locked -p lfsplanet_land_speed_record -- --track BL1 --layout-file /path/to/layout.lyt
```

The standalone runner uses in-memory results. Scheduled events, catalogue assets
and persistent results are supplied by the application integration. See
[game server documentation](../../docs/game-server.md) for configuration.
