use std::time::Duration;

use insim::{
    Colour,
    core::object::{
        ObjectInfo,
        insim::{InsimCheckpoint, InsimCheckpointKind},
    },
    identifiers::{ConnectionId, PlayerId},
    insim::Uco,
};
use kitcar::{
    AppError, Connected, Disconnected, Event, Packet, PlayerLeft, PlayerTeleportedToPits, Sender,
    SessionEnded, SessionStarted, State, TakingOver, World, mtc,
};
use lfsplanet_game::{GamePhase, SetupStarted};

use super::{
    events::MetronomeResult,
    state::{Metronome, MetronomeAttempt},
    ui::{MetronomeConnectionProps, MetronomeUi},
};

async fn standings(state: &State<Metronome>) -> Result<Vec<crate::MetronomeStanding>, AppError> {
    let leaderboard = state.read().leaderboard.clone();
    leaderboard.standings().await.map_err(AppError::Handler)
}

async fn refresh_ui(state: &State<Metronome>, ui: &MetronomeUi) -> Result<(), AppError> {
    let target = state.read().config.target;
    ui.assign_global(super::state::MetronomeGlobal {
        target,
        leaderboard: standings(state).await?,
    });
    Ok(())
}

pub(super) async fn on_setup_started(
    _: Event<SetupStarted>,
    state: State<Metronome>,
    ui: MetronomeUi,
) -> Result<(), AppError> {
    state.write().phase = GamePhase::SettingUp;
    refresh_ui(&state, &ui).await?;
    Ok(())
}

/// Abandons a player's incomplete run without scoring it.
async fn abandon_run(
    state: &State<Metronome>,
    ui: &MetronomeUi,
    plid: PlayerId,
) -> Result<(), AppError> {
    let run = state.write().active_runs.remove(&plid);
    if let Some(run) = run {
        let best_delta_ms = standings(state)
            .await?
            .iter()
            .find(|entry| entry.uname == run.uname)
            .map(|entry| entry.delta_ms);
        let _ = ui
            .assign_player(
                run.ucid,
                MetronomeConnectionProps {
                    uname: run.uname,
                    in_run: false,
                    best_delta_ms,
                },
            )
            .await;
        refresh_ui(state, ui).await?;
    }
    Ok(())
}

pub(super) async fn on_connected(
    Event(Connected(info)): Event<Connected>,
    ui: MetronomeUi,
) -> Result<(), AppError> {
    let _ = ui
        .assign_player(
            info.ucid,
            MetronomeConnectionProps {
                uname: info.uname.clone(),
                in_run: false,
                best_delta_ms: None,
            },
        )
        .await;
    Ok(())
}

pub(super) async fn on_disconnected(
    _: Event<Disconnected>,
    state: State<Metronome>,
    ui: MetronomeUi,
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
                "Metronome - not enough players, restarting.",
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
    state: State<Metronome>,
    sender: Sender,
    ui: MetronomeUi,
) -> Result<(), AppError> {
    if state.read().phase != GamePhase::SettingUp {
        return Ok(());
    }
    state.write().phase = GamePhase::Racing;
    let target_secs = state.read().config.target.as_secs_f64();
    sender.packets(mtc(
        format!("Metronome - target: {target_secs:.1}s. Cross checkpoint 1 to start."),
        Some(ConnectionId::ALL),
    ))?;
    refresh_ui(&state, &ui).await?;
    Ok(())
}

pub(super) async fn on_session_ended(
    _: Event<SessionEnded>,
    state: State<Metronome>,
    ui: MetronomeUi,
) -> Result<(), AppError> {
    if state.read().phase != GamePhase::Racing {
        return Ok(());
    }
    state.write().active_runs.clear();
    state.write().phase = GamePhase::Waiting;
    refresh_ui(&state, &ui).await?;
    Ok(())
}

pub(super) async fn on_toc(
    Event(TakingOver { after, .. }): Event<TakingOver>,
    state: State<Metronome>,
) -> Result<(), AppError> {
    if let Some(r) = state.write().active_runs.get_mut(&after.plid) {
        r.ucid = after.ucid;
    }
    Ok(())
}

pub(super) async fn on_player_left(
    Event(PlayerLeft(player)): Event<PlayerLeft>,
    state: State<Metronome>,
    ui: MetronomeUi,
) -> Result<(), AppError> {
    abandon_run(&state, &ui, player.plid).await?;
    Ok(())
}

pub(super) async fn on_player_teleported_to_pits(
    Event(PlayerTeleportedToPits(player)): Event<PlayerTeleportedToPits>,
    state: State<Metronome>,
    ui: MetronomeUi,
) -> Result<(), AppError> {
    abandon_run(&state, &ui, player.plid).await?;
    Ok(())
}

pub(super) async fn on_uco(
    Packet(uco): Packet<Uco>,
    state: State<Metronome>,
    sender: Sender,
    ui: MetronomeUi,
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
    if player.ptype.contains(insim::insim::PlayerType::AI) {
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
            MetronomeAttempt {
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
                MetronomeConnectionProps {
                    uname: uname.clone(),
                    in_run: true,
                    best_delta_ms: None,
                },
            )
            .await;
    } else if is_finish {
        let run = state.write().active_runs.remove(&uco.plid);
        if let Some(run) = run {
            let elapsed = uco.time.saturating_sub(run.started_at);
            let target = state.read().config.target;
            let delta = target.abs_diff(elapsed);
            let delta_ms = elapsed.as_millis() as i64 - target.as_millis() as i64;

            if let Some(pkt) = world.connection(player.ucid).map(|c| c.spec()) {
                let _ = sender.packet(pkt);
            }

            let prev_best = standings(&state)
                .await?
                .iter()
                .find(|entry| entry.uname == run.uname)
                .map(|entry| entry.delta_ms);
            let is_pb = match prev_best {
                Some(prev) => delta_ms.abs() < prev.abs(),
                None => true,
            };

            let leaderboard = state.read().leaderboard.clone();
            leaderboard
                .record(MetronomeResult {
                    uname: run.uname.clone(),
                    pname: player.pname.clone(),
                    delta_ms,
                })
                .await
                .map_err(AppError::Handler)?;

            let tier_str = tier_label(delta)
                .map(|t| format!(" [{t}]"))
                .unwrap_or_default();

            if is_pb {
                sender.packets(mtc(
                    format!("New best! Off by: {:.3}s{}", delta.as_secs_f64(), tier_str)
                        .light_green(),
                    Some(player.ucid),
                ))?;
            } else {
                sender.packets(mtc(
                    format!("Off by: {:.3}s{}", delta.as_secs_f64(), tier_str).yellow(),
                    Some(player.ucid),
                ))?;
            }

            let best_in_lb = standings(&state)
                .await?
                .iter()
                .find(|entry| entry.uname == run.uname)
                .map(|entry| entry.delta_ms);

            let _ = ui
                .assign_player(
                    player.ucid,
                    MetronomeConnectionProps {
                        uname: run.uname,
                        in_run: false,
                        best_delta_ms: best_in_lb,
                    },
                )
                .await;
        }
    }

    refresh_ui(&state, &ui).await?;
    Ok(())
}

fn tier_label(delta: Duration) -> Option<&'static str> {
    let ms = delta.as_millis();
    if ms <= 100 {
        Some("Platinum")
    } else if ms <= 500 {
        Some("Gold")
    } else if ms <= 2000 {
        Some("Silver")
    } else if ms <= 5000 {
        Some("Bronze")
    } else {
        None
    }
}
