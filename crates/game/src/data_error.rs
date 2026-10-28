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
    Commands,
    Phases,
    Hands,
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
            DataFile::Commands => "commands.toml",
            DataFile::Phases => "phases.toml",
            DataFile::Hands => "hands.toml",
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

/// Assertion helpers for error tests. Test-only, so none of it reaches wasm.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::fmt::{Debug, Display};

    /// Panics unless `result` is an `Err` whose text contains `needle`.
    #[track_caller]
    pub(crate) fn assert_err_contains<T: Debug, E: Display>(result: Result<T, E>, needle: &str) {
        match result {
            Ok(v) => panic!("expected an error containing {needle:?}, got Ok({v:?})"),
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains(needle),
                    "error {msg:?} does not contain {needle:?}"
                );
            }
        }
    }

    /// Converts `err` to a `DataError` and checks its file.
    #[track_caller]
    fn data_error(err: impl Into<DataError>, file: DataFile) -> DataError {
        let e: DataError = err.into();
        assert_eq!(e.file, file, "wrong file in {e}");
        e
    }

    /// Checks `err` is a `BadEntry` in `file` for `entry`, its problem containing `problem_contains`.
    #[track_caller]
    pub(crate) fn assert_bad_entry(
        err: impl Into<DataError>,
        file: DataFile,
        entry: &str,
        problem_contains: &str,
    ) {
        let e = data_error(err, file);
        match &e.kind {
            DataErrorKind::BadEntry { entry: en, problem } => {
                assert_eq!(en, entry, "wrong entry in {e}");
                assert!(
                    problem.contains(problem_contains),
                    "problem {problem:?} does not contain {problem_contains:?}"
                );
            }
            _ => panic!("expected BadEntry, got {e:?}"),
        }
    }

    /// Checks `err` is an `UnknownItem` in `file` for `entry` naming `item`.
    #[track_caller]
    pub(crate) fn assert_unknown_item(
        err: impl Into<DataError>,
        file: DataFile,
        entry: &str,
        item: &str,
    ) {
        let e = data_error(err, file);
        match &e.kind {
            DataErrorKind::UnknownItem {
                entry: en,
                item: it,
            } => {
                assert_eq!(en, entry, "wrong entry in {e}");
                assert_eq!(it, item, "wrong item in {e}");
            }
            _ => panic!("expected UnknownItem, got {e:?}"),
        }
    }

    /// Checks `err` is a `Parse` error in `file`.
    #[track_caller]
    pub(crate) fn assert_parse_error(err: impl Into<DataError>, file: DataFile) {
        let e = data_error(err, file);
        assert!(
            matches!(e.kind, DataErrorKind::Parse(_)),
            "expected Parse, got {e:?}"
        );
    }

    /// Checks `err` is a `MissingKey` in `file` for `key`.
    #[track_caller]
    pub(crate) fn assert_missing_key(err: impl Into<DataError>, file: DataFile, key: &str) {
        let e = data_error(err, file);
        match &e.kind {
            DataErrorKind::MissingKey(k) => assert_eq!(k, key, "wrong key in {e}"),
            _ => panic!("expected MissingKey, got {e:?}"),
        }
    }

    fn bad(file: DataFile, entry: &str, problem: &str) -> DataError {
        DataError::new(
            file,
            DataErrorKind::BadEntry {
                entry: entry.into(),
                problem: problem.into(),
            },
        )
    }

    fn unknown(file: DataFile, entry: &str, item: &str) -> DataError {
        DataError::new(
            file,
            DataErrorKind::UnknownItem {
                entry: entry.into(),
                item: item.into(),
            },
        )
    }

    fn parse(file: DataFile) -> DataError {
        DataError::new(file, DataErrorKind::Parse("bad".into()))
    }

    fn missing(file: DataFile, key: &str) -> DataError {
        DataError::new(file, DataErrorKind::MissingKey(key.into()))
    }

    #[test]
    fn err_contains_passes() {
        assert_err_contains::<(), _>(Err("a long message"), "long");
        assert_err_contains::<(), _>(Err(bad(DataFile::Crops, "wheat", "no stages")), "wheat");
    }

    #[test]
    #[should_panic(expected = "got Ok(5)")]
    fn err_contains_fails_on_ok() {
        assert_err_contains(Ok::<_, String>(5), "x");
    }

    #[test]
    #[should_panic(expected = "does not contain")]
    fn err_contains_fails_on_missing_substring() {
        assert_err_contains::<(), _>(Err("abc"), "zzz");
    }

    #[test]
    fn bad_entry_passes() {
        assert_bad_entry(
            bad(DataFile::Crops, "wheat", "no stages"),
            DataFile::Crops,
            "wheat",
            "stages",
        );
    }

    #[test]
    #[should_panic(expected = "wrong file")]
    fn bad_entry_fails_on_wrong_file() {
        assert_bad_entry(
            bad(DataFile::Crops, "wheat", "no stages"),
            DataFile::Items,
            "wheat",
            "stages",
        );
    }

    #[test]
    #[should_panic(expected = "wrong entry")]
    fn bad_entry_fails_on_wrong_entry() {
        assert_bad_entry(
            bad(DataFile::Crops, "wheat", "no stages"),
            DataFile::Crops,
            "corn",
            "stages",
        );
    }

    #[test]
    #[should_panic(expected = "does not contain")]
    fn bad_entry_fails_on_missing_substring() {
        assert_bad_entry(
            bad(DataFile::Crops, "wheat", "no stages"),
            DataFile::Crops,
            "wheat",
            "zzz",
        );
    }

    #[test]
    #[should_panic(expected = "expected BadEntry")]
    fn bad_entry_fails_on_wrong_kind() {
        assert_bad_entry(parse(DataFile::Crops), DataFile::Crops, "wheat", "");
    }

    #[test]
    fn unknown_item_passes() {
        assert_unknown_item(
            unknown(DataFile::Crops, "wheat", "seed"),
            DataFile::Crops,
            "wheat",
            "seed",
        );
    }

    #[test]
    #[should_panic(expected = "wrong file")]
    fn unknown_item_fails_on_wrong_file() {
        assert_unknown_item(
            unknown(DataFile::Crops, "wheat", "seed"),
            DataFile::Animals,
            "wheat",
            "seed",
        );
    }

    #[test]
    #[should_panic(expected = "wrong entry")]
    fn unknown_item_fails_on_wrong_entry() {
        assert_unknown_item(
            unknown(DataFile::Crops, "wheat", "seed"),
            DataFile::Crops,
            "corn",
            "seed",
        );
    }

    #[test]
    #[should_panic(expected = "wrong item")]
    fn unknown_item_fails_on_wrong_item() {
        assert_unknown_item(
            unknown(DataFile::Crops, "wheat", "seed"),
            DataFile::Crops,
            "wheat",
            "stick",
        );
    }

    #[test]
    #[should_panic(expected = "expected UnknownItem")]
    fn unknown_item_fails_on_wrong_kind() {
        assert_unknown_item(parse(DataFile::Crops), DataFile::Crops, "wheat", "seed");
    }

    #[test]
    fn parse_error_passes() {
        assert_parse_error(parse(DataFile::Input), DataFile::Input);
    }

    #[test]
    #[should_panic(expected = "wrong file")]
    fn parse_error_fails_on_wrong_file() {
        assert_parse_error(parse(DataFile::Input), DataFile::Player);
    }

    #[test]
    #[should_panic(expected = "expected Parse")]
    fn parse_error_fails_on_wrong_kind() {
        assert_parse_error(missing(DataFile::Input, "k"), DataFile::Input);
    }

    #[test]
    fn missing_key_passes() {
        assert_missing_key(
            missing(DataFile::Player, "gravity"),
            DataFile::Player,
            "gravity",
        );
    }

    #[test]
    #[should_panic(expected = "wrong file")]
    fn missing_key_fails_on_wrong_file() {
        assert_missing_key(
            missing(DataFile::Player, "gravity"),
            DataFile::Items,
            "gravity",
        );
    }

    #[test]
    #[should_panic(expected = "wrong key")]
    fn missing_key_fails_on_wrong_key() {
        assert_missing_key(
            missing(DataFile::Player, "gravity"),
            DataFile::Player,
            "speed",
        );
    }

    #[test]
    #[should_panic(expected = "expected MissingKey")]
    fn missing_key_fails_on_wrong_kind() {
        assert_missing_key(parse(DataFile::Player), DataFile::Player, "gravity");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(DataFile, &str); 8] = [
        (DataFile::Player, "player.toml"),
        (DataFile::Animals, "animals.toml"),
        (DataFile::Items, "items.toml"),
        (DataFile::Targeting, "targeting.toml"),
        (DataFile::Input, "input.toml"),
        (DataFile::Interactions, "interactions.toml"),
        (DataFile::Crops, "crops.toml"),
        (DataFile::Commands, "commands.toml"),
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
