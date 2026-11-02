use crate::habitat::Habitats;
use crate::sea_flora::*;

const KELP: &str = r#"
[[sea_flora]]
name = "kelp"
shape = "column"
blocks = ["kelp_0", "kelp_1"]
habitats = ["cold_open"]
bottom = ["gravel"]
depth_min = 4
depth_max = 16
column = { height_min = 3, height_max = 12, surface_gap_min = 0 }
cluster_scale = 16.0
salt = 1
chance = 0.5
"#;

const OYSTER: &str = r#"
[[sea_flora]]
name = "oyster"
shape = "cluster"
blocks = ["oyster"]
habitats = ["rocky_shore"]
bottom = ["stone"]
depth_min = 1
depth_max = 5
cluster_scale = 8.0
salt = 2
chance = 0.3
"#;

const COLUMN: &str = "column = { height_min = 3, height_max = 12, surface_gap_min = 0 }\n";

fn load(text: &str) -> Result<SeaFlora, String> {
    SeaFlora::from_toml(text, &Habitats::shipped())
}

/// `KELP` with `from` replaced by `to`; panics if `from` is missing.
fn kelp_with(from: &str, to: &str) -> String {
    assert!(KELP.contains(from), "{from}");
    KELP.replace(from, to)
}

fn rejects(text: &str, needle: &str) {
    let e = load(text).unwrap_err();
    assert!(e.contains(needle), "{e}");
}

#[test]
fn shipped_file_has_kelp_oyster_and_scallop() {
    let f = SeaFlora::shipped();
    let names: Vec<&str> = f.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["kelp", "oyster", "scallop"]);
    let h = Habitats::shipped();
    let kelp = f.get("kelp").unwrap();
    assert_eq!(
        kelp.blocks,
        [
            terrain::block_id("kelp_0").unwrap(),
            terrain::block_id("kelp_1").unwrap()
        ]
    );
    assert_eq!(
        kelp.shape,
        SeaFloraShape::Column(SeaFloraColumn {
            height_min: 3,
            height_max: 12,
            surface_gap_min: 0
        })
    );
    assert_eq!((kelp.depth_min, kelp.depth_max), (4, 16));
    let oyster = f.get("oyster").unwrap();
    assert_eq!(oyster.shape, SeaFloraShape::Cluster);
    assert_eq!(oyster.shore_distance_max, Some(6));
    assert!(oyster.habitats.contains(&h.id_of("rocky_shore").unwrap()));
    let scallop = f.get("scallop").unwrap();
    assert_eq!(scallop.blocks, [terrain::block_id("scallop").unwrap()]);
    for e in &f.entries {
        for name in ["frozen", "deep_water"] {
            assert!(!e.habitats.contains(&h.id_of(name).unwrap()), "{}", e.name);
        }
    }
}

#[test]
fn test_entries_load() {
    let f = load(&format!("{KELP}{OYSTER}")).unwrap();
    assert_eq!(f.entries.len(), 2);
}

#[test]
fn unknown_block_is_rejected() {
    rejects(
        &kelp_with("\"kelp_1\"", "\"kelp_9\""),
        "sea flora 'kelp': unknown block 'kelp_9'",
    );
}

#[test]
fn unknown_habitat_is_rejected() {
    rejects(
        &kelp_with("[\"cold_open\"]", "[\"reef\"]"),
        "sea flora 'kelp': unknown habitat 'reef'",
    );
}

#[test]
fn unknown_bottom_is_rejected() {
    rejects(
        &kelp_with("[\"gravel\"]", "[\"coral\"]"),
        "sea flora 'kelp': unknown bottom block 'coral'",
    );
}

#[test]
fn empty_lists_are_rejected() {
    rejects(
        &kelp_with("[\"cold_open\"]", "[]"),
        "sea flora 'kelp': no habitat",
    );
    rejects(
        &kelp_with("[\"gravel\"]", "[]"),
        "sea flora 'kelp': no bottom block",
    );
}

#[test]
fn empty_ranges_are_rejected() {
    rejects(
        &kelp_with("depth_max = 16", "depth_max = 3"),
        "sea flora 'kelp': depth_min > depth_max",
    );
    rejects(
        &kelp_with("height_max = 12", "height_max = 2"),
        "sea flora 'kelp': height_min > height_max",
    );
}

#[test]
fn duplicate_salt_is_rejected() {
    let oyster = OYSTER.replace("salt = 2", "salt = 1");
    rejects(
        &format!("{KELP}{oyster}"),
        "sea flora 'oyster': name or salt already used by 'kelp'",
    );
}

#[test]
fn duplicate_name_is_rejected() {
    let again = KELP.replace("salt = 1", "salt = 5");
    rejects(
        &format!("{KELP}{again}"),
        "sea flora 'kelp': name or salt already used by 'kelp'",
    );
}

#[test]
fn shape_fields_must_match() {
    assert!(load(&kelp_with(", surface_gap_min = 0", "")).is_err());
    rejects(&kelp_with(COLUMN, ""), "sea flora 'kelp': shape 'column'");
    rejects(
        &kelp_with("[\"kelp_0\", \"kelp_1\"]", "[\"kelp_0\"]"),
        "sea flora 'kelp': a column takes 2 block(s)",
    );
    rejects(
        &OYSTER.replace("chance", &format!("{COLUMN}chance")),
        "sea flora 'oyster': shape 'cluster'",
    );
    rejects(
        &kelp_with("\"column\"", "\"blob\""),
        "sea flora 'kelp': shape 'blob'",
    );
}

#[test]
fn chance_and_scale_are_checked() {
    rejects(
        &kelp_with("chance = 0.5", "chance = 1.5"),
        "sea flora 'kelp': chance",
    );
    rejects(
        &kelp_with("cluster_scale = 16.0", "cluster_scale = 0.0"),
        "sea flora 'kelp': cluster_scale",
    );
}

#[test]
fn unknown_field_is_rejected() {
    assert!(load(&kelp_with("salt = 1", "salt = 1\nsalty = 2")).is_err());
}

#[test]
fn shipped_equals_bundled_file() {
    let text = include_str!("../../../data/sea_flora.toml");
    assert_eq!(SeaFlora::shipped(), load(text).unwrap());
}
