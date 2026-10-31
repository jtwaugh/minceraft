//! Parsing and validation of structure templates, shared by `build.rs`
//! (which bakes the shipped templates into the crate) and `Structure::parse`
//! (feature `toml-io`). `build.rs` includes this file by path, so keep it
//! free of crate-internal dependencies beyond `structure_error` and
//! `structure_piece`.

use std::collections::HashMap;

use crate::structure_error::StructureError;
use crate::structure_piece::{Connector, Facing, PieceKind, Pool, Spawn, SpawnKind};

/// A parsed template: cells are x-fastest, then z, then y.
pub struct ParsedStructure {
    pub size: [i32; 3],
    pub origin_offset: [i32; 3],
    pub raft: bool,
    pub blocks: Vec<u8>,
    /// Village piece kind (villages-and-villagers 2.1), if the template is one.
    pub kind: Option<PieceKind>,
    pub connectors: Vec<Connector>,
    pub spawns: Vec<Spawn>,
    /// Allowed quarter turns, ascending; all four unless `rotation` says otherwise.
    pub rotations: Vec<u8>,
}

/// Looks up `key` as a string in `v` and maps it with `pick`; a value
/// `pick` rejects becomes `bad(value)`.
fn choice<T>(
    v: &toml::Value,
    key: &'static str,
    pick: fn(&str) -> Option<T>,
    bad: fn(String) -> StructureError,
) -> Result<T, StructureError> {
    let s = v
        .get(key)
        .and_then(|s| s.as_str())
        .ok_or(StructureError::Field(key))?;
    pick(s).ok_or_else(|| bad(s.to_string()))
}

/// A `cell` that must lie inside `size`.
fn cell_in(
    v: &toml::Value,
    field: &'static str,
    size: [i32; 3],
) -> Result<[i32; 3], StructureError> {
    let cell = int3(v, "cell").map_err(|_| StructureError::Field(field))?;
    if cell.iter().zip(size).any(|(&c, s)| c < 0 || c >= s) {
        return Err(StructureError::CellOutOfBounds { field, cell });
    }
    Ok(cell)
}

/// The optional list `key`, each entry parsed by `item`.
fn list<T>(
    v: &toml::Value,
    key: &'static str,
    item: impl Fn(&toml::Value) -> Result<T, StructureError>,
) -> Result<Vec<T>, StructureError> {
    match v.get(key) {
        None => Ok(Vec::new()),
        Some(a) => a
            .as_array()
            .ok_or(StructureError::Field(key))?
            .iter()
            .map(item)
            .collect(),
    }
}

fn connector(v: &toml::Value, size: [i32; 3]) -> Result<Connector, StructureError> {
    let facing = |s: &str| match s {
        "+x" => Some(Facing::PosX),
        "-x" => Some(Facing::NegX),
        "+z" => Some(Facing::PosZ),
        "-z" => Some(Facing::NegZ),
        _ => None,
    };
    let pool = |s: &str| match s {
        "boardwalks" => Some(Pool::Boardwalks),
        "buildings" => Some(Pool::Buildings),
        "decor" => Some(Pool::Decor),
        "caps" => Some(Pool::Caps),
        _ => None,
    };
    Ok(Connector {
        cell: cell_in(v, "connectors", size)?,
        facing: choice(v, "facing", facing, StructureError::BadFacing)?,
        pool: choice(v, "pool", pool, StructureError::BadPool)?,
    })
}

fn spawn(v: &toml::Value, size: [i32; 3]) -> Result<Spawn, StructureError> {
    let kind = |s: &str| (s == "villager").then_some(SpawnKind::Villager);
    Ok(Spawn {
        cell: cell_in(v, "spawns", size)?,
        kind: choice(v, "kind", kind, StructureError::BadSpawnKind)?,
    })
}

fn rotations(v: &toml::Value) -> Result<Vec<u8>, StructureError> {
    if v.get("rotation").is_none() {
        return Ok(vec![0, 1, 2, 3]);
    }
    let mut out = list(v, "rotation", |r| {
        let n = r.as_integer().ok_or(StructureError::Field("rotation"))?;
        u8::try_from(n)
            .ok()
            .filter(|&q| q < 4)
            .ok_or(StructureError::BadRotation(n))
    })?;
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err(StructureError::Field("rotation"));
    }
    Ok(out)
}

