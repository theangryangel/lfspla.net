/// A completed Metronome attempt submitted to a leaderboard.
#[derive(Clone, Debug)]
pub struct MetronomeResult {
    /// The driver's LFS username.
    pub uname: String,
    /// The driver's current display name.
    pub pname: String,
    /// Milliseconds off the target duration.
    pub delta_ms: i64,
}

/// One classified Metronome leaderboard entry.
#[derive(Clone, Debug)]
pub struct MetronomeStanding {
    /// The driver's LFS username.
    pub uname: String,
    /// The driver's display name.
    pub pname: String,
    /// Classified signed deviation from the target, in milliseconds.
    pub delta_ms: i64,
}
