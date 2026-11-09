use std::{cmp::Reverse, time::Instant};

use insim::{
    Colour,
    core::object::{
        ObjectInfo,
        insim::{InsimCheckpoint, InsimCircle},
    },
    identifiers::ConnectionId,
    insim::{Axm, Con, Crs, Pit, PmoAction, PmoFlags, Uco, UcoAction},
};
use kitcar::{
    AppError, Connected, Disconnected, Event, Packet, PlayerLeft, PlayerTeleportedToPits, Sender,
    SessionEnded, SessionStarted, State, World, mtc,
};
use lfsplanet_game::{GamePhase, SetupStarted};

pub(super) async fn on_setup_started(
    _: Event<SetupStarted>,
    state: State<Bomb>,
    ui: BombUi,
) -> Result<(), AppError> {
    state.write().phase = GamePhase::SettingUp;
    refresh_ui(&state, &ui).await?;
    Ok(())
}

use super::{
    events::{BombResult, BombTick},
    state::{ActiveBombRun, Bomb, BombGlobal, CheckpointOutcome},
    ui::{BombConnectionProps, BombUi},
};

/// Rebuilds the global board from game state and active runs.
async fn refresh_ui(state: &State<Bomb>, ui: &BombUi) -> Result<(), AppError> {
    let (phase, leaderboard, mut active) = {
        let b = state.read();
        let active = b
            .active_runs
            .values()
            .map(|r| {
                (
                    r.uname.clone(),
                    r.pname.clone(),
                    r.checkpoints,
                    r.deadline,
                    r.current_timeout,
                )
            })
            .collect::<Vec<_>>();
        (b.phase, b.leaderboard.clone(), active)
    };
    let leaderboard = leaderboard.standings().await.map_err(AppError::Handler)?;
    active.sort_by_key(|r| Reverse(r.2));
    ui.assign_global(BombGlobal {
        phase,
        leaderboard,
        active_runs: active,
    });
    Ok(())
}

/// Updates a player's UI state.
async fn set_player(ui: &BombUi, ucid: ConnectionId, uname: String, in_run: bool) {
    let _ = ui
        .assign_player(ucid, BombConnectionProps { uname, in_run })
        .await;
}

/// Scores and announces an already-removed run.
async fn end_run(
    state: &State<Bomb>,
    ui: &BombUi,
    sender: &Sender,
    ucid: Option<ConnectionId>,
    run: &ActiveBombRun,
    message: String,
) -> Result<(), AppError> {
    let now = Instant::now();
    let survival_ms = run.survival_ms(now);
    if let Some(ucid) = ucid {
        let _ = sender.packets(mtc(message, Some(ucid)));
        set_player(ui, ucid, run.uname.clone(), false).await;
    }
    let leaderboard = state.read().leaderboard.clone();
    leaderboard
        .record(BombResult {
            uname: run.uname.clone(),
            pname: run.pname.clone(),
            vehicle: run.vehicle,
            checkpoints: run.checkpoints,
            survival_ms,
        })
        .await
        .map_err(AppError::Handler)?;
    refresh_ui(state, ui).await
}

#[derive(Clone, Copy)]
enum BombRunEnd {
    Left,
    TeleportedToPits,
    Pitted,
}

async fn remove_and_end_run(
    state: &State<Bomb>,
    ui: &BombUi,
    sender: &Sender,
    plid: insim::identifiers::PlayerId,
    ucid: Option<ConnectionId>,
    reason: BombRunEnd,
) -> Result<(), AppError> {
    let run = state.write().active_runs.remove(&plid);
    if let Some(run) = run {
        let message = match reason {
            BombRunEnd::Left => {
                format!("Run ended - left race after {} cps", run.checkpoints).red()
            }
            BombRunEnd::TeleportedToPits => format!(
                "TELE-PITTED - run ended after {} cps. No shortcuts.",
                run.checkpoints
            )
            .red(),
            BombRunEnd::Pitted => format!(
                "PITTED - run ended after {} cps. Commit to your fuel.",
                run.checkpoints
            )
            .red(),
        };
        end_run(state, ui, sender, ucid, &run, message).await?;
    }
    Ok(())
}

pub(super) async fn on_connected(
    Event(Connected(info)): Event<Connected>,
    state: State<Bomb>,
    ui: BombUi,
) -> Result<(), AppError> {
    set_player(&ui, info.ucid, info.uname.clone(), false).await;
    refresh_ui(&state, &ui).await?;
    Ok(())
}

