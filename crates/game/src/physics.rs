//! Player body and land-movement tunables loaded from `data/player.toml`.
//! See docs/design/basic-gameplay.md sections 4.1, 4.2, 4.5 and 4.6.

use crate::data_error::{DataError, DataErrorKind, DataFile};
use std::fmt;

const DEFAULT_PLAYER: &str = include_str!("../../../data/player.toml");

/// Every tunable from sections 4.1 to 4.6.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerPhysics {
    /// Bounding box width, blocks.
    pub width: f32,
    /// Bounding box height, blocks.
    pub height: f32,
    pub step_height: f32,
    pub max_health: u32,
    /// Eye height above the feet, blocks.
    pub eye_height: f32,
    /// How much lower the eye sits while sneaking.
    pub sneak_eye_drop: f32,
    /// Multiplier on forward and strafe input.
    pub input_scale: f32,
    /// Extra multiplier on input while sneaking.
    pub sneak_scale: f32,
    /// Multiplier on input while sprinting.
    pub sprint_scale: f32,
    /// Slipperiness of a normal block.
    pub slipperiness: f32,
    /// Horizontal friction per tick in air; on ground it is multiplied by
    /// the block's slipperiness.
    pub friction: f32,
    pub ground_accel: f32,
    pub ground_accel_ref: f32,
    pub air_accel: f32,
    pub jump_velocity: f32,
    /// Horizontal speed added along the facing when a sprinting player jumps.
    pub sprint_jump_boost: f32,
    pub gravity: f32,
    pub drag: f32,
    /// Step size when shrinking motion at a ledge while sneaking.
    pub sneak_edge_step: f32,
    /// Fall distance that does no damage.
    pub fall_damage_threshold: f32,
    /// Horizontal acceleration in water.
    pub water_accel: f32,
    /// Per-tick velocity multiplier in water.
    pub water_velocity_multiplier: f32,
    pub water_gravity: f32,
    /// Added to vertical velocity per tick while swimming up.
    pub water_swim_up_impulse: f32,
    /// Vertical velocity when climbing out of water onto a ledge.
    pub water_climb_out_velocity: f32,
    /// Clearance above the feet required for a climb-out.
    pub water_climb_out_clearance: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhysicsError {
    Parse(String),
    Missing(String),
    Invalid(String),
}

impl From<PhysicsError> for DataError {
    fn from(e: PhysicsError) -> DataError {
        let kind = match e {
            PhysicsError::Parse(m) => DataErrorKind::Parse(m),
            PhysicsError::Missing(k) => DataErrorKind::MissingKey(k),
            PhysicsError::Invalid(m) => DataErrorKind::Invalid(m),
        };
        DataError::new(DataFile::Player, kind)
    }
}

impl fmt::Display for PhysicsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        DataError::from(self.clone()).fmt(f)
    }
}

impl std::error::Error for PhysicsError {}

impl PlayerPhysics {
    /// Eye height above the feet, lower while sneaking.
    pub fn eye_offset(&self, sneaking: bool) -> f32 {
        if sneaking {
            self.eye_height - self.sneak_eye_drop
        } else {
            self.eye_height
        }
    }

    /// The shipped defaults from `data/player.toml`.
    pub fn default_from_data() -> Result<PlayerPhysics, PhysicsError> {
        PlayerPhysics::parse(DEFAULT_PLAYER)
    }

