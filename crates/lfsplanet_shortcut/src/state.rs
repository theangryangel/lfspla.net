use std::{collections::HashMap, sync::Arc, time::Duration};

use insim::identifiers::{ConnectionId, PlayerId};
use lfsplanet_game::GamePhase;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{ShortcutLeaderboard, ShortcutStanding};

#[derive(Clone, Default, Debug)]
pub(super) struct ShortcutGlobal {
    pub(super) phase: String,
    pub(super) leaderboard: Vec<ShortcutStanding>,
}

#[derive(Clone, Debug)]
pub(super) struct ShortcutAttempt {
    pub(super) ucid: ConnectionId,
    pub(super) uname: String,
    pub(super) started_at: Duration,
}

pub(super) struct ShortcutInner {
    pub(super) phase: GamePhase,
    pub(super) active_runs: HashMap<PlayerId, ShortcutAttempt>,
    pub(super) leaderboard: ShortcutLeaderboard,
}

#[derive(Clone)]
pub(super) struct Shortcut {
    inner: Arc<RwLock<ShortcutInner>>,
}

impl Shortcut {
    pub(super) fn new(leaderboard: ShortcutLeaderboard) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ShortcutInner {
                phase: GamePhase::default(),
                active_runs: HashMap::new(),
                leaderboard,
            })),
        }
    }

    pub(super) fn read(&self) -> RwLockReadGuard<'_, ShortcutInner> {
        self.inner.read()
    }

    pub(super) fn write(&self) -> RwLockWriteGuard<'_, ShortcutInner> {
        self.inner.write()
    }
}

impl ShortcutInner {}