pub(super) async fn on_disconnected(
    _: Event<Disconnected>,
    state: State<Bomb>,
    ui: BombUi,
    world: World,
    sender: Sender,
) -> Result<(), AppError> {
    if world.client_count() == 0 {
        let phase = state.read().phase;
        match phase {
            GamePhase::Racing => {
                finish_session(&state, &world, &sender, &ui, true).await?;
                sender.packet(world.end())?;
            }
            GamePhase::SettingUp => state.write().phase = GamePhase::Waiting,
            GamePhase::Waiting => {}
        }
    }
    refresh_ui(&state, &ui).await?;
    Ok(())
}

pub(super) async fn on_session_started(
    _: Event<SessionStarted>,
    state: State<Bomb>,
    sender: Sender,
    ui: BombUi,
) -> Result<(), AppError> {
    if state.read().phase != GamePhase::SettingUp {
        return Ok(());
    }
    state.write().phase = GamePhase::Racing;
    let cfg_secs = {
        let b = state.read();
        b.config.checkpoint_timeout.as_secs_f64()
    };
    sender.packets(mtc(
        format!("Bomb - hit checkpoints before the {cfg_secs:.0}s timer expires!"),
        Some(ConnectionId::ALL),
    ))?;
    refresh_ui(&state, &ui).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)] // a handler's args are all magic extractors
pub(super) async fn on_session_ended(
    _: Event<SessionEnded>,
    state: State<Bomb>,
    world: World,
    ui: BombUi,
    sender: Sender,
) -> Result<(), AppError> {
    if state.read().phase != GamePhase::Racing {
        return Ok(());
    }
    finish_session(&state, &world, &sender, &ui, false).await?;
    Ok(())
}

async fn finish_session(
    state: &State<Bomb>,
    world: &World,
    sender: &Sender,
    ui: &BombUi,
    shortfall: bool,
) -> Result<(), AppError> {
    let ended_msg = if shortfall {
        "Run ended - not enough players after"
    } else {
        "Run ended - race finished after"
    };
    let runs = std::mem::take(&mut state.write().active_runs);
    for (plid, run) in runs {
        let ucid = world.player(plid).map(|p| p.ucid);
        let message = format!("{ended_msg} {} cps", run.checkpoints).yellow();
        end_run(state, ui, sender, ucid, &run, message).await?;
    }
    state.write().phase = GamePhase::Waiting;
    if shortfall {
        let _ = sender.packets(mtc(
            "Bomb - not enough players, restarting.",
            Some(ConnectionId::ALL),
        ));
    }
    refresh_ui(state, ui).await
}

pub(super) async fn on_player_left(
    Event(PlayerLeft(player)): Event<PlayerLeft>,
    state: State<Bomb>,
    sender: Sender,
    ui: BombUi,
) -> Result<(), AppError> {
    remove_and_end_run(
        &state,
        &ui,
        &sender,
        player.plid,
        Some(player.ucid),
        BombRunEnd::Left,
    )
    .await?;
    Ok(())
}

pub(super) async fn on_player_teleported_to_pits(
    Event(PlayerTeleportedToPits(player)): Event<PlayerTeleportedToPits>,
    state: State<Bomb>,
    sender: Sender,
    ui: BombUi,
) -> Result<(), AppError> {
    remove_and_end_run(
        &state,
        &ui,
        &sender,
        player.plid,
        Some(player.ucid),
        BombRunEnd::TeleportedToPits,
    )
    .await?;
    Ok(())
}

pub(super) async fn on_pit(
    Packet(pit): Packet<Pit>,
    state: State<Bomb>,
    world: World,
    sender: Sender,
    ui: BombUi,
) -> Result<(), AppError> {
    let ucid = world.player(pit.plid).map(|p| p.ucid);
    remove_and_end_run(&state, &ui, &sender, pit.plid, ucid, BombRunEnd::Pitted).await?;
    Ok(())
}

pub(super) async fn on_crs(
    Packet(crs): Packet<Crs>,
    state: State<Bomb>,
    world: World,
    sender: Sender,
    ui: BombUi,
) -> Result<(), AppError> {
    let now = Instant::now();
    let outcome = state.write().on_reset(crs.plid, now);
    if let Some((penalty, time_left)) = outcome {
        if let Some(ucid) = world.player(crs.plid).map(|p| p.ucid) {
            sender.packets(mtc(
                format!(
                    "PENALTY -{:.2}s - {:.1}s left",
                    penalty.as_secs_f64(),
                    time_left.as_secs_f64()
                )
                .red(),
                Some(ucid),
            ))?;
        }
        refresh_ui(&state, &ui).await?;
    }
    Ok(())
}

