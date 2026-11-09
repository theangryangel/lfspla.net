#![allow(missing_docs, missing_debug_implementations)]
//! Persistence-agnostic Shortcut mini-game.

mod config;
mod events;
mod handlers;
mod state;
mod ui;

use anyhow as _;
pub use config::{ShortcutArgs, ShortcutConfig, ShortcutRunConfig};
pub use events::{ShortcutResult, ShortcutStanding};
use handlers::{
    on_connected, on_disconnected, on_player_left, on_player_teleported_to_pits, on_session_ended,
    on_session_started, on_setup_started, on_toc, on_uco,
};
use kitcar::{App, AppError, HandlerExt, State};
use lfsplanet_game::{GamePhase, SessionSetup, VehicleRestrictions};
use state::{Shortcut, ShortcutGlobal};
use tracing as _;
use tracing_subscriber as _;
use ui::{ShortcutUi, ShortcutView};

/// Shortcut's selected leaderboard backend.
pub type ShortcutLeaderboard = lfsplanet_game::Leaderboard<ShortcutStanding, ShortcutResult>;

/// Creates the standalone in-memory Shortcut leaderboard.
pub fn in_memory_leaderboard() -> ShortcutLeaderboard {
    std::sync::Arc::new(lfsplanet_game::InMemoryLeaderboardStore::new(
        |standings: &mut Vec<ShortcutStanding>, result: ShortcutResult| {
            if let Some(entry) = standings
                .iter_mut()
                .find(|entry| entry.uname == result.uname)
            {
                if result.time_ms < entry.time_ms {
                    entry.pname = result.pname;
                    entry.time_ms = result.time_ms;
                }
            } else {
                standings.push(ShortcutStanding {
                    uname: result.uname,
                    pname: result.pname,
                    time_ms: result.time_ms,
                });
            }
            standings.sort_by_key(|entry| entry.time_ms);
        },
    ))
}

/// Runs one Shortcut game against an established InSim connection.
pub async fn run_shortcut_connection(
    connection: insim::net::tokio_impl::Framed,
    config: ShortcutConfig,
    leaderboard: ShortcutLeaderboard,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<(), AppError> {
    let setup = SessionSetup::untimed(config.track, config.layout.clone())
        .with_vehicle_rules(&config.vehicle_rules);
    let restrictions = VehicleRestrictions::new(config.vehicle_rules.clone());
    let while_racing =
        |state: State<Shortcut>, _ui: ShortcutUi| state.read().phase == GamePhase::Racing;
    let app = App::<Shortcut>::with_state(Shortcut::new(leaderboard))
        .with_ui::<ShortcutView>(ShortcutGlobal {
            phase: GamePhase::Waiting.to_string(),
            ..Default::default()
        })
        .handle(setup)
        .handle(restrictions)
        .handle(on_connected)
        .handle(on_disconnected)
        .handle(on_session_started)
        .handle(on_session_ended)
        .handle(on_setup_started)
        .handle(on_player_left.run_if(while_racing))
        .handle(on_player_teleported_to_pits.run_if(while_racing))
        .handle(on_toc.run_if(while_racing))
        .handle(on_uco.run_if(while_racing));
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

pub async fn run_shortcut_with(cfg: ShortcutRunConfig) -> Result<(), AppError> {
    let builder = insim::tcp(cfg.insim.addr)
        .isi_iname("shortcut".to_string())
        .isi_prefix('!')
        .isi_admin_password(cfg.insim.admin_password);

    let connection = builder.connect_async().await?;
    run_shortcut_connection(
        connection,
        cfg.config,
        cfg.leaderboard,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
}

/// Runs Shortcut standalone.
pub async fn run_shortcut(args: ShortcutArgs) -> Result<(), AppError> {
    let layout = args
        .layout
        .try_into()
        .map_err(|error| AppError::Handler(Box::new(error)))?;
    run_shortcut_with(ShortcutRunConfig {
        insim: args.insim,
        config: ShortcutConfig {
            track: args.track,
            layout,
            vehicle_rules: Vec::new(),
        },
        leaderboard: in_memory_leaderboard(),
    })
    .await
}
