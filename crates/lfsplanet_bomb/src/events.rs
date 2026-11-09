use insim::core::vehicle::Vehicle;

#[derive(Clone, Debug)]
pub(super) struct BombTick;

/// A completed Bomb run submitted to a leaderboard.
#[derive(Clone, Debug)]
pub struct BombResult {
    /// The driver's LFS username.
    pub uname: String,
    /// The driver's current display name.
    pub pname: String,
    /// The vehicle driven.
    pub vehicle: Vehicle,
    /// Checkpoints hit before the run ended.
    pub checkpoints: i64,
    /// How long the run survived, in milliseconds.
    pub survival_ms: i64,
}

/// One classified Bomb leaderboard entry.
#[derive(Clone, Debug)]
pub struct BombStanding {
    /// The driver's LFS username.
    pub uname: String,
    /// The driver's display name.
    pub pname: String,
    /// Classified checkpoint count.
    pub checkpoints: i64,
    /// Classified survival time in milliseconds.
    pub survival_ms: i64,
}
