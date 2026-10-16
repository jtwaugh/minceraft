use crate::fish::*;
use crate::habitat::Habitats;

const TEST_FISH: &str = r#"
[[species]]
id = "any"
weight = 1

[[species]]
id = "cod"
weight = 5
habitats = ["cold_open", "deep_water"]
temp_max = 0.4
depth_min = 3
depth_max = 20
"#;

fn fish() -> (Fish, Habitats) {
    let h = Habitats::shipped();
    (Fish::from_toml(TEST_FISH, &h).unwrap(), h)
}

fn col(h: &Habitats) -> FishColumn {
    FishColumn {
        habitat: h.id_of("cold_open").unwrap(),
        temperature: 0.3,
        depth: 10,
    }
}

fn ids(f: &Fish, c: &FishColumn) -> Vec<String> {
    f.species_for(c).iter().map(|s| s.0.to_string()).collect()
}

#[test]
fn matching_column_returns_species_with_weights() {
    let (f, h) = fish();
    assert_eq!(f.species_for(&col(&h)), [("any", 1), ("cod", 5)]);
}

#[test]
fn habitat_filter() {
    let (f, h) = fish();
    let c = FishColumn {
        habitat: h.id_of("warm_shallows").unwrap(),
        ..col(&h)
    };
    assert_eq!(ids(&f, &c), ["any"]);
    let c = FishColumn {
        habitat: h.id_of("deep_water").unwrap(),
        ..col(&h)
    };
    assert_eq!(ids(&f, &c), ["any", "cod"]);
}

#[test]
fn temperature_filter() {
    let (f, h) = fish();
    let edge = FishColumn {
        temperature: 0.4,
        ..col(&h)
    };
    assert_eq!(ids(&f, &edge), ["any", "cod"]);
    let hot = FishColumn {
        temperature: 0.41,
        ..col(&h)
    };
    assert_eq!(ids(&f, &hot), ["any"]);
}

#[test]
fn depth_filter() {
    let (f, h) = fish();
    for d in [3, 20] {
        let c = FishColumn {
            depth: d,
            ..col(&h)
        };
        assert_eq!(ids(&f, &c), ["any", "cod"]);
    }
    for d in [2, 21] {
        let c = FishColumn {
            depth: d,
            ..col(&h)
        };
        assert_eq!(ids(&f, &c), ["any"]);
    }
}

#[test]
fn no_fit_is_empty() {
    let h = Habitats::shipped();
    let f = Fish::from_toml(
        &TEST_FISH.replace(
            "id = \"any\"\nweight = 1",
            "id = \"any\"\nweight = 1\ndepth_min = 50",
        ),
        &h,
    )
    .unwrap();
    let c = FishColumn {
        depth: 10,
        temperature: 0.9,
        habitat: h.id_of("warm_shallows").unwrap(),
    };
    assert!(f.species_for(&c).is_empty());
}

#[test]
fn shipped_file_loads_with_no_species() {
    let f = Fish::shipped();
    assert!(f.species.is_empty());
    let h = Habitats::shipped();
    assert!(f.species_for(&col(&h)).is_empty());
}

#[test]
fn bad_files_are_rejected() {
    let h = Habitats::shipped();
    let bad = [
        "[[species]]\nid=\"a\"\nweight=1\n[[species]]\nid=\"a\"\nweight=1",
        "[[species]]\nid=\"a\"\nweight=0",
        "[[species]]\nid=\"a\"\nweight=1\nhabitats=[\"nope\"]",
        "[[species]]\nid=\"a\"\nweight=1\ntemp_min=0.9\ntemp_max=0.1",
        "[[species]]\nid=\"a\"\nweight=1\ndepth_min=9\ndepth_max=1",
    ];
    for t in bad {
        assert!(Fish::from_toml(t, &h).is_err(), "{t}");
    }
}
