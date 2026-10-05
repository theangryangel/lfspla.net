use crate::milliseconds::Milliseconds;
use fake::{
    Dummy, Fake, Faker,
    faker::{internet::en::Username, name::en::Name},
};
use rand::{RngExt, seq::SliceRandom};
use std::collections::HashSet;
use time::{Duration, OffsetDateTime};

#[derive(Debug, Dummy)]
pub(super) struct Profile {
    #[dummy(faker = "Username()")]
    pub username: String,
    #[dummy(faker = "Name()")]
    pub name: String,
    /// Lap pace multiplier (thousandths).
    #[dummy(faker = "1000..1500")]
    pub pace: i64,
    #[dummy(faker = "0..100")]
    activity: u8,
}

pub(super) fn profiles(rng: &mut impl RngExt, count: u32) -> Vec<Profile> {
    let mut usernames = HashSet::new();
    let count = usize::try_from(count).expect("player count fits in usize");
    let mut profiles = Vec::with_capacity(count);
    while profiles.len() < count {
        let profile: Profile = Faker.fake_with_rng(rng);
        if usernames.insert(profile.username.to_ascii_lowercase()) {
            profiles.push(profile);
        }
    }
    profiles
}

/// Give every selected chart a driver, then fill remaining entries by activity.
pub(super) fn entries(
    rng: &mut impl RngExt,
    profile: &Profile,
    player: usize,
    players: usize,
    charts: usize,
) -> Vec<usize> {
    let target = match profile.activity {
        0..=64 => rng.random_range(1..=5),
        65..=94 => rng.random_range(6..=60),
        _ => rng.random_range(charts.div_ceil(2)..=charts),
    }
    .min(charts);
    let mut selected: Vec<_> = (player..charts).step_by(players).collect();
    let mut remaining: Vec<_> = (0..charts)
        .filter(|index| index % players != player)
        .collect();
    remaining.shuffle(rng);
    selected.extend(
        remaining
            .into_iter()
            .take(target.saturating_sub(selected.len())),
    );
    selected.sort_unstable();
    selected
}

pub(super) fn lap(
    rng: &mut impl RngExt,
    profile: &Profile,
    baseline: Milliseconds,
    now: OffsetDateTime,
) -> (Milliseconds, OffsetDateTime) {
    let milliseconds = baseline.as_millis() * (profile.pace + rng.random_range(0..100)) / 1000;
    (
        Milliseconds::from_millis(milliseconds),
        now - Duration::seconds(rng.random_range(0..31_536_000)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::StdRng};
    use std::collections::HashSet;

    #[test]
    fn participation_is_varied_unique_repeatable_and_covers_selected_charts() {
        let mut rng = StdRng::seed_from_u64(42);
        let profiles = profiles(&mut rng, 3000);
        let mut covered = HashSet::new();
        let mut counts = HashSet::new();
        for (index, profile) in profiles.iter().enumerate() {
            let charts = entries(&mut rng, profile, index, profiles.len(), 900);
            assert!(!charts.is_empty());
            assert!(charts.windows(2).all(|pair| pair[0] < pair[1]));
            counts.insert(charts.len());
            covered.extend(charts);
        }
        assert_eq!(covered.len(), 900);
        assert!(counts.len() > 30);
        let profile = &profiles[0];
        let a = entries(&mut StdRng::seed_from_u64(7), profile, 0, 3, 10);
        let b = entries(&mut StdRng::seed_from_u64(7), profile, 0, 3, 10);
        assert_eq!(a, b);
    }
}
