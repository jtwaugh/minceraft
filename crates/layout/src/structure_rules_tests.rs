use crate::structure_error::StructureError;
use crate::structure_piece::*;
use crate::structure_rules::*;

const BASE: &str = "size=[3,2,3]\norigin_offset=[0,0,0]\npalette={ P=\"stone\", \".\"=\"air\" }\n\
    layers=[[\"PPP\",\"PPP\",\"PPP\"],[\"...\",\"...\",\"...\"]]\n";

const VILLAGE: &str = "kind = \"boardwalk\"\nrotation = [2, 0]\n\
    connectors = [\n\
      { cell = [0, 1, 1], facing = \"-x\", pool = \"boardwalks\" },\n\
      { cell = [2, 1, 1], facing = \"+x\", pool = \"buildings\" },\n\
      { cell = [1, 1, 0], facing = \"-z\", pool = \"decor\" },\n\
      { cell = [1, 1, 2], facing = \"+z\", pool = \"caps\" },\n\
    ]\n\
    spawns = [{ cell = [1, 1, 1], kind = \"villager\" }]\n";

fn parse(extra: &str) -> Result<ParsedStructure, StructureError> {
    parse_structure(&format!("{extra}{BASE}"), &terrain::block_id)
}

#[test]
fn village_keys_parse() {
    let p = parse(VILLAGE).unwrap();
    assert_eq!(p.kind, Some(PieceKind::Boardwalk));
    assert_eq!(p.rotations, vec![0, 2]);
    assert_eq!(
        p.connectors,
        vec![
            Connector {
                cell: [0, 1, 1],
                facing: Facing::NegX,
                pool: Pool::Boardwalks
            },
            Connector {
                cell: [2, 1, 1],
                facing: Facing::PosX,
                pool: Pool::Buildings
            },
            Connector {
                cell: [1, 1, 0],
                facing: Facing::NegZ,
                pool: Pool::Decor
            },
            Connector {
                cell: [1, 1, 2],
                facing: Facing::PosZ,
                pool: Pool::Caps
            },
        ]
    );
    assert_eq!(p.spawns, vec![[1, 1, 1]]);
    let plain = parse("").unwrap();
    assert_eq!(plain.blocks, p.blocks);
}

#[test]
fn every_kind_parses() {
    let kinds = [
        ("plaza", PieceKind::Plaza),
        ("dock", PieceKind::Dock),
        ("boardwalk", PieceKind::Boardwalk),
        ("building", PieceKind::Building),
        ("cap", PieceKind::Cap),
        ("decor", PieceKind::Decor),
    ];
    for (name, kind) in kinds {
        let p = parse(&format!("kind = \"{name}\"\n")).unwrap();
        assert_eq!(p.kind, Some(kind), "{name}");
    }
}

#[test]
fn template_without_village_keys_is_unchanged() {
    let p = parse("").unwrap();
    assert_eq!(p.size, [3, 2, 3]);
    assert_eq!(p.kind, None);
    assert!(p.connectors.is_empty());
    assert!(p.spawns.is_empty());
    assert_eq!(p.rotations, vec![0, 1, 2, 3]);
}

#[test]
fn raft_start_parses_as_before() {
    let src = include_str!("../../../data/structures/raft_start.toml");
    let p = parse_structure(src, &terrain::block_id).unwrap();
    let s = crate::Structure::raft_start();
    assert_eq!(
        (p.size, p.origin_offset, p.raft),
        (s.size, s.origin_offset, s.raft)
    );
    assert_eq!(crate::Structure::parse(src).unwrap(), s);
    assert_eq!(p.kind, None);
    assert!(p.connectors.is_empty() && p.spawns.is_empty());
    assert_eq!(p.rotations, vec![0, 1, 2, 3]);
}

fn err(extra: &str) -> StructureError {
    match parse(extra) {
        Ok(_) => panic!("{extra:?} parsed"),
        Err(e) => e,
    }
}

const C: &str = "connectors = [{ cell = [0, 1, 1], facing = \"-x\", pool = \"boardwalks\" }]\n";

