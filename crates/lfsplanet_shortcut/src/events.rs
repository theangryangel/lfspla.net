/// A completed Shortcut attempt submitted to a leaderboard.
#[derive(Clone, Debug)]
pub struct ShortcutResult {
    /// The driver's LFS username.
    pub uname: String,
    /// The driver's current display name.
    pub pname: String,
    /// The vehicle driven.
    pub vehicle: String,
    /// Lap time, in milliseconds.
    pub time_ms: i64,
}

/// One classified Shortcut leaderboard entry.
#[derive(Clone, Debug)]
pub struct ShortcutStanding {
    /// The driver's LFS username.
    pub uname: String,
    /// The driver's display name.
    pub pname: String,
    /// Classified lap time in milliseconds.
    pub time_ms: i64,
}
