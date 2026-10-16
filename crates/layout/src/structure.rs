//! Structure templates: small block patterns stored as data
//! (docs/design/world-layout.md 4.4).
//!
//! The built-in templates in `data/structures/` are parsed by `build.rs`;
//! `Structure::parse` (feature `toml-io`) reads other sources at run time.

pub use crate::structure_error::StructureError;

/// A template as emitted by `build.rs`.
pub(crate) struct StaticStructure {
    pub size: [i32; 3],
    pub origin_offset: [i32; 3],
    pub raft: bool,
    pub blocks: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/structure.rs"));

/// A parsed template. Cells are stored x-fastest, then z, then y.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Structure {
    pub size: [i32; 3],
    pub origin_offset: [i32; 3],
    /// Every non-air cell joins the raft mask.
    pub raft: bool,
    blocks: Vec<u8>,
}

impl Structure {
    /// Parses a template, resolving palette names to block ids.
    #[cfg(feature = "toml-io")]
    pub fn parse(src: &str) -> Result<Structure, StructureError> {
        let p = crate::structure_rules::parse_structure(src, &|name| {
            terrain::BLOCK_INFO
                .iter()
                .position(|b| b.name == name)
                .map(|i| i as u8)
        })?;
        Ok(Structure {
            size: p.size,
            origin_offset: p.origin_offset,
            raft: p.raft,
            blocks: p.blocks,
        })
    }

    /// The `raft_start` template from `data/structures/raft_start.toml`.
    pub fn raft_start() -> Structure {
        let s = &RAFT_START;
        Structure {
            size: s.size,
            origin_offset: s.origin_offset,
            raft: s.raft,
            blocks: s.blocks.to_vec(),
        }
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
