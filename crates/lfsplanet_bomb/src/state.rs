use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use insim::{
    core::{
        object::{ObjectInfo, insim::InsimCircle},
        vehicle::Vehicle,
    },
    identifiers::PlayerId,
    insim::PlayerType,
};
use kitcar::PlayerInfo;
use lfsplanet_game::{GamePhase, LayoutSpec};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::config::{BombConfig, COLLISION_THRESHOLD_MPS};
use crate::BombLeaderboard;

#[derive(Clone, Debug, Default)]
pub(super) struct CircleSequence(Vec<u8>);

impl CircleSequence {
    pub(super) fn accumulate(&mut self, indices: impl Iterator<Item = u8>) {
        self.0.extend(indices);
    }

    pub(super) fn finalize(&mut self) {
        self.0.sort_unstable();
        self.0.dedup();
    }

    pub(super) fn clear(&mut self) {
        self.0.clear();
    }

    /// Returns whether `candidate` follows `last`, accepting any without a layout.
    pub(super) fn is_next(&self, last: u8, candidate: u8) -> bool {
        if self.0.is_empty() {
            return true;
        }
        self.0
            .iter()
            .position(|&i| i == last)
            .map(|pos| self.0[(pos + 1) % self.0.len()])
            == Some(candidate)
    }
}

/// An in-progress Bomb run.
#[derive(Clone, Debug)]
pub(super) struct ActiveBombRun {
    pub(super) started_at: Instant,
    pub(super) deadline: Instant,
    pub(super) current_timeout: Duration,
    pub(super) checkpoints: i64,
    pub(super) uname: String,
    pub(super) pname: String,
    pub(super) vehicle: Vehicle,
    pub(super) last_circle_index: Option<u8>,
}

impl ActiveBombRun {
    pub(super) fn new(
        uname: String,
        pname: String,
        vehicle: Vehicle,
        config: &BombConfig,
        now: Instant,
        first_circle_index: u8,
    ) -> Self {
        Self {
            started_at: now,
            deadline: now + config.checkpoint_timeout,
            current_timeout: config.checkpoint_timeout,
            checkpoints: 0,
            uname,
            pname,
            vehicle,
            last_circle_index: Some(first_circle_index),
        }
    }

    pub(super) fn survival_ms(&self, now: Instant) -> i64 {
        self.deadline
            .min(now)
            .saturating_duration_since(self.started_at)
            .as_millis() as i64
    }

    pub(super) fn time_left(&self, now: Instant) -> Duration {
        self.deadline.saturating_duration_since(now)
    }
}

#[derive(Debug)]
pub(super) enum CheckpointOutcome {
    Started {
        uname: String,
    },
    Refreshed {
        checkpoints: i64,
        new_window: Duration,
    },
    Extended {
        checkpoints: i64,
        time_left: Duration,
    },
}

#[derive(Clone, Default, Debug)]
pub(super) struct BombGlobal {
    pub(super) phase: GamePhase,
    pub(super) leaderboard: Vec<crate::BombStanding>,
    pub(super) active_runs: Vec<(String, String, i64, Instant, Duration)>,
}

pub(super) struct BombInner {
    pub(super) config: BombConfig,
    pub(super) phase: GamePhase,
    pub(super) leaderboard: BombLeaderboard,
    pub(super) circle_sequence: CircleSequence,
    pub(super) active_runs: HashMap<PlayerId, ActiveBombRun>,
}

#[derive(Clone)]
pub(super) struct Bomb {
    inner: Arc<RwLock<BombInner>>,
}

impl Bomb {
    pub(super) fn new(config: BombConfig, leaderboard: BombLeaderboard) -> Self {
        let mut circle_sequence = CircleSequence::default();
        if let Some(LayoutSpec::Objects(objects)) = &config.layout {
            circle_sequence.accumulate(objects.iter().filter_map(|object| {
                if let ObjectInfo::InsimCircle(InsimCircle { index, .. }) = object {
                    Some(*index)
                } else {
                    None
                }
            }));
            circle_sequence.finalize();
        }
        Self {
            inner: Arc::new(RwLock::new(BombInner {
                config,
                phase: GamePhase::default(),
                leaderboard,
                circle_sequence,
                active_runs: HashMap::new(),
            })),
        }
    }

