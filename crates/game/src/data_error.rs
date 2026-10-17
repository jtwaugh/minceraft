//! Shared error type for the TOML data files the game loads.
//! See docs/design/error-and-state-conventions.md, Phase B.

use std::fmt;

/// A data file under `data/`. The only place a file name is spelled out for
/// error messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataFile {
    Player,
    Animals,
    Items,
    Targeting,
    Input,
    Interactions,
    Crops,
}

impl DataFile {
    pub fn name(self) -> &'static str {
        match self {
            DataFile::Player => "player.toml",
            DataFile::Animals => "animals.toml",
            DataFile::Items => "items.toml",
            DataFile::Targeting => "targeting.toml",
            DataFile::Input => "input.toml",
            DataFile::Interactions => "interactions.toml",
            DataFile::Crops => "crops.toml",
        }
    }
}

/// What went wrong in a data file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataErrorKind {
    /// The text is not valid TOML, or does not fit the schema.
    Parse(String),
    /// A required key is absent.
    MissingKey(String),
    /// A value is out of range or otherwise unusable.
    Invalid(String),
    /// One entry (a crop, an animal, a rule, ...) has a problem.
    BadEntry { entry: String, problem: String },
    /// An entry names an item that `items.toml` does not define.
    UnknownItem { entry: String, item: String },
    /// A name is defined twice; `kind` says what sort of name.
    Duplicate { kind: String, name: String },
}

/// A problem in one data file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataError {
    pub file: DataFile,
    pub kind: DataErrorKind,
}

impl DataError {
    pub fn new(file: DataFile, kind: DataErrorKind) -> Self {
        DataError { file, kind }
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = self.file.name();
        match &self.kind {
            DataErrorKind::Parse(msg) | DataErrorKind::Invalid(msg) => {
                write!(f, "{file}: {msg}")
            }
            DataErrorKind::MissingKey(key) => write!(f, "{file}: missing `{key}`"),
            DataErrorKind::BadEntry { entry, problem } => {
                write!(f, "{file}: {entry}: {problem}")
            }
            DataErrorKind::UnknownItem { entry, item } => {
                write!(
                    f,
                    "{file}: {entry}: `{item}` is not in {}",
                    DataFile::Items.name()
                )
            }
            DataErrorKind::Duplicate { kind, name } => {
                write!(f, "{file}: duplicate {kind} `{name}`")
            }
        }
    }
}

impl std::error::Error for DataError {}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(DataFile, &str); 7] = [
        (DataFile::Player, "player.toml"),
        (DataFile::Animals, "animals.toml"),
        (DataFile::Items, "items.toml"),
        (DataFile::Targeting, "targeting.toml"),
        (DataFile::Input, "input.toml"),
        (DataFile::Interactions, "interactions.toml"),
        (DataFile::Crops, "crops.toml"),
    ];

    fn show(file: DataFile, kind: DataErrorKind) -> String {
        DataError::new(file, kind).to_string()
    }

    #[test]
    fn file_names() {
        for (file, name) in ALL {
            assert_eq!(file.name(), name);
        }
    }

    #[test]
    fn parse_display() {
        let kind = DataErrorKind::Parse("expected `=`".into());
        assert_eq!(show(DataFile::Input, kind), "input.toml: expected `=`");
    }

    #[test]
    fn missing_key_display() {
        let kind = DataErrorKind::MissingKey("gravity".into());
        assert_eq!(
            show(DataFile::Player, kind),
            "player.toml: missing `gravity`"
        );
    }

    #[test]
    fn invalid_display() {
        let kind = DataErrorKind::Invalid("reach must be positive".into());
        assert_eq!(
            show(DataFile::Targeting, kind),
            "targeting.toml: reach must be positive"
        );
    }

    #[test]
    fn bad_entry_display() {
        let kind = DataErrorKind::BadEntry {
            entry: "wheat".into(),
            problem: "no stages".into(),
        };
        assert_eq!(show(DataFile::Crops, kind), "crops.toml: wheat: no stages");
    }

    #[test]
    fn unknown_item_display() {
        let kind = DataErrorKind::UnknownItem {
            entry: "wheat".into(),
            item: "seed".into(),
        };
        assert_eq!(
            show(DataFile::Crops, kind),
            "crops.toml: wheat: `seed` is not in items.toml"
        );
    }

    #[test]
    fn duplicate_display() {
        let kind = DataErrorKind::Duplicate {
            kind: "item".into(),
            name: "stick".into(),
        };
        assert_eq!(
            show(DataFile::Items, kind),
            "items.toml: duplicate item `stick`"
        );
    }

    #[test]
    fn is_an_error() {
        let e: Box<dyn std::error::Error> = Box::new(DataError::new(
            DataFile::Animals,
            DataErrorKind::MissingKey("x".into()),
        ));
        assert!(e.to_string().starts_with("animals.toml: "));
    }
}
