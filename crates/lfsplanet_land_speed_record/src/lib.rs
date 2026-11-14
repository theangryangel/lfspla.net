#![allow(missing_docs, missing_debug_implementations)]
//! Persistence-agnostic Land Speed Record mini-game.

mod config;
mod events;
mod handlers;
mod state;
mod ui;

use anyhow as _;
pub use config::{LandSpeedRecordArgs, LandSpeedRecordConfig, LandSpeedRecordRunConfig};
pub use events::{LandSpeedRecordResult, LandSpeedRecordStanding};
use handlers::{
    on_connected, on_disconnected, on_mci, on_player_left, on_player_teleported_to_pits,
    on_session_ended, on_session_started, on_setup_started, on_toc, on_uco,
};
use kitcar::{App, AppError, HandlerExt, State};
use lfsplanet_game::{GamePhase, SessionSetup, VehicleRestrictions};
use state::{LandSpeedRecord, LandSpeedRecordGlobal};
use tracing as _;
use tracing_subscriber as _;
use ui::{LandSpeedRecordUi, LandSpeedRecordView};

/// LandSpeedRecord's selected leaderboard backend.
pub type LandSpeedRecordLeaderboard =
    lfsplanet_game::Leaderboard<LandSpeedRecordStanding, LandSpeedRecordResult>;

/// Creates the standalone in-memory LandSpeedRecord leaderboard.
pub fn in_memory_leaderboard() -> LandSpeedRecordLeaderboard {
    std::sync::Arc::new(lfsplanet_game::InMemoryLeaderboardStore::new(
        |standings: &mut Vec<LandSpeedRecordStanding>, result: LandSpeedRecordResult| {
            if let Some(entry) = standings
                .iter_mut()
                .find(|entry| entry.uname.eq_ignore_ascii_case(&result.uname))
            {
                if result.speed_kph > entry.speed_kph {
                    entry.pname = result.pname;
                    entry.speed_kph = result.speed_kph;
                }
            } else {
                standings.push(LandSpeedRecordStanding {
                    uname: result.uname,
                    pname: result.pname,
                    speed_kph: result.speed_kph,
                });
            }
            standings.sort_by(|a, b| b.speed_kph.total_cmp(&a.speed_kph));
        },
    ))
}

/// Runs one LandSpeedRecord game against an established InSim connection.
pub async fn run_land_speed_record_connection(
    connection: insim::net::tokio_impl::Framed,
    config: LandSpeedRecordConfig,
    leaderboard: LandSpeedRecordLeaderboard,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<(), AppError> {
    let setup = SessionSetup::untimed(config.track, config.layout.clone())
        .with_vehicle_rules(&config.vehicle_rules);
    let restrictions = VehicleRestrictions::new(config.vehicle_rules.clone());
    let while_racing = |state: State<LandSpeedRecord>, _ui: LandSpeedRecordUi| {
        state.read().phase == GamePhase::Racing
    };
    let app = App::<LandSpeedRecord>::with_state(LandSpeedRecord::new(leaderboard))
        .with_ui::<LandSpeedRecordView>(LandSpeedRecordGlobal {
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
        .handle(on_uco.run_if(while_racing))
        .handle(on_mci.run_if(while_racing));
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

pub async fn run_land_speed_record_with(cfg: LandSpeedRecordRunConfig) -> Result<(), AppError> {
    let builder = insim::tcp(cfg.insim.addr)
        .isi_iname("land_speed_record".to_string())
        .isi_prefix('!')
        .isi_flags(insim::insim::IsiFlags::MCI)
        .isi_interval(std::time::Duration::from_millis(100))
        .isi_admin_password(cfg.insim.admin_password);

    let connection = builder.connect_async().await?;
    run_land_speed_record_connection(
        connection,
        cfg.config,
        cfg.leaderboard,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
}

/// Runs LandSpeedRecord standalone.
pub async fn run_land_speed_record(args: LandSpeedRecordArgs) -> Result<(), AppError> {
    let layout = args
        .layout
        .try_into()
        .map_err(|error| AppError::Handler(Box::new(error)))?;
    run_land_speed_record_with(LandSpeedRecordRunConfig {
        insim: args.insim,
        config: LandSpeedRecordConfig {
            track: args.track,
            layout,
            vehicle_rules: Vec::new(),
        },
        leaderboard: in_memory_leaderboard(),
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn leaderboard_keeps_fastest_speed_and_stable_ties() {
        let leaderboard = in_memory_leaderboard();
        for (uname, speed_kph) in [
            ("Alpha", 200.123),
            ("Beta", 250.456),
            ("alpha", 260.789),
            ("Charlie", 260.789),
            ("Alpha", 190.0),
        ] {
            leaderboard
                .record(LandSpeedRecordResult {
                    uname: uname.into(),
                    pname: uname.into(),
                    vehicle: "XFG".into(),
                    speed_kph,
                })
                .await
                .unwrap();
        }
        let rows = leaderboard.standings().await.unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows.iter()
                .map(|row| row.uname.as_str())
                .collect::<Vec<_>>(),
            ["Alpha", "Charlie", "Beta"]
        );
        assert_eq!(rows[0].speed_kph, 260.789);
        assert_eq!(rows[0].pname, "alpha");
    }
}
