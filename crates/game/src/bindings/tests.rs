use super::*;
use crate::data_error::test_support::assert_err_contains;

#[test]
fn errors_compare_and_clone() {
    let e = BindingsError::DuplicateKey {
        key: "KeyW".into(),
        first: "jump",
        second: "sneak",
    };
    assert_eq!(e.clone(), e);
    assert_ne!(e, BindingsError::Invalid("x".into()));
}

fn shipped() -> String {
    DEFAULT_INPUT.to_owned()
}

#[test]
fn shipped_file_loads_every_action() {
    let b = Bindings::default_from_data().unwrap();
    for a in Action::ALL {
        assert!(!b.get(a).keys.is_empty(), "{}", a.name());
    }
    assert_eq!(b.fov_degrees, 70.0);
    assert_eq!(b.double_tap_ticks, 7);
    assert!(b.sensitivity > 0.0);
    assert_eq!(b.get(Action::MoveForward).keys, ["KeyW"]);
    assert_eq!(b.get(Action::Jump).mode, Mode::Hold);
    assert_eq!(b.get(Action::Primary).mode, Mode::Edge);
    assert_eq!(b.get(Action::Look).mode, Mode::Continuous);
    assert!(b.get(Action::Secondary).hold_repeat);
    assert_eq!(b.action_for_key("Digit3"), Some(Action::SelectTool));
    assert_eq!(b.action_for_key("WheelUp"), Some(Action::SelectTool));
    assert_eq!(b.action_for_key("Tab"), Some(Action::Inventory));
    assert_eq!(b.get(Action::Inventory).keys, ["Tab"]);
    assert_eq!(b.get(Action::Inventory).mode, Mode::Edge);
    assert_eq!(b.action_for_key("KeyZ"), None);
}

#[test]
fn rejects_unknown_action() {
    let text = shipped() + "\n[actions.fly]\nkeys = [\"KeyF\"]\nmode = \"hold\"\n";
    assert_eq!(
        Bindings::parse(&text),
        Err(BindingsError::UnknownAction("fly".into()))
    );
}

#[test]
fn rejects_duplicate_key_within_action() {
    let text = shipped().replace("keys = [\"KeyE\"]", "keys = [\"KeyE\", \"KeyE\"]");
    let err = Bindings::parse(&text).unwrap_err();
    assert!(matches!(err, BindingsError::DuplicateKey { first, second, .. } if first == second));
    assert_err_contains::<(), _>(Err(err), "listed twice");
}

#[test]
fn rejects_key_shared_between_actions() {
    let text = shipped().replace("keys = [\"KeyQ\"]", "keys = [\"KeyE\"]");
    let err = Bindings::parse(&text).unwrap_err();
    assert_err_contains::<(), _>(Err(err), "bound to both");
}

#[test]
fn rejects_missing_action() {
    let text = shipped().replace("[actions.drop]", "[ignored.drop]");
    assert_eq!(
        Bindings::parse(&text),
        Err(BindingsError::MissingAction("drop"))
    );
}

#[test]
fn rejects_malformed_input() {
    assert!(matches!(
        Bindings::parse("not toml ["),
        Err(BindingsError::Parse(_))
    ));
    let bad_mode = shipped().replace("mode = \"hold\"", "mode = \"toggle\"");
    assert!(matches!(
        Bindings::parse(&bad_mode),
        Err(BindingsError::Invalid(_))
    ));
    let bad_fov = shipped().replace("fov_degrees = 70.0", "fov_degrees = 400.0");
    assert!(matches!(
        Bindings::parse(&bad_fov),
        Err(BindingsError::Invalid(_))
    ));
}

#[test]
fn errors_convert_to_data_error_with_same_message() {
    let cases = [
        (
            BindingsError::Parse("expected `=`".into()),
            "input.toml: expected `=`",
        ),
        (
            BindingsError::UnknownAction("fly".into()),
            "input.toml: unknown action `fly`",
        ),
        (
            BindingsError::MissingAction("drop"),
            "input.toml: missing action `drop`",
        ),
        (
            BindingsError::DuplicateKey {
                key: "KeyW".into(),
                first: "jump",
                second: "jump",
            },
            "input.toml: key `KeyW` listed twice for action `jump`",
        ),
        (
            BindingsError::DuplicateKey {
                key: "KeyW".into(),
                first: "jump",
                second: "drop",
            },
            "input.toml: key `KeyW` bound to both `jump` and `drop`",
        ),
        (BindingsError::Invalid("bad".into()), "input.toml: bad"),
    ];
    for (err, msg) in cases {
        let shown = err.to_string();
        let data = DataError::from(err);
        assert_eq!(data.file, DataFile::Input);
        assert_eq!(data.to_string(), msg);
        assert_eq!(shown, msg);
    }
}

#[test]
fn rejects_missing_or_bad_double_tap_window() {
    let missing = shipped().replace("double_tap_ticks = 7", "");
    assert_err_contains::<(), _>(Bindings::parse(&missing).map(|_| ()), "double_tap_ticks");
    for bad in ["\"7\"", "7.5", "0"] {
        let text = shipped().replace("double_tap_ticks = 7", &format!("double_tap_ticks = {bad}"));
        assert_err_contains::<(), _>(Bindings::parse(&text).map(|_| ()), "double_tap_ticks");
    }
}