    pub(super) fn read(&self) -> RwLockReadGuard<'_, BombInner> {
        self.inner.read()
    }

    pub(super) fn write(&self) -> RwLockWriteGuard<'_, BombInner> {
        self.inner.write()
    }
}

impl BombInner {
    pub(super) fn accumulate_circles(&mut self, indices: impl Iterator<Item = u8>) {
        self.circle_sequence.accumulate(indices);
    }

    pub(super) fn finalize_circles(&mut self) {
        self.circle_sequence.finalize();
    }

    pub(super) fn clear_circles(&mut self) {
        self.circle_sequence.clear();
    }

    /// Advances or starts a run when crossing a circle.
    pub(super) fn on_checkpoint(
        &mut self,
        uname: &str,
        player: &PlayerInfo,
        circle_index: u8,
        now: Instant,
    ) -> Option<CheckpointOutcome> {
        // Advance an existing valid run.
        if let Some(outcome) = self.active_runs.get_mut(&player.plid).map(|run| {
            if run.deadline < now {
                return None;
            }
            if let Some(last) = run.last_circle_index
                && !self.circle_sequence.is_next(last, circle_index)
            {
                return None;
            }
            run.last_circle_index = Some(circle_index);
            run.deadline = now + run.current_timeout;
            run.current_timeout = run
                .current_timeout
                .saturating_sub(self.config.checkpoint_penalty);
            run.checkpoints += 1;
            Some(CheckpointOutcome::Extended {
                checkpoints: run.checkpoints,
                time_left: run.time_left(now),
            })
        }) {
            return outcome;
        }

        // Only humans can start runs.
        if player.ptype.contains(PlayerType::AI) {
            return None;
        }
        let _ = self.active_runs.insert(
            player.plid,
            ActiveBombRun::new(
                uname.to_owned(),
                player.pname.clone(),
                player.vehicle,
                &self.config,
                now,
                circle_index,
            ),
        );
        Some(CheckpointOutcome::Started {
            uname: uname.to_owned(),
        })
    }

    pub(super) fn on_time_bonus(
        &mut self,
        plid: PlayerId,
        now: Instant,
    ) -> Option<CheckpointOutcome> {
        self.active_runs.get_mut(&plid).and_then(|run| {
            if run.deadline < now {
                return None;
            }
            run.current_timeout = self.config.checkpoint_timeout;
            run.deadline = now + self.config.checkpoint_timeout;
            run.checkpoints += 1;
            Some(CheckpointOutcome::Refreshed {
                checkpoints: run.checkpoints,
                new_window: self.config.checkpoint_timeout,
            })
        })
    }

    pub(super) fn on_collision(
        &mut self,
        plid: PlayerId,
        speed_diff_mps: f32,
        now: Instant,
    ) -> Option<(Duration, Duration)> {
        let fraction = (speed_diff_mps / COLLISION_THRESHOLD_MPS).clamp(0.0, 1.0);
        let penalty = Duration::from_millis(
            (fraction * self.config.collision_max_penalty.as_millis() as f32) as u64,
        );
        if penalty.is_zero() {
            return None;
        }
        self.active_runs.get_mut(&plid).map(|run| {
            run.deadline = run.deadline.checked_sub(penalty).unwrap_or(now);
            (penalty, run.time_left(now))
        })
    }

    pub(super) fn on_reset(
        &mut self,
        plid: PlayerId,
        now: Instant,
    ) -> Option<(Duration, Duration)> {
        let penalty = self.config.checkpoint_penalty;
        self.active_runs.get_mut(&plid).map(|run| {
            run.deadline = run.deadline.checked_sub(penalty).unwrap_or(now);
            (penalty, run.time_left(now))
        })
    }
}
