use crate::fish::*;
use crate::habitat::{HabitatId, Habitats};

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

fn shipped_col(h: &Habitats, habitat: &str, temperature: f64, depth: u32) -> FishColumn {
    FishColumn {
        habitat: h.id_of(habitat).unwrap(),
        temperature,
        depth,
    }
}

#[test]
fn shipped_file_has_three_species() {
    let f = Fish::shipped();
    let ids: Vec<_> = f.species.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["mackerel", "salmon", "tuna"]);
    let h = Habitats::shipped();
    let text = include_str!("../../../data/fish.toml");
    assert_eq!(Fish::from_toml(text, &h).unwrap(), f);
    for s in &f.species {
        assert!(s.habitats.is_some(), "{} must list habitats", s.id);
    }
}

#[test]
fn shipped_species_cover_every_non_frozen_habitat() {
    let (f, h) = (Fish::shipped(), Habitats::shipped());
    for (habitat, temp, depth) in [
        ("warm_shallows", 0.8, 5),
        ("deep_water", 0.5, 20),
        ("rocky_shore", 0.4, 4),
        ("cold_open", 0.3, 10),
        ("temperate_open", 0.6, 10),
    ] {
        let c = shipped_col(&h, habitat, temp, depth);
        assert!(!f.species_for(&c).is_empty(), "{habitat}");
    }
}

#[test]
fn shipped_frozen_column_has_no_fish() {
    let (f, h) = (Fish::shipped(), Habitats::shipped());
    for (temp, depth) in [(0.0, 1), (0.1, 10), (0.3, 30)] {
        let c = shipped_col(&h, "frozen", temp, depth);
        assert!(f.species_for(&c).is_empty());
    }
}

#[test]
fn shipped_cold_and_warm_lists_differ() {
    let (f, h) = (Fish::shipped(), Habitats::shipped());
    let cold = f.species_for(&shipped_col(&h, "cold_open", 0.3, 10));
    let warm = f.species_for(&shipped_col(&h, "warm_shallows", 0.8, 5));
    assert_eq!(cold, [("mackerel", 10), ("salmon", 5)]);
    assert_eq!(warm, [("tuna", 2)]);
}

/// Over real habitat maps: frozen columns have no fish, and every column of
/// a habitat whose species have no depth or temperature limit has some.
#[test]
fn shipped_species_over_habitat_maps() {
    let (f, h) = (Fish::shipped(), Habitats::shipped());
    let always = ["cold_open", "temperate_open", "rocky_shore", "deep_water"].map(|n| h.id_of(n));
    let frozen = h.id_of("frozen");
    let mut water = 0;
    for seed in [1, 12345] {
        let l = crate::build_layout(
            &crate::WorldConfig { seed },
            &[crate::default_v0_level(seed)],
        );
        let m = l.habitat_map(-40, -40, 96, 96);
        for i in 0..m.habitat.len() {
            if m.habitat[i] == crate::NO_HABITAT {
                continue;
            }
            water += 1;
            let c = FishColumn {
                habitat: HabitatId(m.habitat[i] as usize),
                temperature: f64::from(m.temperature[i]),
                depth: u32::from(m.derived.water_depth[i]),
            };
            let n = f.species_for(&c).len();
            if Some(c.habitat) == frozen {
                assert_eq!(n, 0, "{c:?}");
            } else if always.contains(&Some(c.habitat)) {
                assert!(n > 0, "{c:?}");
            }
        }
    }
    assert!(water > 0);
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
