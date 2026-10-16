//! Parsing and validation of structure templates, shared by `build.rs`
//! (which bakes the shipped templates into the crate) and `Structure::parse`
//! (feature `toml-io`). `build.rs` includes this file by path, so keep it
//! free of crate-internal dependencies beyond `structure_error`.

use std::collections::HashMap;

use crate::structure_error::StructureError;

/// A parsed template: cells are x-fastest, then z, then y.
pub struct ParsedStructure {
    pub size: [i32; 3],
    pub origin_offset: [i32; 3],
    pub raft: bool,
    pub blocks: Vec<u8>,
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
    Ok(ParsedStructure {
        size,
        origin_offset,
        raft,
        blocks,
    })
}
