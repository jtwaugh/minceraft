use super::*;
use std::collections::BTreeSet;
use terrain::Block;

type Cell = (i32, i32, i32, u8);

fn set(c: Vec<Cell>) -> BTreeSet<Cell> {
    c.into_iter().collect()
}

/// Quarter turn about the origin `o`: (dx, dz) -> (-dz, dx).
fn turn(c: &BTreeSet<Cell>, o: [i32; 3]) -> BTreeSet<Cell> {
    c.iter()
        .map(|&(x, y, z, b)| (o[0] - (z - o[2]), y, o[2] + (x - o[0]), b))
        .collect()
}

const ORIGIN: [i32; 3] = [100, 63, -40];

#[test]
fn raft_start_cells() {
    let s = Structure::raft_start();
    assert!(s.raft);
    let cells = s.cells(ORIGIN, 0);
    assert_eq!(cells.len(), 50);
    let count = |b: Block| cells.iter().filter(|c| c.3 == b as u8).count();
    assert_eq!(count(Block::RaftPlanks), 25);
    assert_eq!(count(Block::ButcherBlock), 1);
    assert_eq!(count(Block::Griddle), 1);
    assert_eq!(count(Block::Crate), 1);
    assert_eq!(count(Block::Air), 22);
    assert_eq!(s.solid_cells(ORIGIN, 0).len(), 28);
    // Stations: B at (1,1), G at (3,1), C at (2,3), offset by (-2, 0, -2).
    assert!(cells.contains(&(99, 64, -41, Block::ButcherBlock as u8)));
    assert!(cells.contains(&(101, 64, -41, Block::Griddle as u8)));
    assert!(cells.contains(&(100, 64, -39, Block::Crate as u8)));
}

#[test]
fn rotation_turns_the_cell_set() {
    let s = Structure::raft_start();
    let mut expect = set(s.cells(ORIGIN, 0));
    for r in 1..=4u8 {
        expect = turn(&expect, ORIGIN);
        assert_eq!(set(s.cells(ORIGIN, r % 4)), expect, "rotation {r}");
    }
    assert_eq!(set(s.cells(ORIGIN, 4)), set(s.cells(ORIGIN, 0)));
}

#[test]
fn cells_are_pure() {
    let s = Structure::raft_start();
    assert_eq!(s.cells(ORIGIN, 3), s.cells(ORIGIN, 3));
    assert_eq!(s.cells(ORIGIN, 1), Structure::raft_start().cells(ORIGIN, 1));
}

/// `raft_start` (origin_offset [-2, 0, -2]) with markers on asymmetric cells.
fn marked() -> Structure {
    let mut s = Structure::raft_start();
    s.connectors = vec![
        Connector {
            cell: [4, 1, 1],
            facing: Facing::PosX,
            pool: Pool::Boardwalks,
        },
        Connector {
            cell: [0, 1, 3],
            facing: Facing::NegZ,
            pool: Pool::Caps,
        },
    ];
    s.spawns = vec![[1, 1, 0], [3, 1, 4]];
    s
}

/// The world cell `cells` gives template cell `c`.
fn block_at(s: &Structure, c: [i32; 3], r: u8) -> [i32; 3] {
    let [sx, _, sz] = s.size;
    let (x, y, z, _) = s.cells(ORIGIN, r)[((c[1] * sz + c[2]) * sx + c[0]) as usize];
    [x, y, z]
}

#[test]
fn markers_land_on_their_blocks() {
    let s = marked();
    assert_ne!(s.origin_offset, [0, 0, 0]);
    for r in 0..4u8 {
        let conns = s.connectors_at(ORIGIN, r);
        for (placed, c) in conns.iter().zip(&s.connectors) {
            assert_eq!(placed.cell, block_at(&s, c.cell, r), "rotation {r}");
            assert_eq!(placed.pool, c.pool);
        }
        let spawns = s.spawns_at(ORIGIN, r);
        for (placed, &c) in spawns.iter().zip(&s.spawns) {
            assert_eq!(*placed, block_at(&s, c, r), "rotation {r}");
        }
        assert_eq!(spawns.len(), 2);
    }
    // Rotation 1 by hand: [4,1,1] -> offset (2, -1) -> (1, 2).
    assert_eq!(s.connectors_at(ORIGIN, 1)[0].cell, [101, 64, -38]);
}

#[test]
fn facings_turn_a_quarter_per_rotation() {
    let s = marked();
    let facings = |r| {
        s.connectors_at(ORIGIN, r)
            .iter()
            .map(|c| c.facing)
            .collect::<Vec<_>>()
    };
    assert_eq!(facings(0), [Facing::PosX, Facing::NegZ]);
    assert_eq!(facings(1), [Facing::PosZ, Facing::PosX]);
    assert_eq!(facings(2), [Facing::NegX, Facing::PosZ]);
    assert_eq!(facings(3), [Facing::NegZ, Facing::NegX]);
    assert_eq!(facings(4), facings(0));
    assert_eq!(s.connectors_at(ORIGIN, 4), s.connectors_at(ORIGIN, 0));
}

