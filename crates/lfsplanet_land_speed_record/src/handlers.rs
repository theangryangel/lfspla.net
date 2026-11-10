use super::{
    events::LandSpeedRecordResult,
    state::{LandSpeedRecord, PendingCrossing},
    ui::{LandSpeedRecordConnectionProps, LandSpeedRecordUi},
};
use insim::{
    Colour,
    core::object::{
        ObjectInfo,
        insim::{InsimCheckpoint, InsimCheckpointKind},
    },
    identifiers::ConnectionId,
    insim::{Mci, PlayerType, Uco, UcoAction},
};
use kitcar::{
    AppError, Connected, Disconnected, Event, Packet, PlayerLeft, PlayerTeleportedToPits, Sender,
    SessionEnded, SessionStarted, State, TakingOver, World, mtc,
};
use lfsplanet_game::{GamePhase, SetupStarted};
use std::time::Instant;

async fn refresh_ui(
    state: &State<LandSpeedRecord>,
    ui: &LandSpeedRecordUi,
) -> Result<(), AppError> {
    let leaderboard = state.read().leaderboard.clone();
    let standings = leaderboard.standings().await.map_err(AppError::Handler)?;
    ui.assign_global(super::state::LandSpeedRecordGlobal {
        phase: state.read().phase.to_string(),
        leaderboard: standings,
    });
    Ok(())
}

pub(super) async fn on_setup_started(
    _: Event<SetupStarted>,
    state: State<LandSpeedRecord>,
    ui: LandSpeedRecordUi,
) -> Result<(), AppError> {
    {
        let mut s = state.write();
        s.pending.clear();
        s.phase = GamePhase::SettingUp;
    }
    refresh_ui(&state, &ui).await
}
pub(super) async fn on_connected(
    Event(Connected(info)): Event<Connected>,
    state: State<LandSpeedRecord>,
    ui: LandSpeedRecordUi,
) -> Result<(), AppError> {
    let _ = ui
        .assign_player(
            info.ucid,
            LandSpeedRecordConnectionProps { uname: info.uname },
        )
        .await;
    refresh_ui(&state, &ui).await
}
pub(super) async fn on_disconnected(
    Event(disconnected): Event<Disconnected>,
    state: State<LandSpeedRecord>,
    ui: LandSpeedRecordUi,
    world: World,
    sender: Sender,
) -> Result<(), AppError> {
    state
        .write()
        .pending
        .retain(|_, crossing| crossing.ucid != disconnected.ucid);
    if world.client_count() == 0 {
        let was_racing = state.read().phase == GamePhase::Racing;
        {
            let mut s = state.write();
            s.pending.clear();
            s.phase = GamePhase::Waiting;
        }
        if was_racing {
            sender.packet(world.end())?;
        }
    }
    refresh_ui(&state, &ui).await
}
pub(super) async fn on_session_started(
    _: Event<SessionStarted>,
    state: State<LandSpeedRecord>,
    sender: Sender,
    ui: LandSpeedRecordUi,
) -> Result<(), AppError> {
    if state.read().phase != GamePhase::SettingUp {
        return Ok(());
    }
    state.write().phase = GamePhase::Racing;
    sender.packets(mtc(
        "Land Speed Record - cross checkpoint 1 as fast as you can! Slipstreaming is allowed.",
        Some(ConnectionId::ALL),
    ))?;
    refresh_ui(&state, &ui).await
}
pub(super) async fn on_session_ended(
    _: Event<SessionEnded>,
    state: State<LandSpeedRecord>,
    ui: LandSpeedRecordUi,
) -> Result<(), AppError> {
    {
        let mut s = state.write();
        s.pending.clear();
        s.phase = GamePhase::Waiting;
    }
    refresh_ui(&state, &ui).await
}
pub(super) async fn on_toc(
    Event(TakingOver { after, .. }): Event<TakingOver>,
    state: State<LandSpeedRecord>,
) -> Result<(), AppError> {
    let _ = state.write().pending.remove(&after.plid);
    Ok(())
}
pub(super) async fn on_player_left(
    Event(PlayerLeft(player)): Event<PlayerLeft>,
    state: State<LandSpeedRecord>,
) -> Result<(), AppError> {
    let _ = state.write().pending.remove(&player.plid);
    Ok(())
}
pub(super) async fn on_player_teleported_to_pits(
    Event(PlayerTeleportedToPits(player)): Event<PlayerTeleportedToPits>,
    state: State<LandSpeedRecord>,
) -> Result<(), AppError> {
    let _ = state.write().pending.remove(&player.plid);
    Ok(())
}

