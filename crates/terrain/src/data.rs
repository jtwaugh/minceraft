//! Data tables generated at compile time from `data/blocks.toml` and
//! `data/biomes.toml` (see `build.rs`). The types are declared here; the
//! values (and the `Block` / `Biome` enums) come from the generated file.

/// Collision shape of a block (SPEC 16).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Collision {
    None,
    Full,
    /// Cactus: inset 1/16 on the four sides and the top.
    Inset,
}

/// Static properties of one block.
#[derive(Clone, Copy, Debug)]
pub struct BlockInfo {
    pub name: &'static str,
    /// Fire encouragement (spread onto neighbouring air).
    pub fire_encouragement: u8,
    /// Fire flammability (catching and burning away).
    pub flammability: u8,
    pub collision: Collision,
    /// Light opacity (255 = opaque, leaves 1, water / ice 3).
    pub opacity: u8,
    /// "Opaque cube" flag (leaves included, quirk B6).
    pub opaque_cube: bool,
    pub liquid: bool,
    /// Flat fallback colour.
    pub color: [u8; 3],
}

/// Small-plant patch attempts per 16x16 cell (SPEC 9.1). Fractions below 1
/// are one patch with that probability.
#[derive(Clone, Copy, Debug)]
pub struct Plants {
    pub dandelion: f64,
    pub rose: f64,
    pub tall_grass: f64,
    /// Probability that a tall-grass patch is ferns.
    pub fern_share: f64,
    pub dead_bush: f64,
    pub cactus: f64,
    pub sugar_cane: f64,
    pub pumpkin: f64,
    pub brown_mushroom: f64,
    pub red_mushroom: f64,
}

/// Static properties of one biome.
#[derive(Clone, Copy, Debug)]
pub struct BiomeInfo {
    pub name: &'static str,
    pub map_color: [u8; 3],
    /// Default top / filler block ids for the surface pass.
    pub top: u8,
    pub filler: u8,
    /// Whether the noise-driven base count N is added to the tree count.
    pub tree_noise: bool,
    pub tree_bonus: i32,
    /// Relative weights indexed by [`TreeKind`].
    pub tree_weights: [u32; 5],
    pub plants: Plants,
    /// Three stops, by humidity.
    pub grass_ramp: [[u8; 3]; 3],
    /// Three stops, by humidity.
    pub leaf_ramp: [[u8; 3]; 3],
    /// Three stops, by temperature.
    pub sky_ramp: [[u8; 3]; 3],
    pub creatures: &'static [(&'static str, u32)],
    pub monsters: &'static [(&'static str, u32)],
    pub water_creatures: &'static [(&'static str, u32)],
}

/// Tree generators (SPEC 11), index into `BiomeInfo::tree_weights`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeKind {
    Oak = 0,
    Birch = 1,
    TaigaA = 2,
    TaigaB = 3,
    BigOak = 4,
}

impl TreeKind {
    pub const ALL: [TreeKind; 5] = [
        TreeKind::Oak,
        TreeKind::Birch,
        TreeKind::TaigaA,
        TreeKind::TaigaB,
        TreeKind::BigOak,
    ];
}

/// Oak / birch shape. `height = (base, spread)`: `base + rand(spread)`.
pub struct SmallTree {
    pub log: u8,
    pub leaves: u8,
    pub height: (u32, u32),
    /// Leaf-layer radii bottom to top; the top layer is at `y + h`.
    pub canopy: &'static [i32],
}

pub struct TaigaA {
    pub log: u8,
    pub leaves: u8,
    pub height: (u32, u32),
    /// `a = h - base - rand(spread)`.
    pub bare_trunk: (u32, u32),
}

pub struct TaigaB {
    pub log: u8,
    pub leaves: u8,
    pub height: (u32, u32),
    /// `a = base + rand(spread)`.
    pub bare_trunk: (u32, u32),
    /// `R = base + rand(spread)`.
    pub max_radius: (u32, u32),
}

pub struct BigOak {
    pub log: u8,
    pub leaves: u8,
    pub height: (u32, u32),
    pub min_height: i32,
    pub trunk_ratio: f64,
    pub branch_slope: f64,
    pub foliage_density: f64,
    pub branch_cutoff: f64,
    pub level_cutoff: f64,
    pub foliage_radii: &'static [f64],
}

include!(concat!(env!("OUT_DIR"), "/data.rs"));

impl Block {
    pub fn from_u8(v: u8) -> Block {
        if (v as usize) < Block::COUNT {
            Block::ALL[v as usize]
        } else {
            Block::Air
        }
    }

    #[inline]
    pub fn info(self) -> &'static BlockInfo {
        &BLOCK_INFO[self as usize]
    }
}

impl Biome {
    pub fn from_u8(v: u8) -> Biome {
        Biome::ALL[(v as usize).min(Biome::COUNT - 1)]
    }

    #[inline]
    pub fn info(self) -> &'static BiomeInfo {
        &BIOME_INFO[self as usize]
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }
}

/// Byte-level property helpers for the hot paths.
#[inline]
pub fn opacity(b: u8) -> u8 {
    BLOCK_INFO.get(b as usize).map_or(0, |i| i.opacity)
}

/// Movement-blocking: any collision except none (SPEC Part II notation).
#[inline]
pub fn blocks_movement(b: u8) -> bool {
    BLOCK_INFO
        .get(b as usize)
        .is_some_and(|i| i.collision != Collision::None)
}

#[inline]
pub fn is_opaque_cube(b: u8) -> bool {
    BLOCK_INFO.get(b as usize).is_some_and(|i| i.opaque_cube)
}

#[inline]
pub fn is_liquid(b: u8) -> bool {
    BLOCK_INFO.get(b as usize).is_some_and(|i| i.liquid)
}

#[inline]
pub fn is_leaves(b: u8) -> bool {
    b == Block::OakLeaves as u8 || b == Block::BirchLeaves as u8 || b == Block::SpruceLeaves as u8
}
