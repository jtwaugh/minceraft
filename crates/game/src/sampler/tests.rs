use super::*;

fn sampler() -> Sampler {
    Sampler::new(Bindings::default_from_data().unwrap())
}

#[test]
fn inventory_key_changes_no_tick_field() {
    let mut s = sampler();
    s.key_down("Tab");
    s.key_up("Tab");
    assert_eq!(s.drain(), sampler().drain());
    s.key_down("Tab");
    assert_eq!(s.drain(), sampler().drain());
}

#[test]
fn wasd_and_opposites() {
    let mut s = sampler();
    s.key_down("KeyW");
    s.key_down("KeyD");
    let t = s.drain();
    assert_eq!((t.move_forward, t.move_strafe), (1.0, 1.0));
    s.key_down("KeyS");
    assert_eq!(s.drain().move_forward, 0.0);
    s.key_up("KeyW");
    s.key_up("KeyS");
    s.key_up("KeyD");
    s.key_down("KeyA");
    let t = s.drain();
    assert_eq!((t.move_forward, t.move_strafe), (0.0, -1.0));
}

#[test]
fn click_between_ticks_is_not_lost_and_repeat_is_ignored() {
    let mut s = sampler();
    s.key_down("MouseLeft");
    s.key_down("MouseLeft");
    s.key_up("MouseLeft");
    assert!(s.drain().primary);
    assert!(!s.drain().primary);
}

#[test]
fn secondary_edge_and_hold() {
    let mut s = sampler();
    s.key_down("MouseRight");
    let t = s.drain();
    assert!(t.secondary && t.secondary_held);
    let t = s.drain();
    assert!(!t.secondary && t.secondary_held);
    s.key_up("MouseRight");
    assert!(!s.drain().secondary_held);
}

#[test]
fn jump_sneak_interact_drop() {
    let mut s = sampler();
    s.key_down("Space");
    s.key_down("ShiftLeft");
    s.key_down("KeyE");
    s.key_down("KeyQ");
    let t = s.drain();
    assert!(t.jump && t.sneak && t.interact && t.drop);
    s.key_up("Space");
    let t = s.drain();
    assert!(!t.jump && t.sneak && !t.interact && !t.drop);
}

#[test]
fn tool_slots_from_digits_and_wheel() {
    let mut s = sampler();
    s.key_down("Digit3");
    assert_eq!(s.drain().tool_slot, Some(2));
    assert_eq!(s.drain().tool_slot, None);
    s.wheel(1.0);
    assert_eq!(s.drain().tool_slot, Some(3));
    s.wheel(1.0);
    s.wheel(1.0);
    assert_eq!(s.drain().tool_slot, Some(0));
    s.wheel(-1.0);
    assert_eq!(s.drain().tool_slot, Some(4));
    s.key_down("Digit9");
    assert_eq!(s.drain().tool_slot, None);
}

#[test]
fn mouse_turns_absolute_angles_and_clamps_pitch() {
    let mut s = sampler();
    let k = s.bindings().sensitivity;
    s.mouse_move(100.0, -40.0);
    let (yaw, pitch) = s.look();
    assert!((yaw - 100.0 * k).abs() < 1e-6 && (pitch - 40.0 * k).abs() < 1e-6);
    s.mouse_move(0.0, -1.0e6);
    assert_eq!(s.look().1, std::f32::consts::FRAC_PI_2);
    let t = s.drain();
    assert_eq!((t.yaw, t.pitch), s.look());
}

#[test]
fn clear_held_releases_inputs_but_keeps_edges_and_look() {
    let mut s = sampler();
    s.key_down("KeyW");
    s.key_down("Space");
    s.key_down("MouseRight");
    s.mouse_move(10.0, 0.0);
    s.clear_held();
    let t = s.drain();
    assert_eq!(
        (t.move_forward, t.jump, t.secondary_held),
        (0.0, false, false)
    );
    assert!(t.secondary);
    assert!(t.yaw > 0.0);
    // A key released after the clear is not a stray event.
    s.key_up("KeyW");
    s.key_down("KeyW");
    assert_eq!(s.drain().move_forward, 1.0);
}

#[test]
fn unbound_keys_are_ignored() {
    let mut s = sampler();
    s.key_down("KeyZ");
    s.key_up("KeyZ");
    assert_eq!(s.drain(), TickInput::default());
}
