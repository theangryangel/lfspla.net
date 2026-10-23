use super::parse;
use time::macros::datetime;

const LEAGUES: &str = include_str!("fixtures/leagues.html.txt");
const LEAGUE: &str = include_str!("fixtures/league.html.txt");
const SEASON: &str = include_str!("fixtures/season.html.txt");
const CALENDAR: &str = include_str!("fixtures/calendar.html.txt");

#[test]
fn discovers_leagues_and_only_active_seasons() {
    let leagues = parse::leagues(LEAGUES).unwrap();
    assert!(
        leagues
            .iter()
            .any(|league| league.external_id == 432 && league.name == "Fragmaster's GTi Thursday")
    );
    let (league, seasons) = parse::league(LEAGUE, 432).unwrap();
    assert_eq!(league.name, "Fragmaster's GTi Thursday");
    assert_eq!(seasons.len(), 1);
    assert_eq!(seasons[0].external_id, 1347);
    assert_eq!(seasons[0].name, "GTi Thursday 2026");
}

#[test]
fn maps_multiple_sessions_to_one_advert_with_utc_start() {
    let (_, seasons) = parse::league(LEAGUE, 432).unwrap();
    let html = SEASON.replace("2026-10-29 19:55Z", "2026-10-29 18:00Z");
    let events = parse::season(&html, &seasons[0]).unwrap();
    assert_eq!(events.len(), 44);
    let event = events
        .iter()
        .find(|event| event.external_id == 6696)
        .unwrap();
    assert_eq!(event.title, "Round 43 - 0.7 vs 0.8 XFG TEST");
    assert_eq!(event.starts_at, datetime!(2026-10-22 18:30 UTC));
    assert_eq!(event.track_label.as_deref(), Some("South City Long Rev"));
    assert_eq!(event.cars_label, None); // XFG and a mod in separate races.
    assert_eq!(
        event.signup_url.as_deref(),
        Some("https://discord.gg/vKTyGDp")
    );
    assert!(
        event
            .description
            .as_ref()
            .unwrap()
            .contains("Be on Discord.")
    );
    let first = events
        .iter()
        .find(|event| event.external_id == 5986)
        .unwrap();
    assert_eq!(first.cars_label.as_deref(), Some("XFG"));
    // Broadcast time occurs before qualifying; it must not become the start time.
    let broadcast = events
        .iter()
        .find(|event| event.external_id == 6699)
        .unwrap();
    assert_eq!(broadcast.starts_at, datetime!(2026-10-29 19:30 UTC));
}