fn is_speed_trap(uco: &Uco) -> bool {
    matches!(uco.ucoaction, UcoAction::CpFwd)
        && matches!(
            uco.info,
            ObjectInfo::InsimCheckpoint(InsimCheckpoint {
                kind: InsimCheckpointKind::Checkpoint1,
                ..
            })
        )
}

pub(super) async fn on_uco(
    Packet(uco): Packet<Uco>,
    state: State<LandSpeedRecord>,
    world: World,
) -> Result<(), AppError> {
    if !is_speed_trap(&uco) {
        return Ok(());
    }
    let Some(player) = world.player(uco.plid) else {
        return Ok(());
    };
    if player.ptype.contains(PlayerType::AI) {
        return Ok(());
    }
    let Some(connection) = world.connection(player.ucid) else {
        return Ok(());
    };
    if connection.uname.trim().is_empty() {
        return Ok(());
    }
    let _ = state.write().pending.insert(
        uco.plid,
        PendingCrossing {
            ucid: player.ucid,
            uname: connection.uname,
            crossed_at: Instant::now(),
        },
    );
    Ok(())
}

pub(super) async fn on_mci(
    Packet(mci): Packet<Mci>,
    state: State<LandSpeedRecord>,
    sender: Sender,
    ui: LandSpeedRecordUi,
    world: World,
) -> Result<(), AppError> {
    let mut scored = false;
    for car in mci.info {
        let crossing = state.write().take_crossing(car.plid, Instant::now());
        let Some(crossing) = crossing else {
            continue;
        };
        let Some(player) = world.player(car.plid) else {
            continue;
        };
        let Some(connection) = world.connection(player.ucid) else {
            continue;
        };
        if player.ucid != crossing.ucid
            || !connection.uname.eq_ignore_ascii_case(&crossing.uname)
            || player.ptype.contains(PlayerType::AI)
        {
            continue;
        }
        let speed_kph = f64::from(car.speed.to_metres_per_sec()) * 3.6;
        if speed_kph <= 0.0 {
            continue;
        }
        let leaderboard = state.read().leaderboard.clone();
        let previous = leaderboard
            .standings()
            .await
            .map_err(AppError::Handler)?
            .into_iter()
            .find(|entry| entry.uname.eq_ignore_ascii_case(&crossing.uname));
        let is_pb = previous
            .as_ref()
            .is_none_or(|entry| speed_kph > entry.speed_kph);
        leaderboard
            .record(LandSpeedRecordResult {
                uname: crossing.uname,
                pname: player.pname.clone(),
                vehicle: player.vehicle.to_string(),
                speed_kph,
            })
            .await
            .map_err(AppError::Handler)?;
        let message = if is_pb {
            format!("New PB! {speed_kph:.2} km/h").light_green()
        } else {
            format!(
                "Speed: {speed_kph:.2} km/h | PB: {:.2} km/h",
                previous.unwrap().speed_kph
            )
            .yellow()
        };
        sender.packets(mtc(message, Some(player.ucid)))?;
        scored = true;
    }
    if scored {
        refresh_ui(&state, &ui).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_forward_checkpoint_one_crossings_trigger_a_measurement() {
        let mut uco = Uco {
            reqi: Default::default(),
            plid: Default::default(),
            ucoaction: UcoAction::CpFwd,
            time: Default::default(),
            c: Default::default(),
            info: ObjectInfo::InsimCheckpoint(InsimCheckpoint {
                kind: InsimCheckpointKind::Checkpoint1,
                ..Default::default()
            }),
        };
        assert!(is_speed_trap(&uco));
        uco.ucoaction = UcoAction::CpRev;
        assert!(!is_speed_trap(&uco));
        uco.ucoaction = UcoAction::CpFwd;
        uco.info = ObjectInfo::InsimCheckpoint(InsimCheckpoint {
            kind: InsimCheckpointKind::Finish,
            ..Default::default()
        });
        assert!(!is_speed_trap(&uco));
    }
}
