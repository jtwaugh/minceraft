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
            spawn_water_radius: 8.0
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
            temp_end: 0.15
        })
    );
    assert!(f.to_toml().unwrap().contains("[climate]"));
}

#[test]
fn missing_climate_loads() {
    let t = EXAMPLE.split("[climate]").next().unwrap();
    let f = LevelFile::from_toml(t).unwrap();
    assert_eq!(f.climate, None);
    assert!(!f.to_toml().unwrap().contains("climate"));
}

#[test]
fn mistyped_climate_field_is_named() {
    let t = EXAMPLE.replace("temp_end = 0.15", "temp_end = \"warm\"");
    match LevelFile::from_toml(&t) {
        Err(LevelFileError::Parse(m)) => assert!(m.contains("temp_end"), "{m}"),
        other => panic!("{other:?}"),
    }
}
