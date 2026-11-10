/// A completed LandSpeedRecord attempt submitted to a leaderboard.
#[derive(Clone, Debug)]
pub struct LandSpeedRecordResult {
    /// The driver's LFS username.
    pub uname: String,
    /// The driver's current display name.
    pub pname: String,
    /// The vehicle driven.
    pub vehicle: String,
    /// Speed in kilometres per hour.
    pub speed_kph: f64,
}

/// One classified LandSpeedRecord leaderboard entry.
#[derive(Clone, Debug)]
pub struct LandSpeedRecordStanding {
    /// The driver's LFS username.
    pub uname: String,
    /// The driver's display name.
    pub pname: String,
    /// Best speed in kilometres per hour.
    pub speed_kph: f64,
}
