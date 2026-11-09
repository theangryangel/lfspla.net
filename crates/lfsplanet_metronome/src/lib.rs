#![allow(missing_docs, missing_debug_implementations)]
//! Persistence-agnostic Metronome mini-game.

mod config;
mod events;
mod handlers;
mod state;
mod ui;

use std::time::Duration;

use anyhow as _;
pub use config::{MetronomeArgs, MetronomeConfig, MetronomeRunConfig};
pub use events::{MetronomeResult, MetronomeStanding};
use handlers::{
    on_connected, on_disconnected, on_player_left, on_player_teleported_to_pits, on_session_ended,
    on_session_started, on_setup_started, on_toc, on_uco,
};
use kitcar::{App, AppError, HandlerExt, State};
use lfsplanet_game::{GamePhase, SessionSetup, VehicleRestrictions};
use state::{Metronome, MetronomeGlobal};
use tracing as _;
use tracing_subscriber as _;
use ui::{MetronomeUi, MetronomeView};

/// Metronome's selected leaderboard backend.
pub type MetronomeLeaderboard = lfsplanet_game::Leaderboard<MetronomeStanding, MetronomeResult>;

/// Creates the standalone in-memory Metronome leaderboard.
pub fn in_memory_leaderboard() -> MetronomeLeaderboard {
    std::sync::Arc::new(lfsplanet_game::InMemoryLeaderboardStore::new(
        |standings: &mut Vec<MetronomeStanding>, result: MetronomeResult| {
            if let Some(entry) = standings
                .iter_mut()
                .find(|entry| entry.uname == result.uname)
            {
                if result.delta_ms.abs() < entry.delta_ms.abs() {
                    entry.pname = result.pname;
                    entry.delta_ms = result.delta_ms;
                }
            } else {
                standings.push(MetronomeStanding {
                    uname: result.uname,
                    pname: result.pname,
                    delta_ms: result.delta_ms,
                });
            }
            standings.sort_by_key(|entry| entry.delta_ms.abs());
        },
    ))
}

/// Runs one Metronome game against an established InSim connection.
pub async fn run_metronome_connection(
    connection: insim::net::tokio_impl::Framed,
    config: MetronomeConfig,
    leaderboard: MetronomeLeaderboard,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<(), AppError> {
    let target = config.target;
    let setup = SessionSetup::untimed(config.track, config.layout.clone())
        .with_vehicle_rules(&config.vehicle_rules);
    let restrictions = VehicleRestrictions::new(config.vehicle_rules.clone());
    let while_racing =
        |state: State<Metronome>, _ui: MetronomeUi| state.read().phase == GamePhase::Racing;
    let app = App::<Metronome>::with_state(Metronome::new(config, leaderboard))
        .with_ui::<MetronomeView>(MetronomeGlobal {
            target,
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

pub async fn run_metronome_with(cfg: MetronomeRunConfig) -> Result<(), AppError> {
    let builder = insim::tcp(cfg.insim.addr)
        .isi_iname("metronome".to_string())
        .isi_prefix('!')
        .isi_admin_password(cfg.insim.admin_password);

    let connection = builder.connect_async().await?;
    run_metronome_connection(
        connection,
        cfg.config,
        cfg.leaderboard,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
}

/// Runs Metronome standalone.
pub async fn run_metronome(args: MetronomeArgs) -> Result<(), AppError> {
    let layout = args
        .layout
        .try_into()
        .map_err(|error| AppError::Handler(Box::new(error)))?;
    run_metronome_with(MetronomeRunConfig {
        insim: args.insim,
        config: MetronomeConfig {
            target: Duration::from_millis(args.target_ms),
            track: args.track,
            layout,
            vehicle_rules: Vec::new(),
        },
        leaderboard: in_memory_leaderboard(),
    })
    .await
}
