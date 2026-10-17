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
        match self {
            Self::Toml(msg) => write!(f, "template is not valid TOML: {msg}"),
            Self::Field(name) => write!(f, "field `{name}` is missing or has the wrong type"),
            Self::BadPaletteKey(key) => {
                write!(f, "palette key {key:?} is not a single character")
            }
            Self::UnknownBlock(name) => write!(f, "palette names unknown block `{name}`"),
            Self::UnknownSymbol(c) => {
                write!(
                    f,
                    "layer uses symbol {c:?} that the palette does not define"
                )
            }
            Self::LayerCount { expected, found } => {
                write!(f, "expected {expected} layers but found {found}")
            }
            Self::RowCount {
                layer,
                expected,
                found,
            } => write!(f, "layer {layer} has {found} rows, expected {expected}"),
            Self::RowLength {
                layer,
                row,
                expected,
                found,
            } => write!(
                f,
                "layer {layer} row {row} is {found} cells long, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for StructureError {}
