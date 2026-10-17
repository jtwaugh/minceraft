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
points = [[0, 0], [80, 25], [162, 41]]

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
