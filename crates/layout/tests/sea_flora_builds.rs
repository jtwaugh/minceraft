//! The sea flora pass runs in every feature build: the shipped table is baked
//! in by `build.rs`, so `--no-default-features` places the same shellfish as
//! the default build. Run with and without `--no-default-features`.

use layout::{build_route, build_route_layout, SeaFlora, WorldConfig};
use terrain::HEIGHT;

/// Shellfish placed in the area below, counted in the default build.
const EXPECTED: usize = 6;

#[test]
fn shellfish_placed_the_same_in_every_build() {
    let seed = 12345;
    let (l, _) = build_route_layout(&WorldConfig { seed }, &build_route(seed, 2));
    let r = l.generate_layout_region(-160, -160, 320, 320, true).region;
    let ids: Vec<u8> = SeaFlora::shipped()
        .entries
        .iter()
        .filter(|e| e.name != "kelp")
        .map(|e| e.blocks[0])
        .collect();
    assert!(r.blocks.len() % HEIGHT == 0);
    let n = r.blocks.iter().filter(|b| ids.contains(b)).count();
    assert_eq!(n, EXPECTED);
}
