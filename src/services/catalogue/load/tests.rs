use super::*;
use crate::models::era::definition;
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
    for document in files(&paths).unwrap() {
        let CatalogueDefinition::Era(era) = document.definition else {
            panic!("bundled era changed kind")
        };
        let yaml = serde_saphyr::to_string(&CatalogueDefinition::Era(era.clone())).unwrap();
        let CatalogueDefinition::Era(exported) = serde_saphyr::from_str(&yaml).unwrap() else {
            panic!("era round trip changed kind")
        };
        assert!(yaml.starts_with("kind: Era\n"));
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

fn write_definition(directory: &Path, name: &str, input: &str) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, input).unwrap();
    path
}

#[test]
fn seed_requires_an_explicit_kind() {
    let directory = tempfile::tempdir().unwrap();
    let era = definition::test_definitions().remove(0);
    let yaml = serde_saphyr::to_string(&era).unwrap();
    let path = write_definition(directory.path(), "era.yaml", &yaml);
    let error = files(&[path])
        .err()
        .expect("seed accepted an untagged document");
    let message = format!("{error:#}");
    assert!(message.contains("kind"), "{message}");
    assert!(message.contains("era.yaml"), "{message}");
}

#[test]
fn rejects_unknown_kinds_fields_and_duplicate_eras_with_source_context() {
    let directory = tempfile::tempdir().unwrap();
    let era = definition::test_definitions().remove(0);
    let yaml = serde_saphyr::to_string(&CatalogueDefinition::Era(era)).unwrap();
    for input in [
        yaml.replace("kind: Era", "kind: Unknown"),
        format!("{yaml}\nunrecognised: true\n"),
    ] {
        let path = write_definition(directory.path(), "invalid.yaml", &input);
        let error = files(&[path]).err().expect("invalid input was accepted");
        assert!(format!("{error:#}").contains("invalid.yaml"));
    }
    let first = write_definition(directory.path(), "first.yaml", &yaml);
    let second = write_definition(directory.path(), "second.yaml", &yaml);
    let error = files(&[first, second])
        .err()
        .expect("duplicate era was accepted");
    assert!(error.to_string().contains("duplicate era"));
    assert!(error.to_string().contains("second.yaml"));
}

#[test]
fn bundled_track_artwork_is_explicit_and_readable() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/tracks");
    let paths = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yaml")
        })
        .collect::<Vec<_>>();
    let documents = files(&paths).unwrap();
    let outlines = std::fs::read_dir(&directory)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|extension| extension == "svg")
        })
        .count();
    let tracks: usize = documents
        .iter()
        .map(|document| match &document.definition {
            CatalogueDefinition::Location(location) => location
                .tracks
                .iter()
                .filter(|track| track.outline.is_some())
                .count(),
            _ => panic!("bundled track document changed kind"),
        })
        .sum();
    assert_eq!(tracks, outlines);
    for track in insim_core::track::Track::ALL {
        assert!(documents.iter().any(|document| matches!(&document.definition,
            CatalogueDefinition::Location(location) if location.tracks.iter().any(|definition| definition.id == *track))),
            "canonical track {track} needs an explicit bundled definition");
    }
    assert!(!documents.is_empty());
}

#[test]
fn track_assets_resolve_relative_to_the_definition_and_validate_before_apply() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("images")).unwrap();
    std::fs::write(directory.path().join("images/BL1.svg"), "<svg/>").unwrap();
    let path = write_definition(
        directory.path(),
        "track.yaml",
        "kind: Location\nid: BL\nname: Blackwood\ntracks:\n  - id: BL1\n    outline: ./images/BL1.svg\n",
    );
    assert!(files(std::slice::from_ref(&path)).is_ok());
    let path = write_definition(
        directory.path(),
        "track.yaml",
        "kind: Location\nid: BL\nname: Blackwood\ntracks:\n  - id: BL1\n    image: ./images/missing.webp\n",
    );
    let error = files(&[path]).err().unwrap();
    let message = format!("{error:#}");
    assert!(message.contains("missing.webp"));
    assert!(message.contains("track.yaml"));
}

