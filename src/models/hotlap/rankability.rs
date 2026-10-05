use insim_core::{track::Track, vehicle::Vehicle};

/// A canonical identifier cannot participate in a ranked hotlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("identifier cannot participate in a ranked hotlap")]
pub struct NotHotlapRankable;

/// Whether a canonical LFS identifier can participate in a ranked hotlap.
pub trait HotlapRankable: Sized {
    /// Whether this value can participate in a ranked hotlap.
    fn is_hotlap_rankable(&self) -> bool;

    /// Returns this value when it can participate in a ranked hotlap.
    fn ensure_hotlap_rankable(self) -> Result<Self, NotHotlapRankable> {
        if self.is_hotlap_rankable() {
            Ok(self)
        } else {
            Err(NotHotlapRankable)
        }
    }
}

impl HotlapRankable for Track {
    fn is_hotlap_rankable(&self) -> bool {
        !self.is_open()
    }
}

impl HotlapRankable for Vehicle {
    fn is_hotlap_rankable(&self) -> bool {
        *self != Vehicle::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_rankable_vehicles() {
        assert!(Vehicle::Xfg.is_hotlap_rankable());
        assert!("728419".parse::<Vehicle>().unwrap().is_hotlap_rankable());
        assert!(!Vehicle::Unknown.is_hotlap_rankable());
    }

    #[test]
    fn identifies_rankable_tracks() {
        assert!(Track::Bl1r.is_hotlap_rankable());
        assert!(!Track::Bl1x.is_hotlap_rankable());
    }

    #[test]
    fn ensures_rankable_identifiers() {
        assert_eq!(Vehicle::Xfg.ensure_hotlap_rankable(), Ok(Vehicle::Xfg));
        assert_eq!(
            Vehicle::Unknown.ensure_hotlap_rankable(),
            Err(NotHotlapRankable)
        );
    }
}
