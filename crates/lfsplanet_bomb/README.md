# lfsplanet_bomb

Bomb mini-game, ported from Anomalous Motorsports with its existing
Kitcar implementation. Depends on `lfsplanet_game`, with no database or web
dependency. The library accepts a leaderboard implementation from its caller.

For standalone development against a local LFS server:

```sh
cargo run -p lfsplanet_bomb -- --help
cargo run -p lfsplanet_bomb -- --track BL1 --layout-file /path/to/layout.lyt
```

The standalone runner uses in-memory results. Scheduled events, catalogue assets
and persistent results are supplied by the application integration.
