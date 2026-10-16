//! Why a structure template failed to parse. `build.rs` includes this file
//! by path, so keep it free of dependencies.

use std::fmt;

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
