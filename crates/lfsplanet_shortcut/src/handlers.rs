use insim::{
    Colour,
    core::object::{
        ObjectInfo,
        insim::{InsimCheckpoint, InsimCheckpointKind},
    },
    identifiers::{ConnectionId, PlayerId},
    insim::{PlayerType, Uco},
};
use kitcar::{
    AppError, Connected, Disconnected, Event, Packet, PlayerLeft, PlayerTeleportedToPits, Sender,
    SessionEnded, SessionStarted, State, TakingOver, World, mtc,
};
use lfsplanet_game::{GamePhase, SetupStarted};

use super::{
    events::ShortcutResult,
    state::{Shortcut, ShortcutAttempt},
    ui::{ShortcutConnectionProps, ShortcutUi},
};

async fn standings(state: &State<Shortcut>) -> Result<Vec<crate::ShortcutStanding>, AppError> {
    let leaderboard = state.read().leaderboard.clone();
    leaderboard.standings().await.map_err(AppError::Handler)
}

async fn refresh_ui(state: &State<Shortcut>, ui: &ShortcutUi) -> Result<(), AppError> {
    let phase = state.read().phase.to_string();
    ui.assign_global(super::state::ShortcutGlobal {
        phase,
        leaderboard: standings(state).await?,
    });
    Ok(())
}

pub(super) async fn on_setup_started(
    _: Event<SetupStarted>,
    state: State<Shortcut>,
    ui: ShortcutUi,
) -> Result<(), AppError> {
    state.write().phase = GamePhase::SettingUp;
    refresh_ui(&state, &ui).await?;
    Ok(())
}

/// Abandons a player's incomplete run without scoring it.
async fn abandon_run(
    state: &State<Shortcut>,
    ui: &ShortcutUi,
    plid: PlayerId,
) -> Result<(), AppError> {
    let run = state.write().active_runs.remove(&plid);
    if let Some(run) = run {
        let best_time_ms = standings(state)
            .await?
            .iter()
            .find(|entry| entry.uname == run.uname)
            .map(|entry| entry.time_ms);
        let _ = ui
            .assign_player(
                run.ucid,
                ShortcutConnectionProps {
                    uname: run.uname,
                    in_run: false,
                    best_time_ms,
                },
            )
            .await;
        refresh_ui(state, ui).await?;
    }
    Ok(())
}

pub(super) async fn on_connected(
    Event(Connected(info)): Event<Connected>,
    ui: ShortcutUi,
) -> Result<(), AppError> {
    let _ = ui
        .assign_player(
            info.ucid,
            ShortcutConnectionProps {
                uname: info.uname.clone(),
                in_run: false,
                best_time_ms: None,
            },
        )
        .await;
    Ok(())
}

pub(super) async fn on_disconnected(
    _: Event<Disconnected>,
    state: State<Shortcut>,
    ui: ShortcutUi,
    world: World,
    sender: Sender,
) -> Result<(), AppError> {
    if world.client_count() == 0 {
        let was_racing = state.read().phase == GamePhase::Racing;
        let mut s = state.write();
        s.active_runs.clear();
        s.phase = GamePhase::Waiting;
        drop(s);
        if was_racing {
            let _ = sender.packets(mtc(
                "Shortcut - not enough players, restarting.",
                Some(ConnectionId::ALL),
            ));
            sender.packet(world.end())?;
        }
    }
    refresh_ui(&state, &ui).await?;
    Ok(())
}

pub(super) async fn on_session_started(
    _: Event<SessionStarted>,
    state: State<Shortcut>,
    sender: Sender,
    ui: ShortcutUi,
) -> Result<(), AppError> {
    if state.read().phase != GamePhase::SettingUp {
        return Ok(());
    }
    state.write().phase = GamePhase::Racing;
    sender.packets(mtc(
        "Shortcut - cross checkpoint 1 to start your timed attempt!",
        Some(ConnectionId::ALL),
    ))?;
    refresh_ui(&state, &ui).await?;
    Ok(())
}

pub(super) async fn on_session_ended(
    _: Event<SessionEnded>,
    state: State<Shortcut>,
    ui: ShortcutUi,
) -> Result<(), AppError> {
    if state.read().phase != GamePhase::Racing {
        return Ok(());
    }
    {
        let mut s = state.write();
        s.active_runs.clear();
        s.phase = GamePhase::Waiting;
    }
    refresh_ui(&state, &ui).await?;
    Ok(())
}

pub(super) async fn on_toc(
    Event(TakingOver { after, .. }): Event<TakingOver>,
    state: State<Shortcut>,
) -> Result<(), AppError> {
    if let Some(r) = state.write().active_runs.get_mut(&after.plid) {
        r.ucid = after.ucid;
    }
    Ok(())
}

