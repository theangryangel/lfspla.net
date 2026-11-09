# lfsplanet_game

Shared Kitcar runtime support for the LFS Planet mini-games: session setup,
vehicle restrictions, HUD components, layout loading and leaderboard interfaces.

Ported from Anomalous Motorsports. This crate has no database or web dependency.
The application supplies persistent leaderboard implementations; standalone game
runners use in-memory leaderboards. The storage helper is retained from the
original runtime; catalogue attachment integration belongs in the application.

InSim, layout parsing and Kitcar use the workspace's pinned upstream revision.
See `../lfsplanet_bomb`, `../lfsplanet_metronome` and `../lfsplanet_shortcut`.
