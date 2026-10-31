//! Structure templates: small block patterns stored as data
//! (docs/design/world-layout.md 4.4).
//!
//! The built-in templates in `data/structures/` are parsed by `build.rs`;
//! `Structure::parse` (feature `toml-io`) reads other sources at run time.

pub use crate::structure_error::StructureError;
use crate::structure_piece::{Connector, Facing, PieceKind};

/// A template as emitted by `build.rs`.
pub(crate) struct StaticStructure {
    pub size: [i32; 3],
    pub origin_offset: [i32; 3],
    pub raft: bool,
    pub blocks: &'static [u8],
    pub kind: Option<PieceKind>,
    pub connectors: &'static [Connector],
    pub spawns: &'static [[i32; 3]],
    pub rotations: &'static [u8],
}

impl StaticStructure {
    pub(crate) fn to_structure(&self) -> Structure {
        Structure {
            size: self.size,
            origin_offset: self.origin_offset,
            raft: self.raft,
            blocks: self.blocks.to_vec(),
            kind: self.kind,
            connectors: self.connectors.to_vec(),
            spawns: self.spawns.to_vec(),
            rotations: self.rotations.to_vec(),
        }
    }
}

/// The template called `name` in `list`.
pub(crate) fn find(list: &[(&str, StaticStructure)], name: &str) -> Option<Structure> {
    let found = list.iter().find(|(n, _)| *n == name);
    found.map(|(_, s)| s.to_structure())
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
    /// Village piece kind, if the template is one (villages-and-villagers 2.2).
    pub kind: Option<PieceKind>,
    pub connectors: Vec<Connector>,
    /// Villager spawn cells.
    pub spawns: Vec<[i32; 3]>,
    /// Allowed quarter turns, ascending.
    pub rotations: Vec<u8>,
}

impl Structure {
    /// Parses a template, resolving palette names to block ids.
    #[cfg(feature = "toml-io")]
    pub fn parse(src: &str) -> Result<Structure, StructureError> {
        let p = crate::structure_rules::parse_structure(src, &terrain::block_id)?;
        Ok(Structure {
            size: p.size,
            origin_offset: p.origin_offset,
            raft: p.raft,
            blocks: p.blocks,
            kind: p.kind,
            connectors: p.connectors,
            spawns: p.spawns,
            rotations: p.rotations,
        })
    }

    /// The `raft_start` template from `data/structures/raft_start.toml`.
    pub fn raft_start() -> Structure {
        RAFT_START.to_structure()
    }

    /// The templates in `data/structures/villages/` as `(name, template)`,
    /// `name` being the file stem, sorted by file name.
    pub fn village_templates() -> Vec<(&'static str, Structure)> {
        let list = VILLAGE_TEMPLATES.iter();
        list.map(|(n, s)| (*n, s.to_structure())).collect()
    }

    /// The built-in template called `name`, if there is one.
    pub fn named(name: &str) -> Option<Structure> {
        match name {
            "raft_start" => Some(Structure::raft_start()),
            _ => find(VILLAGE_TEMPLATES, name),
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
                    let [wx, wy, wz] = self.place([x, y, z].map(|c| c as i32), origin, rotation);
                    out.push((wx, wy, wz, self.blocks[(y * sz + z) * sx + x]));
                }
            }
        }
        out
    }

    /// The world cell of template cell `cell`, placed as in [`Structure::cells`].
    fn place(&self, cell: [i32; 3], origin: [i32; 3], rotation: u8) -> [i32; 3] {
        let mut dx = cell[0] + self.origin_offset[0];
        let mut dz = cell[2] + self.origin_offset[2];
        for _ in 0..rotation % 4 {
            (dx, dz) = (-dz, dx);
        }
        let dy = cell[1] + self.origin_offset[1];
        [origin[0] + dx, origin[1] + dy, origin[2] + dz]
    }

    /// The connectors placed as in [`Structure::cells`]: world cells, facings
    /// turned the same quarter turns.
    pub fn connectors_at(&self, origin: [i32; 3], rotation: u8) -> Vec<Connector> {
        self.connectors
            .iter()
            .map(|c| Connector {
                cell: self.place(c.cell, origin, rotation),
                facing: (0..rotation % 4).fold(c.facing, |f, _| turn(f)),
                pool: c.pool,
            })
            .collect()
    }

    /// The villager spawn cells placed as in [`Structure::cells`].
    pub fn spawns_at(&self, origin: [i32; 3], rotation: u8) -> Vec<[i32; 3]> {
        self.spawns
            .iter()
            .map(|&c| self.place(c, origin, rotation))
            .collect()
    }

    /// Whether the template may be placed turned `rotation` quarter turns (mod 4).
    pub fn rotation_allowed(&self, rotation: u8) -> bool {
        self.rotations.contains(&(rotation % 4))
    }

    /// The non-air cells of [`Structure::cells`]; with `raft` set these are
    /// the raft mask cells.
    pub fn solid_cells(&self, origin: [i32; 3], rotation: u8) -> Vec<(i32, i32, i32, u8)> {
        let mut c = self.cells(origin, rotation);
        c.retain(|&(_, _, _, b)| b != terrain::Block::Air as u8);
        c
    }
}

/// One quarter turn of a facing, matching `(dx, dz) -> (-dz, dx)`.
fn turn(f: Facing) -> Facing {
    match f {
        Facing::PosX => Facing::PosZ,
        Facing::PosZ => Facing::NegX,
        Facing::NegX => Facing::NegZ,
        Facing::NegZ => Facing::PosX,
    }
}