pub(super) async fn on_con(
    Packet(con): Packet<Con>,
    state: State<Bomb>,
    world: World,
    sender: Sender,
    ui: BombUi,
) -> Result<(), AppError> {
    let now = Instant::now();
    let mps = con.spclose.to_metres_per_sec();
    let mut any_hit = false;
    for plid in [con.a.plid, con.b.plid] {
        if let Some((penalty, time_left)) = state.write().on_collision(plid, mps, now) {
            if let Some(ucid) = world.player(plid).map(|p| p.ucid) {
                sender.packets(mtc(
                    format!(
                        "PENALTY -{:.2}s - {:.1}s left",
                        penalty.as_secs_f64(),
                        time_left.as_secs_f64()
                    )
                    .red(),
                    Some(ucid),
                ))?;
            }
            any_hit = true;
        }
    }
    if any_hit {
        refresh_ui(&state, &ui).await?;
    }
    Ok(())
}

pub(super) async fn on_axm(Packet(axm): Packet<Axm>, state: State<Bomb>) -> Result<(), AppError> {
    let file_end = axm.flags.contains(PmoFlags::FILE_END);
    match axm.action {
        PmoAction::ClearAll => {
            state.write().clear_circles();
        }
        PmoAction::LoadingFile(objects)
        | PmoAction::TinyAxm(objects)
        | PmoAction::AddObjects(objects) => {
            let indices = objects.into_iter().filter_map(|o| {
                if let ObjectInfo::InsimCircle(InsimCircle { index, .. }) = o {
                    Some(index)
                } else {
                    None
                }
            });
            state.write().accumulate_circles(indices);
            if file_end {
                state.write().finalize_circles();
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) async fn on_uco(
    Packet(uco): Packet<Uco>,
    state: State<Bomb>,
    world: World,
    sender: Sender,
    ui: BombUi,
) -> Result<(), AppError> {
    let now = Instant::now();

    let outcome = match (&uco.ucoaction, &uco.info) {
        (UcoAction::CircleEnter, ObjectInfo::InsimCircle(InsimCircle { index, .. })) => {
            let Some(player) = world.player(uco.plid) else {
                return Ok(());
            };
            let uname = world
                .connection(player.ucid)
                .map(|c| c.uname)
                .unwrap_or_default();
            state.write().on_checkpoint(&uname, &player, *index, now)
        }
        (_, ObjectInfo::InsimCheckpoint(InsimCheckpoint { .. })) => {
            state.write().on_time_bonus(uco.plid, now)
        }
        _ => return Ok(()),
    };
    let Some(outcome) = outcome else {
        return Ok(());
    };
    let Some(ucid) = world.player(uco.plid).map(|p| p.ucid) else {
        return Ok(());
    };

    match outcome {
        CheckpointOutcome::Started { uname } => {
            sender.packets(mtc(
                "Run started - hit every checkpoint!".light_green(),
                Some(ucid),
            ))?;
            set_player(&ui, ucid, uname, true).await;
        }
        CheckpointOutcome::Refreshed {
            checkpoints,
            new_window,
        } => {
            sender.packets(mtc(
                format!(
                    "FINISH - cp {checkpoints} - REFRESHED {:.1}s",
                    new_window.as_secs_f64()
                )
                .yellow(),
                Some(ucid),
            ))?;
        }
        CheckpointOutcome::Extended {
            checkpoints,
            time_left,
        } => {
            sender.packets(mtc(
                format!("cp {checkpoints} - {:.1}s left", time_left.as_secs_f64()).light_green(),
                Some(ucid),
            ))?;
        }
    }
    refresh_ui(&state, &ui).await?;
    Ok(())
}

pub(super) async fn on_tick(
    _: Event<BombTick>,
    state: State<Bomb>,
    world: World,
    sender: Sender,
    ui: BombUi,
) -> Result<(), AppError> {
    let now = Instant::now();
    let expired: Vec<_> = state
        .write()
        .active_runs
        .extract_if(|_, run| run.deadline < now)
        .collect();
    let nothing_expired = expired.is_empty();
    for (plid, run) in &expired {
        let ucid = world.player(*plid).map(|p| p.ucid);
        let message = format!(
            "BOOM - {} cps, {:.1}s",
            run.checkpoints,
            run.survival_ms(now) as f64 / 1000.0
        )
        .red();
        end_run(&state, &ui, &sender, ucid, run, message).await?;
    }
    // Keep active countdowns fresh between expirations.
    if nothing_expired && !state.read().active_runs.is_empty() {
        refresh_ui(&state, &ui).await?;
    }
    Ok(())
}
