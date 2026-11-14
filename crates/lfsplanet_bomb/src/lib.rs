#![allow(missing_docs, missing_debug_implementations)]
//! Persistence-agnostic Bomb mini-game.

mod chat;
mod config;
mod events;
mod handlers;
mod state;
mod ui;

use anyhow as _;
pub use config::{BombArgs, BombConfig, BombRunConfig};
use config::{PENALTY_CLEAR_DELAY, TICK_PERIOD};
use events::BombTick;
pub use events::{BombResult, BombStanding};
use handlers::{
    on_axm, on_con, on_connected, on_crs, on_disconnected, on_pit, on_player_left,
    on_player_teleported_to_pits, on_session_ended, on_session_started, on_setup_started, on_tick,
    on_uco,
};
use insim::insim::IsiFlags;
use kitcar::{App, AppError, ChatParser, HandlerExt, PenaltyClearer, State};
use lfsplanet_game::{GamePhase, SessionSetup, VehicleRestrictions};
use state::{Bomb, BombGlobal};
use tracing as _;
use tracing_subscriber as _;
use ui::BombView;

/// Bomb's selected leaderboard backend.
pub type BombLeaderboard = lfsplanet_game::Leaderboard<BombStanding, BombResult>;

/// Creates the standalone in-memory Bomb leaderboard.
pub fn in_memory_leaderboard() -> BombLeaderboard {
    std::sync::Arc::new(lfsplanet_game::InMemoryLeaderboardStore::new(
        |standings: &mut Vec<BombStanding>, result: BombResult| {
            standings.push(BombStanding {
                uname: result.uname,
                pname: result.pname,
                checkpoints: result.checkpoints,
                survival_ms: result.survival_ms,
            });
            standings.sort_by(|a, b| {
                b.checkpoints
                    .cmp(&a.checkpoints)
                    .then(b.survival_ms.cmp(&a.survival_ms))
            });
            standings.truncate(10);
        },
    ))
}

/// Runs one Bomb game against an established InSim connection.
pub async fn run_bomb_connection(
    connection: insim::net::tokio_impl::Framed,
    config: BombConfig,
    leaderboard: BombLeaderboard,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<(), AppError> {
    let setup = SessionSetup::untimed(config.track, config.layout.clone())
        .with_vehicle_rules(&config.vehicle_rules);
    let restrictions = VehicleRestrictions::new(config.vehicle_rules.clone());
    let clearer = PenaltyClearer::new(PENALTY_CLEAR_DELAY);
    let state = Bomb::new(config, leaderboard);
    let while_racing = |state: State<Bomb>| state.read().phase == GamePhase::Racing;
    let app = App::<Bomb>::with_state(state)
        .with_ui::<BombView>(BombGlobal::default())
        .handle(clearer)
        .handle(setup)
        .handle(restrictions)
        .handle(ChatParser::<chat::Cmd>::new(&['!']))
        .handle(on_connected)
        .handle(on_disconnected)
        .handle(on_axm)
        .handle(on_session_started)
        .handle(on_session_ended)
        .handle(on_setup_started)
        .handle(on_player_left)
        .handle(on_player_teleported_to_pits)
        .handle(on_pit.run_if(while_racing))
        .handle(on_crs.run_if(while_racing))
        .handle(on_con.run_if(while_racing))
        .handle(on_uco.run_if(while_racing))
        .handle(on_tick.run_if(while_racing))
        .handle(chat::handle_chat)
        .periodic(TICK_PERIOD, BombTick);
    let app_cancel = app.cancel_token().clone();
    let running = kitcar::run_connection(connection, app);
    tokio::pin!(running);
    tokio::select! {
        result = &mut running => result,
        () = cancel.cancelled() => {
            app_cancel.cancel();
            running.await
        },
    }
}

pub async fn run_bomb_with(cfg: BombRunConfig) -> Result<(), AppError> {
    let builder = insim::tcp(cfg.insim.addr)
        .isi_iname("bomb".to_string())
        .isi_prefix('!')
        .isi_admin_password(cfg.insim.admin_password)
        .isi_flags(IsiFlags::AXM_LOAD);

    let connection = builder.connect_async().await?;
    run_bomb_connection(
        connection,
        cfg.config,
        cfg.leaderboard,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
}

/// Runs Bomb standalone.
pub async fn run_bomb(args: BombArgs) -> Result<(), AppError> {
    let layout = args
        .layout
        .try_into()
        .map_err(|error| AppError::Handler(Box::new(error)))?;
    run_bomb_with(BombRunConfig {
        insim: args.insim,
        config: BombConfig {
            track: args.track,
            layout,
            ..BombConfig::default()
        },
        leaderboard: in_memory_leaderboard(),
    })
    .await
}
