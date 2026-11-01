//! Invariants every village template must keep (docs/design/villages-and-villagers.md
//! 2.1, 2.2). Covers each file in `data/structures/villages/`.

use crate::structure_piece::{Facing, PieceKind};
use crate::Structure;

fn count_range(kind: PieceKind) -> (usize, usize) {
    match kind {
        PieceKind::Plaza => (3, 6),
        PieceKind::Dock => (1, 1),
        PieceKind::Boardwalk => (2, 4),
        PieceKind::Building => (1, 3),
        PieceKind::Cap | PieceKind::Decor => (1, 1),
    }
}

#[test]
fn templates_exist_and_have_a_kind() {
    let list = Structure::village_templates();
    for name in ["plaza_small", "plaza_wide", "dock_straight"] {
        assert!(list.iter().any(|(n, _)| *n == name), "{name}");
    }
    for (name, s) in &list {
        assert!(s.kind.is_some(), "{name} has no kind");
    }
}

#[test]
fn homes_have_a_villager_and_one_to_three_connectors() {
    let list = Structure::village_templates();
    for name in ["home_hut_a", "home_hut_b", "home_house", "home_longhouse"] {
        let (_, s) = list.iter().find(|(n, _)| *n == name).expect(name);
        assert_eq!(s.kind, Some(PieceKind::Building), "{name}");
        assert!((1..=3).contains(&s.connectors.len()), "{name}");
        assert!(!s.spawns.is_empty(), "{name} has no villager");
    }
}

#[test]
fn connector_counts_match_the_kind_table() {
    for (name, s) in Structure::village_templates() {
        let (lo, hi) = count_range(s.kind.unwrap());
        let n = s.connectors.len();
        assert!((lo..=hi).contains(&n), "{name}: {n} connectors");
    }
}

#[test]
fn connectors_and_spawns_stay_on_the_edge_at_floor_height() {
    for (name, s) in Structure::village_templates() {
        for rot in 0..4u8 {
            let origin = [100, 64, -40];
            // The bounding box of the placed template.
            let cells = s.cells(origin, rot);
            let min = |i: usize| cells.iter().map(|c| [c.0, c.1, c.2][i]).min().unwrap();
            let max = |i: usize| cells.iter().map(|c| [c.0, c.1, c.2][i]).max().unwrap();
            for c in s.connectors_at(origin, rot) {
                let at = format!("{name} rot {rot} {c:?}");
                assert_eq!(c.cell[1], origin[1] + 1 + s.origin_offset[1], "{at}");
                let (axis, edge) = match c.facing {
                    Facing::PosX => (0, max(0)),
                    Facing::NegX => (0, min(0)),
                    Facing::PosZ => (2, max(2)),
                    Facing::NegZ => (2, min(2)),
                };
                assert_eq!(c.cell[axis], edge, "{at}");
            }
            for c in s.spawns_at(origin, rot) {
                assert_eq!(c[1], origin[1] + 1 + s.origin_offset[1], "{name} rot {rot}");
            }
        }
    }
}

#[test]
fn four_quarter_turns_return_the_original() {
    let turn =
        |c: (i32, i32, i32, u8), o: [i32; 3]| (o[0] - (c.2 - o[2]), c.1, o[2] + (c.0 - o[0]), c.3);
    for (name, s) in Structure::village_templates() {
        let origin = [3, 10, 7];
        let mut cells = s.cells(origin, 0);
        for _ in 0..4 {
            cells = cells.into_iter().map(|c| turn(c, origin)).collect();
        }
        assert_eq!(cells, s.cells(origin, 0), "{name}");
        let once = s.cells(origin, 1);
        let back: Vec<_> = once
            .into_iter()
            .map(|c| turn(turn(turn(c, origin), origin), origin))
            .collect();
        assert_eq!(back, s.cells(origin, 0), "{name}");
    }
}
