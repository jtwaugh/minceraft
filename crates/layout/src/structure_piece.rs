//! Village piece data a template may carry (docs/design/villages-and-villagers.md
//! 2.1, 2.2). `build.rs` includes this file by path, so keep it free of
//! dependencies.

/// The piece kinds of villages-and-villagers 2.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    Plaza,
    Dock,
    Boardwalk,
    Building,
    Cap,
    Decor,
}

/// A connector's direction, in template axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    PosX,
    NegX,
    PosZ,
    NegZ,
}

/// The pool a connector's neighbour is drawn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    Boardwalks,
    Buildings,
    Decor,
    Caps,
}

/// A join point: a template cell, the way it faces and its neighbour's pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Connector {
    pub cell: [i32; 3],
    pub facing: Facing,
    pub pool: Pool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnKind {
    Villager,
}

/// A spawn marker: a template cell and what spawns there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spawn {
    pub cell: [i32; 3],
    pub kind: SpawnKind,
}
