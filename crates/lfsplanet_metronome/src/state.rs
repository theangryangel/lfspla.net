use std::{collections::HashMap, sync::Arc, time::Duration};

use insim::identifiers::{ConnectionId, PlayerId};
use lfsplanet_game::GamePhase;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::config::MetronomeConfig;
use crate::{MetronomeLeaderboard, MetronomeStanding};

#[derive(Clone, Default, Debug)]
pub(super) struct MetronomeGlobal {
    pub(super) target: Duration,
    pub(super) leaderboard: Vec<MetronomeStanding>,
}

#[derive(Clone, Debug)]
pub(super) struct MetronomeAttempt {
    pub(super) ucid: ConnectionId,
    pub(super) uname: String,
    pub(super) started_at: Duration,
}

pub(super) struct MetronomeInner {
    pub(super) config: MetronomeConfig,
    pub(super) active_runs: HashMap<PlayerId, MetronomeAttempt>,
    pub(super) leaderboard: MetronomeLeaderboard,
    pub(super) phase: GamePhase,
}

#[derive(Clone)]
pub(super) struct Metronome {
    inner: Arc<RwLock<MetronomeInner>>,
}

impl Metronome {
    pub(super) fn new(config: MetronomeConfig, leaderboard: MetronomeLeaderboard) -> Self {
        Self {
            inner: Arc::new(RwLock::new(MetronomeInner {
                config,
                active_runs: HashMap::new(),
                leaderboard,
                phase: GamePhase::default(),
            })),
        }
    }

    pub(super) fn read(&self) -> RwLockReadGuard<'_, MetronomeInner> {
        self.inner.read()
    }

    pub(super) fn write(&self) -> RwLockWriteGuard<'_, MetronomeInner> {
        self.inner.write()
    }
}

impl MetronomeInner {}