#[test]
fn bundled_standard_vehicle_artwork_is_complete_and_readable() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/vehicles");
    let paths = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yaml")
        })
        .collect::<Vec<_>>();
    let documents = files(&paths).unwrap();
    let expected = crate::models::vehicle::STANDARD_VEHICLES;
    assert_eq!(documents.len(), expected.len());
    for (id, _) in expected {
        assert!(documents.iter().any(|document| matches!(&document.definition, CatalogueDefinition::Vehicle(vehicle) if vehicle.id == *id && vehicle.image.is_some())));
    }
}

#[test]
fn vehicle_input_rejects_unknown_and_mod_ids_and_missing_images() {
    let directory = tempfile::tempdir().unwrap();
    for id in ["not-a-vehicle", "ABCDEF"] {
        let path = write_definition(
            directory.path(),
            "vehicle.yaml",
            &format!("kind: Vehicle\nid: {id}\n"),
        );
        let error = files(&[path]).err().unwrap();
        assert!(format!("{error:#}").contains("vehicle.yaml"));
    }
    let path = write_definition(
        directory.path(),
        "vehicle.yaml",
        "kind: Vehicle\nid: XFG\nimage: ./missing.png\n",
    );
    assert!(format!("{:#}", files(&[path]).err().unwrap()).contains("missing.png"));
    let first = write_definition(directory.path(), "first.yaml", "kind: Vehicle\nid: XFG\n");
    let second = write_definition(directory.path(), "second.yaml", "kind: Vehicle\nid: XFG\n");
    assert!(
        files(&[first, second])
            .err()
            .unwrap()
            .to_string()
            .contains("duplicate vehicle")
    );
}

#[test]
fn seed_rejects_stdin_without_reading_it() {
    let error = files(&[PathBuf::from("-")])
        .err()
        .expect("stdin was accepted");
    assert!(error.to_string().contains("stdin is not supported"));
}

#[test]
fn locations_reject_standalone_tracks_unknown_ids_and_duplicate_tracks() {
    let directory = tempfile::tempdir().unwrap();
    for input in [
        "kind: Track\nid: BL1\n",
        "kind: Location\nid: BL\nname: Blackwood\ntracks:\n  - id: UNKNOWN\n",
        "kind: Location\nid: BL\nname: Blackwood\ntracks:\n  - id: BL1\n  - id: BL1\n",
    ] {
        let path = write_definition(directory.path(), "invalid.yaml", input);
        assert!(format!("{:#}", files(&[path]).err().unwrap()).contains("invalid.yaml"));
    }
    let yaml = "kind: Location\nid: BL\nname: Blackwood\n";
    let first = write_definition(directory.path(), "first.yaml", yaml);
    let second = write_definition(directory.path(), "second.yaml", yaml);
    assert!(
        files(&[first, second])
            .err()
            .unwrap()
            .to_string()
            .contains("duplicate location")
    );
}

#[test]
fn yaml_nesting_defines_location_membership_and_tracks_remain_unique_across_locations() {
    let directory = tempfile::tempdir().unwrap();
    let first = write_definition(
        directory.path(),
        "autocross.yaml",
        "kind: Location\nid: AU\nname: Autocross\ntracks:\n  - id: AU3\n",
    );
    let second = write_definition(
        directory.path(),
        "custom.yaml",
        "kind: Location\nid: CUSTOM\nname: Custom grouping\ntracks:\n  - id: BL1\n  - id: LA1\n",
    );
    assert!(files(&[first.clone(), second.clone()]).is_ok());
    std::fs::write(
        &second,
        "kind: Location\nid: CUSTOM\nname: Custom grouping\ntracks:\n  - id: AU3\n",
    )
    .unwrap();
    let error = files(&[first, second]).err().unwrap();
    let message = format!("{error:#}");
    assert!(message.contains("duplicate track AU3"), "{message}");
    assert!(message.contains("custom.yaml"), "{message}");
}
