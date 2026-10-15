//! Structure templates: small block patterns stored as data
//! (docs/design/world-layout.md 4.4).

use std::collections::HashMap;
use std::fmt;

use terrain::BLOCK_INFO;

/// The `raft_start` template source.
pub const RAFT_START: &str = include_str!("../../../data/structures/raft_start.toml");

/// Why a template failed to parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructureError {
    Toml(String),
    /// A required field is missing or has the wrong type.
    Field(&'static str),
    /// A palette key is not a single character.
    BadPaletteKey(String),
    UnknownBlock(String),
    /// A layer cell uses a character the palette does not define.
    UnknownSymbol(char),
    /// `layers` length differs from `size[1]`.
    LayerCount {
        expected: usize,
        found: usize,
    },
    /// A layer's row count differs from `size[2]`.
    RowCount {
        layer: usize,
        expected: usize,
        found: usize,
    },
    /// A row's length differs from `size[0]`.
    RowLength {
        layer: usize,
        row: usize,
        expected: usize,
        found: usize,
    },
}

impl fmt::Display for StructureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for StructureError {}

/// A parsed template. Cells are stored x-fastest, then z, then y.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Structure {
    pub size: [i32; 3],
    pub origin_offset: [i32; 3],
    /// Every non-air cell joins the raft mask.
    pub raft: bool,
    blocks: Vec<u8>,
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

impl Structure {
    /// Parses a template, resolving palette names to block ids.
    pub fn parse(src: &str) -> Result<Structure, StructureError> {
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
            let id = BLOCK_INFO
                .iter()
                .position(|b| b.name == name)
                .ok_or_else(|| StructureError::UnknownBlock(name.to_string()))?;
            palette.insert(c, id as u8);
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
        Ok(Structure {
            size,
            origin_offset,
            raft,
            blocks,
        })
    }

    /// The `raft_start` template from `data/structures/raft_start.toml`.
    pub fn raft_start() -> Structure {
        Structure::parse(RAFT_START).expect("raft_start.toml is valid")
    }

    /// The built-in template called `name`, if there is one.
    pub fn named(name: &str) -> Option<Structure> {
        match name {
            "raft_start" => Some(Structure::raft_start()),
            _ => None,
        }
    }

    /// Every cell `(x, y, z, block)`, air included, placed at `origin` and
    /// turned `rotation` quarter turns (mod 4) about `origin`. A quarter turn
    /// maps an offset `(dx, dz)` from the origin to `(-dz, dx)`, so
    /// `origin_offset` turns with the template. Order: y, then z, then x of
    /// the unrotated template.
    pub fn cells(&self, origin: [i32; 3], rotation: u8) -> Vec<(i32, i32, i32, u8)> {
        let [sx, sy, sz] = self.size.map(|s| s as usize);
        let mut out = Vec::with_capacity(self.blocks.len());
        for y in 0..sy {
            for z in 0..sz {
                for x in 0..sx {
                    let mut dx = x as i32 + self.origin_offset[0];
                    let mut dz = z as i32 + self.origin_offset[2];
                    for _ in 0..rotation % 4 {
                        (dx, dz) = (-dz, dx);
                    }
                    let dy = y as i32 + self.origin_offset[1];
                    let b = self.blocks[(y * sz + z) * sx + x];
                    out.push((origin[0] + dx, origin[1] + dy, origin[2] + dz, b));
                }
            }
        }
        out
    }

    /// The non-air cells of [`Structure::cells`]; with `raft` set these are
    /// the raft mask cells.
    pub fn solid_cells(&self, origin: [i32; 3], rotation: u8) -> Vec<(i32, i32, i32, u8)> {
        let mut c = self.cells(origin, rotation);
        c.retain(|&(_, _, _, b)| b != terrain::Block::Air as u8);
        c
    }
}