#[test]
fn parses_calendar_destinations_without_executing_javascript() {
    let calendar = parse::calendar(CALENDAR).unwrap();
    assert_eq!(
        calendar[&(432, 1347, 6696)],
        datetime!(2026-10-22 18:30 UTC)
    );
    assert!(parse::calendar(&CALENDAR.replace("location.href='", "evil('")).is_err());
    assert!(parse::calendar(&CALENDAR.replace("2026-10-22 18:30Z", "2026-10-22 18:30")).is_err());
    // An actual calendar can legitimately have no events.
    assert!(
        parse::calendar("<table class=calendarBig></table>")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn rejects_changed_structure_dates_and_parent_identity() {
    let (_, seasons) = parse::league(LEAGUE, 432).unwrap();
    assert!(parse::leagues("<html>Please log in</html>").is_err());
    assert!(parse::leagues("<table id=leaguesTableBody></table>").is_err());
    assert!(parse::league("<div id=BodyDiv><h1>Log in</h1></div>", 432).is_err());
    assert!(parse::season(&SEASON.replace("Start Time", "Starts"), &seasons[0]).is_err());
    assert!(parse::season(&SEASON.replace("2026-01-01 19:30Z", "Today"), &seasons[0]).is_err());
    assert!(
        parse::season(
            &SEASON.replace(
                "/leagues/432/season/1347/round/5986",
                "/leagues/999/season/1347/round/5986"
            ),
            &seasons[0]
        )
        .is_err()
    );
    assert!(
        parse::season(
            &SEASON.replace("https://discord.gg/vKTyGDp", "javascript:alert(1)"),
            &seasons[0]
        )
        .is_err()
    );
}

#[test]
fn source_traversal_is_restricted_to_expected_lfs_paths() {
    for path in [
        "https://evil.example/leagues/432",
        "//evil.example/leagues/432",
        "https://www.lfs.net@evil.example/leagues/432",
        "https://user@www.lfs.net/leagues/432",
        "/account",
        "/leagues/add",
        "/leagues/432?ajax=1",
        "/leagues/0",
        "/leagues/432/season/1/round/../../../../account",
    ] {
        assert!(parse::source_url(path).is_err(), "accepted {path}");
    }
    for path in [
        "/leagues",
        "/leagues/calendar",
        "/leagues/432-Fragmaster%27s-GTi-Thursday",
        "/leagues/432/season/1347#round43",
        "/leagues/432/season/1347/round/6696",
    ] {
        assert!(parse::source_url(path).is_ok(), "rejected {path}");
    }
}

#[test]
fn calendar_history_does_not_require_an_active_season_and_advertised_starts_win() {
    let mut calendar = parse::calendar(CALENDAR).unwrap();
    // RandomCar's final round was on Tuesday; its season is now inactive.
    super::reconcile(
        &mut Vec::new(),
        &calendar,
        695,
        datetime!(2026-10-22 12:00 UTC),
    )
    .unwrap();
    // Missing an upcoming advertised round must still fail the league snapshot.
    assert!(
        super::reconcile(
            &mut Vec::new(),
            &calendar,
            432,
            datetime!(2026-10-22 12:00 UTC)
        )
        .is_err()
    );

    let (_, seasons) = parse::league(LEAGUE, 432).unwrap();
    let mut events = parse::season(SEASON, &seasons[0]).unwrap();
    calendar.insert((432, 1347, 6696), datetime!(2026-10-22 18:00 UTC));
    super::reconcile(&mut events, &calendar, 432, datetime!(2026-10-22 12:00 UTC)).unwrap();
    assert_eq!(events.len(), 44);
    assert_eq!(
        events
            .iter()
            .find(|event| event.external_id == 6696)
            .unwrap()
            .starts_at,
        datetime!(2026-10-22 18:00 UTC)
    );
}

#[test]
fn league_logos_use_the_dedicated_element_and_logo_endpoint() {
    let (league, _) = parse::league(LEAGUE, 432).unwrap();
    assert_eq!(
        league.logo_url.as_deref(),
        Some("/dynamic/leaguelogos/Mo0gaGOM.png")
    );
    let missing = LEAGUE.replace("class=\"leagueLogo\"", "class=\"otherImage\"");
    assert!(parse::league(&missing, 432).unwrap().0.logo_url.is_none());
    for path in [
        "https://evil.example/dynamic/leaguelogos/a.png",
        "/mp/http://example.com/logo.png",
        "/dynamic/leaguelogos/../bad.png",
        "/dynamic/leaguelogos/a.png?url=evil",
        "https://user@www.lfs.net/dynamic/leaguelogos/a.png",
        "/dynamic/leaguelogos/sub/a.png",
    ] {
        assert!(super::fetch::logo_url(path).is_err(), "accepted {path}");
    }
    assert_eq!(
        super::fetch::logo_url(league.logo_url.as_deref().unwrap())
            .unwrap()
            .as_str(),
        "https://www.lfs.net/dynamic/leaguelogos/Mo0gaGOM.png"
    );
    let png = b"\x89PNG\r\n\x1a\n".to_vec();
    assert_eq!(
        super::fetch::decode_logo(png.clone(), "image/png")
            .unwrap()
            .extension,
        "png"
    );
    assert!(super::fetch::decode_logo(png, "image/jpeg").is_err());
    assert!(super::fetch::decode_logo(b"<html>missing</html>".to_vec(), "image/png").is_err());
    assert!(super::fetch::decode_logo(b"<svg></svg>".to_vec(), "image/svg+xml").is_err());
}
