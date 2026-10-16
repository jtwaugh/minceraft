use crate::habitat::*;
use terrain::Block;

fn inp() -> HabitatInputs {
    HabitatInputs {
        temperature: 0.5,
        depth: 10,
        bottom: Block::Dirt as u8,
        shore_distance: 10,
        frozen: false,
    }
}

fn name(i: HabitatInputs) -> String {
    let h = Habitats::shipped();
    h.name(h.classify(&i)).to_string()
}

#[test]
fn shipped_has_six_rules_in_order() {
    let ids: Vec<_> = Habitats::shipped()
        .rules
        .iter()
        .map(|r| r.id.clone())
        .collect();
    assert_eq!(
        ids,
        [
            "frozen",
            "warm_shallows",
            "deep_water",
            "rocky_shore",
            "cold_open",
            "temperate_open"
        ]
    );
}

#[test]
fn each_rule_picks_itself() {
    assert_eq!(
        name(HabitatInputs {
            frozen: true,
            ..inp()
        }),
        "frozen"
    );
    let warm = HabitatInputs {
        temperature: 0.7,
        depth: 4,
        bottom: Block::Sand as u8,
        ..inp()
    };
    assert_eq!(name(warm), "warm_shallows");
    let deep = HabitatInputs {
        depth: 15,
        shore_distance: 30,
        ..inp()
    };
    assert_eq!(name(deep), "deep_water");
    let rocky = HabitatInputs {
        depth: 5,
        shore_distance: 3,
        bottom: Block::Gravel as u8,
        ..inp()
    };
    assert_eq!(name(rocky), "rocky_shore");
    assert_eq!(
        name(HabitatInputs {
            temperature: 0.3,
            ..inp()
        }),
        "cold_open"
    );
    assert_eq!(name(inp()), "temperate_open");
}

#[test]
fn earlier_rule_wins() {
    // Matches frozen and warm_shallows.
    let both = HabitatInputs {
        frozen: true,
        temperature: 0.7,
        depth: 4,
        bottom: Block::Sand as u8,
        ..inp()
    };
    assert_eq!(name(both), "frozen");
    // Matches rocky_shore and cold_open.
    let rc = HabitatInputs {
        temperature: 0.2,
        depth: 5,
        shore_distance: 3,
        bottom: Block::Stone as u8,
        ..inp()
    };
    assert_eq!(name(rc), "rocky_shore");
}

#[test]
fn bounds_are_inclusive() {
    let edge = HabitatInputs {
        temperature: 0.6,
        depth: 6,
        bottom: Block::Sand as u8,
        ..inp()
    };
    assert_eq!(name(edge), "warm_shallows");
    assert_eq!(
        name(HabitatInputs {
            temperature: 0.4,
            ..inp()
        }),
        "cold_open"
    );
}

#[test]
fn free_function_uses_shipped_rules() {
    let h = Habitats::shipped();
    assert_eq!(classify(&inp()), h.id_of("temperate_open").unwrap());
}

#[test]
fn bad_files_are_rejected() {
    let unknown = "[[habitat]]\nid='a'\nbottom=['nope']\n[[habitat]]\nid='z'\n";
    assert!(Habitats::from_toml(unknown).unwrap_err().contains("nope"));
    let dup = "[[habitat]]\nid='a'\nfrozen=true\n[[habitat]]\nid='a'\n";
    assert!(Habitats::from_toml(dup).unwrap_err().contains("duplicate"));
    let no_catch_all = "[[habitat]]\nid='a'\nfrozen=true\n";
    assert!(Habitats::from_toml(no_catch_all).is_err());
    assert!(Habitats::from_toml("").is_err());
    let typo = "[[habitat]]\nid='a'\ndepth_mx=3\n[[habitat]]\nid='z'\n";
    assert!(Habitats::from_toml(typo).is_err());
}
