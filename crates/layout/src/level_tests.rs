use crate::level::*;

const EXAMPLE: &str = r#"
[world]
seed = 8812734419
sea_level = 64

[[level]]
id = "week1"
week = 1
anchorage = [0, 0]
island_center = [28, 6]
spawn_water_radius = 8.0

[[level]]
id = "week2"
week = 2
anchorage = [162, 41]
island_center = [180, 22]
spawn_water_radius = 8.0

[route]
points = [[0, 0], [162, 41]]

[climate]
temp_start = -0.25
temp_end = 0.15
"#;

#[test]
fn example_loads() {
    let f = LevelFile::from_toml(EXAMPLE).unwrap();
    assert_eq!(f.world.seed, 8812734419);
    assert_eq!(f.levels.len(), 2);
    assert_eq!(
        f.levels[1],
        Level {
            id: "week2".into(),
            week: 2,
            anchorage: [162, 41],
            island_center: [180, 22],
            spawn_water_radius: 8.0,
            supplies: Vec::new(),
        }
    );
}

#[test]
fn round_trip() {
    let f = LevelFile::from_toml(EXAMPLE).unwrap();
    let g = LevelFile::from_toml(&f.to_toml().unwrap()).unwrap();
    assert_eq!(f, g);
}

#[test]
fn rejects_sea_level() {
    let t = EXAMPLE.replace("sea_level = 64", "sea_level = 62");
    let e = LevelFile::from_toml(&t).unwrap_err();
    assert_eq!(e, LevelFileError::SeaLevelMismatch { found: 62 });
    assert_eq!(e.clone(), e);
    assert!(e.to_string().contains("sea_level"));
}

#[test]
fn rejects_missing_field() {
    let t = EXAMPLE.replace("week = 2\n", "");
    let e = LevelFile::from_toml(&t).unwrap_err();
    assert!(e.to_string().contains("week"), "{e}");
}

#[test]
fn rejects_duplicate_id() {
    let t = EXAMPLE.replace("week2", "week1");
    let e = LevelFile::from_toml(&t).unwrap_err();
    assert!(matches!(e, LevelFileError::DuplicateLevelId(ref s) if s == "week1"));
}

#[test]
fn route_loads_and_round_trips() {
    let f = LevelFile::from_toml(EXAMPLE).unwrap();
    assert_eq!(f.route.as_ref().unwrap().points, vec![[0, 0], [162, 41]]);
    assert!(f.to_toml().unwrap().contains("[route]"));
}

#[test]
fn no_route_is_none() {
    let t = EXAMPLE.replace("[route]\npoints = [[0, 0], [162, 41]]\n", "");
    let f = LevelFile::from_toml(&t).unwrap();
    assert_eq!(f.route, None);
    assert!(!f.to_toml().unwrap().contains("route"));
}

#[test]
fn shipped_v0_has_no_route() {
    let f = LevelFile::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/levels/v0.toml"
    ))
    .unwrap();
    assert_eq!(f.route, None);
}

#[test]
fn rejects_route_start_mismatch() {
    let t = EXAMPLE.replace("[[0, 0], [162, 41]]", "[[1, 0], [162, 41]]");
    let e = LevelFile::from_toml(&t).unwrap_err();
    assert_eq!(
        e,
        LevelFileError::RouteStartMismatch {
            found: [1, 0],
            anchorage: [0, 0]
        }
    );
    assert!(e.to_string().contains("anchorage"));
}

#[test]
fn rejects_route_point_count() {
    let t = EXAMPLE.replace("[[0, 0], [162, 41]]", "[[0, 0], [80, 25], [162, 41]]");
    let e = LevelFile::from_toml(&t).unwrap_err();
    assert_eq!(
        e,
        LevelFileError::RoutePointCount {
            points: 3,
            levels: 2
        }
    );
    assert!(e.to_string().contains("points"));
}

#[test]
fn climate_loads_and_round_trips() {
    let f = LevelFile::from_toml(EXAMPLE).unwrap();
    assert_eq!(
        f.climate,
        Some(Climate {
            temp_start: -0.25,
            temp_end: 0.15,
            sea_ice_band: true
        })
    );
    let text = f.to_toml().unwrap();
    assert!(text.contains("[climate]"));
    assert_eq!(LevelFile::from_toml(&text).unwrap(), f);
}

#[test]
fn missing_climate_loads() {
    let t = EXAMPLE.split("[climate]").next().unwrap();
    let f = LevelFile::from_toml(t).unwrap();
    assert_eq!(f.climate, None);
    assert!(!f.to_toml().unwrap().contains("climate"));
}

#[test]
fn sea_ice_band_false_round_trips() {
    let t = EXAMPLE.replace("temp_end = 0.15", "temp_end = 0.15\nsea_ice_band = false");
    let f = LevelFile::from_toml(&t).unwrap();
    assert!(!f.climate.unwrap().sea_ice_band);
    let text = f.to_toml().unwrap();
    assert!(text.contains("sea_ice_band = false"));
    assert_eq!(LevelFile::from_toml(&text).unwrap(), f);
}

#[test]
fn non_bool_sea_ice_band_is_named() {
    let t = EXAMPLE.replace("temp_end = 0.15", "temp_end = 0.15\nsea_ice_band = 1");
    match LevelFile::from_toml(&t) {
        Err(LevelFileError::Parse(m)) => assert!(m.contains("sea_ice_band"), "{m}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn mistyped_climate_field_is_named() {
    let t = EXAMPLE.replace("temp_end = 0.15", "temp_end = \"warm\"");
    match LevelFile::from_toml(&t) {
        Err(LevelFileError::Parse(m)) => assert!(m.contains("temp_end"), "{m}"),
        other => panic!("{other:?}"),
    }
}

const SUPPLIES: &str = r#"
[[level.supplies]]
item = "rice_grains"
count = 8
chest = 0

[[level.supplies]]
item = "nori"
count = 4
chest = 1
"#;

fn with_supplies(extra: &str) -> String {
    format!("{}{extra}", EXAMPLE.split("[route]").next().unwrap())
}

#[test]
fn supplies_load_and_round_trip() {
    // `[[level.supplies]]` attaches to the last `[[level]]`.
    let f = LevelFile::from_toml(&with_supplies(SUPPLIES)).unwrap();
    assert!(f.levels[0].supplies.is_empty());
    assert_eq!(
        f.levels[1].supplies[1],
        Supply {
            item: "nori".into(),
            count: 4,
            chest: 1
        }
    );
    assert_eq!(LevelFile::from_toml(&f.to_toml().unwrap()).unwrap(), f);
    assert!(!LevelFile::from_toml(EXAMPLE)
        .unwrap()
        .to_toml()
        .unwrap()
        .contains("supplies"));
}

#[test]
fn bad_supplies_name_the_field() {
    for (from, to, field) in [
        ("count = 8", "count = 0", "count"),
        ("chest = 1", "chest = 2", "chest"),
    ] {
        let e = LevelFile::from_toml(&with_supplies(&SUPPLIES.replace(from, to))).unwrap_err();
        assert!(
            matches!(e, LevelFileError::BadSupply { field: f, .. } if f == field),
            "{e:?}"
        );
        assert!(e.to_string().contains(field), "{e}");
    }
}

#[test]
fn v0_level_file_has_supplies() {
    let f = LevelFile::from_toml(include_str!("../../../data/levels/v0.toml")).unwrap();
    assert_eq!(
        f.levels[0].supplies.iter().map(|s| s.count).sum::<u32>(),
        16
    );
}