#[test]
fn facing_matches_the_turned_offset() {
    let step = |f| match f {
        Facing::PosX => (1, 0),
        Facing::NegX => (-1, 0),
        Facing::PosZ => (0, 1),
        Facing::NegZ => (0, -1),
    };
    let s = marked();
    for r in 0..4u8 {
        for (placed, c) in s.connectors_at(ORIGIN, r).iter().zip(&s.connectors) {
            let (mut dx, mut dz) = step(c.facing);
            for _ in 0..r {
                (dx, dz) = (-dz, dx);
            }
            assert_eq!(step(placed.facing), (dx, dz), "rotation {r}");
        }
    }
}

#[test]
fn rotation_allowed_reads_the_list() {
    let mut s = Structure::raft_start();
    assert!((0..8).all(|r| s.rotation_allowed(r)));
    assert!(s.connectors_at(ORIGIN, 1).is_empty() && s.spawns_at(ORIGIN, 1).is_empty());
    s.rotations = vec![0, 2];
    assert!(s.rotation_allowed(0) && s.rotation_allowed(2) && s.rotation_allowed(6));
    assert!(!s.rotation_allowed(1) && !s.rotation_allowed(3) && !s.rotation_allowed(5));
}

#[cfg(feature = "toml-io")]
#[test]
fn asymmetric_template_rotates_offsets() {
    let s = Structure::parse(
        "size=[2,1,1]\norigin_offset=[0,0,0]\npalette={ A=\"stone\", B=\"dirt\" }\nlayers=[[\"AB\"]]\n",
    )
    .unwrap();
    assert_eq!(
        set(s.cells([0, 0, 0], 1)),
        BTreeSet::from([(0, 0, 0, Block::Stone as u8), (0, 0, 1, Block::Dirt as u8)])
    );
}

#[cfg(feature = "toml-io")]
fn with(size: &str, palette: &str, layers: &str) -> String {
    format!("size={size}\norigin_offset=[0,0,0]\npalette={palette}\nlayers={layers}\n")
}

#[cfg(feature = "toml-io")]
#[test]
fn bad_templates_are_rejected() {
    let p = "{ A = \"stone\", \".\" = \"air\" }";
    assert!(Structure::parse(&with("[2,1,1]", p, "[[\"AA\"]]")).is_ok());
    assert!(matches!(
        Structure::parse(&with("[2,1,1]", "{ A = \"nope\" }", "[[\"AA\"]]")),
        Err(StructureError::UnknownBlock(_))
    ));
    assert!(matches!(
        Structure::parse(&with("[2,1,1]", p, "[[\"AAA\"]]")),
        Err(StructureError::RowLength { .. })
    ));
    assert!(matches!(
        Structure::parse(&with("[2,1,1]", p, "[[\"AA\",\"AA\"]]")),
        Err(StructureError::RowCount { .. })
    ));
    assert!(matches!(
        Structure::parse(&with("[2,2,1]", p, "[[\"AA\"]]")),
        Err(StructureError::LayerCount { .. })
    ));
    assert!(matches!(
        Structure::parse(&with("[2,1,1]", p, "[[\"AZ\"]]")),
        Err(StructureError::UnknownSymbol('Z'))
    ));
    assert!(Structure::parse(&with("[0,1,1]", p, "[[\"\"]]")).is_err());
}

#[test]
fn structure_error_display_is_readable() {
    let cases = [
        (StructureError::Toml("bad".into()), vec!["TOML", "bad"]),
        (StructureError::Field("size"), vec!["`size`"]),
        (StructureError::BadPaletteKey("ab".into()), vec!["\"ab\""]),
        (StructureError::UnknownBlock("mud".into()), vec!["`mud`"]),
        (StructureError::UnknownSymbol('Z'), vec!["'Z'"]),
        (
            StructureError::LayerCount {
                expected: 3,
                found: 5,
            },
            vec!["3", "5", "layers"],
        ),
        (
            StructureError::RowCount {
                layer: 2,
                expected: 4,
                found: 7,
            },
            vec!["layer 2", "7", "4"],
        ),
        (
            StructureError::RowLength {
                layer: 1,
                row: 6,
                expected: 8,
                found: 9,
            },
            vec!["layer 1", "row 6", "9", "8"],
        ),
    ];
    for (err, keys) in cases {
        let text = err.to_string();
        assert!(!text.contains(" { "), "debug-style output: {text}");
        for k in keys {
            assert!(text.contains(k), "{text:?} lacks {k:?}");
        }
    }
}
