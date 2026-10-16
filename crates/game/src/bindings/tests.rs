use super::*;

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
    assert!(b.sensitivity > 0.0);
    assert_eq!(b.get(Action::MoveForward).keys, ["KeyW"]);
    assert_eq!(b.get(Action::Jump).mode, Mode::Hold);
    assert_eq!(b.get(Action::Primary).mode, Mode::Edge);
    assert_eq!(b.get(Action::Look).mode, Mode::Continuous);
    assert!(b.get(Action::Secondary).hold_repeat);
    assert_eq!(b.action_for_key("Digit3"), Some(Action::SelectTool));
    assert_eq!(b.action_for_key("WheelUp"), Some(Action::SelectTool));
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
    assert!(err.to_string().contains("listed twice"));
}

#[test]
fn rejects_key_shared_between_actions() {
    let text = shipped().replace("keys = [\"KeyQ\"]", "keys = [\"KeyE\"]");
    let err = Bindings::parse(&text).unwrap_err();
    assert!(err.to_string().contains("bound to both"), "{err}");
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