    /// Parse player physics text; every value is required and must be a
    /// finite, non-negative number.
    pub fn parse(text: &str) -> Result<PlayerPhysics, PhysicsError> {
        let root: toml::Value = text
            .parse()
            .map_err(|e: toml::de::Error| PhysicsError::Parse(e.to_string()))?;
        let get = |table: &str, key: &str| -> Result<f32, PhysicsError> {
            let path = format!("{table}.{key}");
            let value = root
                .get(table)
                .and_then(|t| t.get(key))
                .ok_or_else(|| PhysicsError::Missing(path.clone()))?;
            let n = match value {
                toml::Value::Float(f) => *f as f32,
                toml::Value::Integer(i) => *i as f32,
                _ => return Err(PhysicsError::Invalid(format!("{path} must be a number"))),
            };
            if !n.is_finite() || n < 0.0 {
                return Err(PhysicsError::Invalid(format!(
                    "{path} must be a non-negative number"
                )));
            }
            Ok(n)
        };
        let max_health = get("body", "max_health")?;
        if max_health.fract() != 0.0 {
            return Err(PhysicsError::Invalid(
                "body.max_health must be a whole number".into(),
            ));
        }
        let sprint_scale = get("movement", "sprint_scale")?;
        if sprint_scale <= 1.0 {
            return Err(PhysicsError::Invalid(
                "movement.sprint_scale must be greater than 1".into(),
            ));
        }
        let sprint_jump_boost = get("movement", "sprint_jump_boost")?;
        if sprint_jump_boost < 0.0 {
            return Err(PhysicsError::Invalid(
                "movement.sprint_jump_boost must not be negative".into(),
            ));
        }
        Ok(PlayerPhysics {
            width: get("body", "width")?,
            height: get("body", "height")?,
            step_height: get("body", "step_height")?,
            max_health: max_health as u32,
            eye_height: get("camera", "eye_height")?,
            sneak_eye_drop: get("camera", "sneak_eye_drop")?,
            input_scale: get("movement", "input_scale")?,
            sneak_scale: get("movement", "sneak_scale")?,
            sprint_scale,
            slipperiness: get("movement", "slipperiness")?,
            friction: get("movement", "friction")?,
            ground_accel: get("movement", "ground_accel")?,
            ground_accel_ref: get("movement", "ground_accel_ref")?,
            air_accel: get("movement", "air_accel")?,
            jump_velocity: get("movement", "jump_velocity")?,
            sprint_jump_boost,
            gravity: get("movement", "gravity")?,
            drag: get("movement", "drag")?,
            sneak_edge_step: get("sneak", "edge_step")?,
            fall_damage_threshold: get("falling", "damage_threshold")?,
            water_accel: get("water", "accel")?,
            water_velocity_multiplier: get("water", "velocity_multiplier")?,
            water_gravity: get("water", "gravity")?,
            water_swim_up_impulse: get("water", "swim_up_impulse")?,
            water_climb_out_velocity: get("water", "climb_out_velocity")?,
            water_climb_out_clearance: get("water", "climb_out_clearance")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_error::test_support::{
        assert_err_contains, assert_missing_key, assert_parse_error,
    };

    #[test]
    fn defaults_match_design_doc() {
        let p = PlayerPhysics::default_from_data().unwrap();
        assert_eq!(p.width, 0.6);
        assert_eq!(p.height, 1.8);
        assert_eq!(p.step_height, 0.5);
        assert_eq!(p.max_health, 20);
        assert_eq!(p.eye_height, 1.62);
        assert_eq!(p.sneak_eye_drop, 0.08);
        assert!((p.eye_offset(true) - 1.54).abs() < 1e-6);
        assert_eq!(p.input_scale, 0.98);
        assert_eq!(p.sneak_scale, 0.3);
        assert_eq!(p.sprint_scale, 1.3);
        assert_eq!(p.slipperiness, 0.6);
        assert_eq!(p.friction, 0.91);
        assert_eq!(p.ground_accel, 0.1);
        assert_eq!(p.ground_accel_ref, 0.16277136);
        assert_eq!(p.air_accel, 0.02);
        assert_eq!(p.jump_velocity, 0.42);
        assert_eq!(p.sprint_jump_boost, 0.2);
        assert_eq!(p.gravity, 0.08);
        assert_eq!(p.drag, 0.98);
        assert_eq!(p.sneak_edge_step, 0.05);
        assert_eq!(p.fall_damage_threshold, 3.0);
        assert_eq!(p.water_accel, 0.02);
        assert_eq!(p.water_velocity_multiplier, 0.8);
        assert_eq!(p.water_gravity, 0.02);
        assert_eq!(p.water_swim_up_impulse, 0.04);
        assert_eq!(p.water_climb_out_velocity, 0.3);
        assert_eq!(p.water_climb_out_clearance, 0.6);
    }

    #[test]
    fn missing_water_key_is_an_error() {
        let text = without("swim_up_impulse");
        assert!(matches!(
            PlayerPhysics::parse(&text),
            Err(PhysicsError::Missing(k)) if k == "water.swim_up_impulse"
        ));
    }

    #[test]
    fn missing_or_bad_sprint_scale_is_an_error() {
        assert!(matches!(
            PlayerPhysics::parse(&without("sprint_scale")),
            Err(PhysicsError::Missing(k)) if k == "movement.sprint_scale"
        ));
        for bad in ["\"fast\"", "1.0"] {
            let text =
                DEFAULT_PLAYER.replace("sprint_scale = 1.3", &format!("sprint_scale = {bad}"));
            assert!(matches!(
                PlayerPhysics::parse(&text),
                Err(PhysicsError::Invalid(_))
            ));
        }
    }

    #[test]
    fn negative_water_value_is_an_error() {
        let text =
            DEFAULT_PLAYER.replace("climb_out_clearance = 0.6", "climb_out_clearance = -0.6");
        assert!(matches!(
            PlayerPhysics::parse(&text),
            Err(PhysicsError::Invalid(_))
        ));
    }

    #[test]
    fn ground_accel_on_normal_block_is_unchanged() {
        let p = PlayerPhysics::default_from_data().unwrap();
        let f = p.slipperiness * p.friction;
        let accel = p.ground_accel * (p.ground_accel_ref / f.powi(3));
        assert!((accel - 0.1).abs() < 1e-4);
        assert!((f - 0.546).abs() < 1e-6);
    }

    fn without(line_prefix: &str) -> String {
        DEFAULT_PLAYER
            .lines()
            .filter(|l| !l.starts_with(line_prefix))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn missing_value_is_an_error() {
        let text = without("jump_velocity");
        assert_missing_key(
            PlayerPhysics::parse(&text).unwrap_err(),
            DataFile::Player,
            "movement.jump_velocity",
        );
    }

    #[test]
    fn missing_table_is_an_error() {
        let text = without("[falling]");
        assert_err_contains(PlayerPhysics::parse(&text), "player.toml");
    }

    #[test]
    fn negative_value_is_an_error() {
        let text = DEFAULT_PLAYER.replace("gravity = 0.08", "gravity = -0.08");
        assert!(matches!(
            PlayerPhysics::parse(&text),
            Err(PhysicsError::Invalid(_))
        ));
    }

    #[test]
    fn non_number_is_an_error() {
        let text = DEFAULT_PLAYER.replace("drag = 0.98", "drag = \"x\"");
        assert!(matches!(
            PlayerPhysics::parse(&text),
            Err(PhysicsError::Invalid(_))
        ));
    }

    #[test]
    fn fractional_health_is_an_error() {
        let text = DEFAULT_PLAYER.replace("max_health = 20", "max_health = 20.5");
        assert_err_contains(PlayerPhysics::parse(&text), "player.toml");
    }

    #[test]
    fn garbage_is_a_parse_error() {
        assert_parse_error(PlayerPhysics::parse("[body").unwrap_err(), DataFile::Player);
    }
}
