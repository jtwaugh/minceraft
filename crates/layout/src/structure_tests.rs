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

fn with(size: &str, palette: &str, layers: &str) -> String {
    format!("size={size}\norigin_offset=[0,0,0]\npalette={palette}\nlayers={layers}\n")
}

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