#[test]
fn bad_village_values_are_rejected() {
    assert_eq!(
        err("kind = \"tower\"\n"),
        StructureError::BadKind("tower".into())
    );
    assert_eq!(
        err(&C.replace("boardwalks", "roofs")),
        StructureError::BadPool("roofs".into())
    );
    assert_eq!(
        err(&C.replace("-x", "+y")),
        StructureError::BadFacing("+y".into())
    );
    assert_eq!(
        err("spawns = [{ cell = [1, 1, 1], kind = \"pig\" }]\n"),
        StructureError::BadSpawnKind("pig".into())
    );
    assert_eq!(err("rotation = [0, 4]\n"), StructureError::BadRotation(4));
    assert_eq!(err("rotation = [-1]\n"), StructureError::BadRotation(-1));
    assert_eq!(
        err(&C.replace("[0, 1, 1]", "[3, 1, 1]")),
        StructureError::CellOutOfBounds {
            field: "connectors",
            cell: [3, 1, 1]
        }
    );
    assert_eq!(
        err("spawns = [{ cell = [1, 2, -1], kind = \"villager\" }]\n"),
        StructureError::CellOutOfBounds {
            field: "spawns",
            cell: [1, 2, -1]
        }
    );
}

#[test]
fn malformed_village_keys_are_field_errors() {
    let cases = [
        ("kind = 3\n", "kind"),
        ("rotation = []\n", "rotation"),
        ("rotation = 1\n", "rotation"),
        ("rotation = [\"one\"]\n", "rotation"),
        ("connectors = { cell = [0, 1, 1] }\n", "connectors"),
        (
            "connectors = [{ facing = \"-x\", pool = \"caps\" }]\n",
            "connectors",
        ),
        (
            "connectors = [{ cell = [0, 1, 1], pool = \"caps\" }]\n",
            "facing",
        ),
        (
            "connectors = [{ cell = [0, 1, 1], facing = \"-x\" }]\n",
            "pool",
        ),
        (
            "spawns = [{ cell = [1, 1], kind = \"villager\" }]\n",
            "spawns",
        ),
        ("spawns = [{ cell = [1, 1, 1] }]\n", "kind"),
    ];
    for (extra, field) in cases {
        assert_eq!(err(extra), StructureError::Field(field), "{extra:?}");
    }
}

#[test]
fn village_error_display_is_readable() {
    let cases = [
        (
            StructureError::BadKind("tower".into()),
            vec!["`tower`", "kind"],
        ),
        (
            StructureError::BadPool("roofs".into()),
            vec!["`roofs`", "pool"],
        ),
        (StructureError::BadFacing("+y".into()), vec!["`+y`", "+x"]),
        (
            StructureError::BadSpawnKind("pig".into()),
            vec!["`pig`", "spawn"],
        ),
        (StructureError::BadRotation(4), vec!["4", "0 to 3"]),
        (
            StructureError::CellOutOfBounds {
                field: "spawns",
                cell: [1, 2, -1],
            },
            vec!["spawns", "[1, 2, -1]", "outside"],
        ),
        (
            StructureError::NotAtFloorHeight {
                field: "connectors",
                cell: [0, 0, 1],
            },
            vec!["connectors", "[0, 0, 1]", "y = 1"],
        ),
        (
            StructureError::ConnectorOffEdge {
                cell: [1, 1, 1],
                facing: "+x",
            },
            vec!["[1, 1, 1]", "edge", "+x"],
        ),
    ];
    for (e, keys) in cases {
        let text = e.to_string();
        for k in keys {
            assert!(text.contains(k), "{text:?} lacks {k:?}");
        }
    }
}

fn conn(cell: &str, facing: &str) -> String {
    format!("connectors = [{{ cell = {cell}, facing = \"{facing}\", pool = \"decor\" }}]\n")
}

#[test]
fn connectors_must_sit_on_the_facing_edge() {
    let good = [
        ("[2, 1, 1]", "+x"),
        ("[0, 1, 1]", "-x"),
        ("[1, 1, 2]", "+z"),
        ("[1, 1, 0]", "-z"),
    ];
    for (cell, facing) in good {
        assert!(parse(&conn(cell, facing)).is_ok(), "{cell} {facing}");
    }
    let bad = [
        ("[1, 1, 1]", "+x", "+x"),
        ("[2, 1, 1]", "-x", "-x"),
        ("[1, 1, 1]", "+z", "+z"),
        ("[1, 1, 2]", "-z", "-z"),
    ];
    for (cell, facing, name) in bad {
        let StructureError::ConnectorOffEdge { facing: f, .. } = err(&conn(cell, facing)) else {
            panic!("{cell} {facing} accepted");
        };
        assert_eq!(f, name);
    }
}

#[test]
fn connectors_and_spawns_must_be_at_floor_height() {
    assert_eq!(
        err(&conn("[0, 0, 1]", "-x")),
        StructureError::NotAtFloorHeight {
            field: "connectors",
            cell: [0, 0, 1]
        }
    );
    assert_eq!(
        err("spawns = [{ cell = [1, 0, 1], kind = \"villager\" }]\n"),
        StructureError::NotAtFloorHeight {
            field: "spawns",
            cell: [1, 0, 1]
        }
    );
}
