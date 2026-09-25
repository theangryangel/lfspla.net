use super::*;
use definition::CombinationDefinition;

fn pair(track: &str, vehicle: &str) -> CombinationDefinition {
    CombinationDefinition {
        track: track.parse().unwrap(),
        vehicle: vehicle.parse().unwrap(),
    }
}

#[test]
fn definitions_admit_pairs_not_the_product_of_their_axes() {
    let mut era = definition::test_definitions().remove(0);
    era.rankings[0].combinations = vec![pair("BL2", "XFG"), pair("FE1", "MRT")].into();
    assert!(era.allows_combination("BL2".parse().unwrap(), "XFG".parse().unwrap()));
    assert!(!era.allows_combination("BL2".parse().unwrap(), "MRT".parse().unwrap()));
    assert!(!era.allows_combination("FE1".parse().unwrap(), "XFG".parse().unwrap()));
}

#[test]
fn bundled_definitions_validate_and_round_trip_without_policies() {
    let paths = ["2005-06-24", "2006-04-21", "2007-12-21", "2026-08-26"]
        .map(|id| Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("assets/eras/{id}.yaml")));
    for era in read_desired(&paths).unwrap() {
        let yaml = serde_saphyr::to_string(&era).unwrap();
        let exported: EraDefinition = serde_saphyr::from_str(&yaml).unwrap();
        assert_eq!(era, exported);
        assert!(!yaml.contains("\ntracks:"));
        assert!(!yaml.contains("\nvehicles:"));
        assert!(
            !era.allows_combination("BL3".parse().unwrap(), "MRT".parse().unwrap()),
            "{}",
            era.id
        );
    }
}
