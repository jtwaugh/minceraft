//! Run in every build, with and without `toml-io`: the baked table drives the pass.

use crate::{build_route, build_route_layout, SeaFlora, WorldConfig};

#[test]
fn shipped_table_places_shellfish_without_toml_io() {
    let seed = 12345;
    let (l, _) = build_route_layout(&WorldConfig { seed }, &build_route(seed, 2));
    let r = l.generate_layout_region(-96, -96, 192, 192, true).region;
    let flora = SeaFlora::shipped();
    let ids: Vec<u8> = ["oyster", "scallop"]
        .iter()
        .map(|n| flora.get(n).unwrap().blocks[0])
        .collect();
    let n = r.blocks.iter().filter(|b| ids.contains(b)).count();
    assert!(n > 0, "no shellfish placed");
}

#[cfg(feature = "toml-io")]
#[test]
fn baked_table_matches_the_file() {
    let text = include_str!("../../../data/sea_flora.toml");
    let parsed = SeaFlora::from_toml(text, &crate::Habitats::shipped()).unwrap();
    assert_eq!(SeaFlora::shipped(), parsed);
}