fn int3(v: &toml::Value, key: &'static str) -> Result<[i32; 3], StructureError> {
    let arr = v
        .get(key)
        .and_then(|a| a.as_array())
        .ok_or(StructureError::Field(key))?;
    let mut out = [0i32; 3];
    if arr.len() != 3 {
        return Err(StructureError::Field(key));
    }
    for (o, e) in out.iter_mut().zip(arr) {
        let n = e.as_integer().ok_or(StructureError::Field(key))?;
        *o = i32::try_from(n).map_err(|_| StructureError::Field(key))?;
    }
    Ok(out)
}

/// Parse a template. `block_id` maps a block name to its id.
pub fn parse_structure(
    src: &str,
    block_id: &dyn Fn(&str) -> Option<u8>,
) -> Result<ParsedStructure, StructureError> {
    let v: toml::Value = src
        .parse()
        .map_err(|e: toml::de::Error| StructureError::Toml(e.to_string()))?;
    let size = int3(&v, "size")?;
    if size.iter().any(|&s| s < 1) {
        return Err(StructureError::Field("size"));
    }
    let origin_offset = int3(&v, "origin_offset")?;
    let raft = match v.get("raft") {
        None => false,
        Some(r) => r.as_bool().ok_or(StructureError::Field("raft"))?,
    };
    let table = v
        .get("palette")
        .and_then(|p| p.as_table())
        .ok_or(StructureError::Field("palette"))?;
    let mut palette: HashMap<char, u8> = HashMap::new();
    for (key, val) in table {
        let mut chars = key.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return Err(StructureError::BadPaletteKey(key.clone()));
        };
        let name = val.as_str().ok_or(StructureError::Field("palette"))?;
        let id = block_id(name).ok_or_else(|| StructureError::UnknownBlock(name.to_string()))?;
        palette.insert(c, id);
    }
    let layers = v
        .get("layers")
        .and_then(|l| l.as_array())
        .ok_or(StructureError::Field("layers"))?;
    let (sx, sy, sz) = (size[0] as usize, size[1] as usize, size[2] as usize);
    if layers.len() != sy {
        return Err(StructureError::LayerCount {
            expected: sy,
            found: layers.len(),
        });
    }
    let mut blocks = Vec::with_capacity(sx * sy * sz);
    for (y, layer) in layers.iter().enumerate() {
        let rows = layer.as_array().ok_or(StructureError::Field("layers"))?;
        if rows.len() != sz {
            return Err(StructureError::RowCount {
                layer: y,
                expected: sz,
                found: rows.len(),
            });
        }
        for (z, row) in rows.iter().enumerate() {
            let row = row.as_str().ok_or(StructureError::Field("layers"))?;
            let found = row.chars().count();
            if found != sx {
                return Err(StructureError::RowLength {
                    layer: y,
                    row: z,
                    expected: sx,
                    found,
                });
            }
            for c in row.chars() {
                blocks.push(*palette.get(&c).ok_or(StructureError::UnknownSymbol(c))?);
            }
        }
    }
    let kind = match v.get("kind") {
        None => None,
        Some(_) => Some(choice(
            &v,
            "kind",
            |s| match s {
                "plaza" => Some(PieceKind::Plaza),
                "dock" => Some(PieceKind::Dock),
                "boardwalk" => Some(PieceKind::Boardwalk),
                "building" => Some(PieceKind::Building),
                "cap" => Some(PieceKind::Cap),
                "decor" => Some(PieceKind::Decor),
                _ => None,
            },
            StructureError::BadKind,
        )?),
    };
    Ok(ParsedStructure {
        size,
        origin_offset,
        raft,
        blocks,
        kind,
        connectors: list(&v, "connectors", |c| connector(c, size))?,
        spawns: list(&v, "spawns", |s| spawn(s, size))?,
        rotations: rotations(&v)?,
    })
}