pub(super) async fn on_player_left(
    Event(PlayerLeft(player)): Event<PlayerLeft>,
    state: State<Shortcut>,
    ui: ShortcutUi,
) -> Result<(), AppError> {
    abandon_run(&state, &ui, player.plid).await?;
    Ok(())
}

pub(super) async fn on_player_teleported_to_pits(
    Event(PlayerTeleportedToPits(player)): Event<PlayerTeleportedToPits>,
    state: State<Shortcut>,
    ui: ShortcutUi,
) -> Result<(), AppError> {
    abandon_run(&state, &ui, player.plid).await?;
    Ok(())
}

pub(super) async fn on_uco(
    Packet(uco): Packet<Uco>,
    state: State<Shortcut>,
    sender: Sender,
    ui: ShortcutUi,
    world: World,
) -> Result<(), AppError> {
    let ObjectInfo::InsimCheckpoint(InsimCheckpoint { kind, .. }) = uco.info else {
        return Ok(());
    };
    let is_cp1 = matches!(kind, InsimCheckpointKind::Checkpoint1);
    let is_finish = matches!(kind, InsimCheckpointKind::Finish);
    if !is_cp1 && !is_finish {
        return Ok(());
    }

    let Some(player) = world.player(uco.plid) else {
        return Ok(());
    };
    if player.ptype.contains(PlayerType::AI) {
        return Ok(());
    }
    let uname = world
        .connection(player.ucid)
        .map(|c| c.uname)
        .unwrap_or_default();

    if is_cp1 {
        if state.read().active_runs.contains_key(&uco.plid) {
            return Ok(());
        }
        let _ = state.write().active_runs.insert(
            uco.plid,
            ShortcutAttempt {
                ucid: player.ucid,
                uname: uname.clone(),
                started_at: uco.time,
            },
        );
        sender.packets(mtc(
            "Run started - reach the finish!".light_green(),
            Some(player.ucid),
        ))?;
        let _ = ui
            .assign_player(
                player.ucid,
                ShortcutConnectionProps {
                    uname: uname.clone(),
                    in_run: true,
                    best_time_ms: None,
                },
            )
            .await;
    } else if is_finish {
        let run = state.write().active_runs.remove(&uco.plid);
        if let Some(run) = run {
            let lap_time = uco.time.saturating_sub(run.started_at);
            let time_ms = lap_time.as_millis() as i64;
            let vehicle = player.vehicle.to_string();

            if let Some(pkt) = world.connection(player.ucid).map(|c| c.spec()) {
                let _ = sender.packet(pkt);
            }

            let prev_best = standings(&state)
                .await?
                .iter()
                .find(|entry| entry.uname == run.uname)
                .map(|entry| entry.time_ms);
            let is_pb = match prev_best {
                Some(prev) => time_ms < prev,
                None => true,
            };

            let leaderboard = state.read().leaderboard.clone();
            leaderboard
                .record(ShortcutResult {
                    uname: run.uname.clone(),
                    pname: player.pname.clone(),
                    vehicle: vehicle.clone(),
                    time_ms,
                })
                .await
                .map_err(AppError::Handler)?;

            let mins = time_ms / 60_000;
            let secs = (time_ms % 60_000) / 1000;
            let millis = time_ms % 1000;
            let time_str = format!("{mins}:{secs:02}.{millis:03}");

            if is_pb {
                sender.packets(mtc(
                    format!("New PB! {} ({})", time_str, vehicle).light_green(),
                    Some(player.ucid),
                ))?;
            } else if let Some(prev) = prev_best {
                let prev_mins = prev / 60_000;
                let prev_secs = (prev % 60_000) / 1000;
                let prev_millis = prev % 1000;
                sender.packets(mtc(
                    format!(
                        "Time: {} | PB: {}:{:02}.{:03}",
                        time_str, prev_mins, prev_secs, prev_millis
                    )
                    .yellow(),
                    Some(player.ucid),
                ))?;
            }

            sender.packets(mtc("Rejoin to retry".yellow(), Some(player.ucid)))?;

            let best_in_lb = standings(&state)
                .await?
                .iter()
                .find(|entry| entry.uname == run.uname)
                .map(|entry| entry.time_ms);

            let _ = ui
                .assign_player(
                    player.ucid,
                    ShortcutConnectionProps {
                        uname: run.uname,
                        in_run: false,
                        best_time_ms: best_in_lb,
                    },
                )
                .await;
        }
    }

    refresh_ui(&state, &ui).await?;
    Ok(())
}
