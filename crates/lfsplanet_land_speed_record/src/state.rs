use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use insim::identifiers::{ConnectionId, PlayerId};
use lfsplanet_game::GamePhase;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{LandSpeedRecordLeaderboard, LandSpeedRecordStanding};

#[derive(Clone, Default, Debug)]
pub(super) struct LandSpeedRecordGlobal {
    pub(super) phase: String,
    pub(super) leaderboard: Vec<LandSpeedRecordStanding>,
}

#[derive(Clone, Debug)]
pub(super) struct PendingCrossing {
    pub(super) ucid: ConnectionId,
    pub(super) uname: String,
    pub(super) crossed_at: Instant,
}

pub(super) struct LandSpeedRecordInner {
    pub(super) phase: GamePhase,
    pub(super) pending: HashMap<PlayerId, PendingCrossing>,
    pub(super) leaderboard: LandSpeedRecordLeaderboard,
}

#[derive(Clone)]
pub(super) struct LandSpeedRecord {
    inner: Arc<RwLock<LandSpeedRecordInner>>,
}

impl LandSpeedRecord {
    pub(super) fn new(leaderboard: LandSpeedRecordLeaderboard) -> Self {
        Self {
            inner: Arc::new(RwLock::new(LandSpeedRecordInner {
                phase: GamePhase::default(),
                pending: HashMap::new(),
                leaderboard,
            })),
        }
    }
    pub(super) fn read(&self) -> RwLockReadGuard<'_, LandSpeedRecordInner> {
        self.inner.read()
    }
    pub(super) fn write(&self) -> RwLockWriteGuard<'_, LandSpeedRecordInner> {
        self.inner.write()
    }
}

impl LandSpeedRecordInner {
    /// Consume only this player's first update after crossing, dropping stale crossings.
    pub(super) fn take_crossing(
        &mut self,
        plid: PlayerId,
        now: Instant,
    ) -> Option<PendingCrossing> {
        self.pending.remove(&plid).filter(|crossing| {
            now.saturating_duration_since(crossing.crossed_at) <= Duration::from_secs(2)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn next_update_is_consumed_once_per_player_and_expires() {
        let state = LandSpeedRecord::new(crate::in_memory_leaderboard());
        let now = Instant::now();
        for id in [1, 2] {
            let _ = state.write().pending.insert(
                PlayerId(id),
                PendingCrossing {
                    ucid: ConnectionId(id),
                    uname: format!("driver{id}"),
                    crossed_at: now,
                },
            );
        }
        assert!(state.write().take_crossing(PlayerId(3), now).is_none());
        assert_eq!(
            state.write().take_crossing(PlayerId(1), now).unwrap().uname,
            "driver1"
        );
        assert!(state.write().take_crossing(PlayerId(1), now).is_none());
        assert!(
            state
                .write()
                .take_crossing(PlayerId(2), now + Duration::from_secs(3))
                .is_none()
        );
        assert!(state.read().pending.is_empty());
    }
}
