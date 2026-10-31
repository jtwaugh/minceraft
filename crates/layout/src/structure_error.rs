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
    /// `kind` is not a village piece kind.
    BadKind(String),
    /// A connector's `pool` is not a known pool.
    BadPool(String),
    /// A connector's `facing` is not `+x`, `-x`, `+z` or `-z`.
    BadFacing(String),
    /// A spawn's `kind` is not a known spawn kind.
    BadSpawnKind(String),
    /// A `rotation` entry is not a quarter turn from 0 to 3.
    BadRotation(i64),
    /// A connector or spawn cell lies outside `size`.
    CellOutOfBounds {
        field: &'static str,
        cell: [i32; 3],
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
            Self::BadKind(s) => write!(f, "unknown piece kind `{s}`"),
            Self::BadPool(s) => write!(f, "unknown connector pool `{s}`"),
            Self::BadFacing(s) => write!(f, "connector facing `{s}` is not +x, -x, +z or -z"),
            Self::BadSpawnKind(s) => write!(f, "unknown spawn kind `{s}`"),
            Self::BadRotation(n) => write!(f, "rotation {n} is not a quarter turn from 0 to 3"),
            Self::CellOutOfBounds { field, cell } => {
                let [x, y, z] = cell;
                write!(
                    f,
                    "{field} cell ({x}, {y}, {z}) lies outside the template size"
                )
            }
        }
    }
}

impl std::error::Error for StructureError {}
