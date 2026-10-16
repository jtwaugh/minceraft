//! Free-look camera with tick-to-tick interpolation, and a debug fly step.
//! See docs/design/basic-gameplay.md section 3 and section 2.2.
//!
//! Conventions (this project's own): right-handed, +Y up, yaw 0 faces -Z and
//! increasing yaw turns right, positive pitch looks up.

use crate::input::TickInput;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Debug fly speed in blocks per tick (10 blocks per second). A Rust constant
/// on purpose: fly mode is a debug camera that real movement (design section 4)
/// replaces, so it is not a tunable worth shipping in `data/`.
pub const FLY_SPEED: f32 = 0.5;

/// Position and look angles at one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

/// Shortest-way interpolation between two angles, across the +/- pi wrap.
pub fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let mut d = (b - a) % TAU;
    if d > PI {
        d -= TAU;
    } else if d < -PI {
        d += TAU;
    }
    a + d * t
}

/// Clamp a movement axis to -1..1; NaN counts as 0.
fn clamp_axis(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(-1.0, 1.0)
    }
}

impl Pose {
    pub fn lerp(self, other: Pose, t: f32) -> Pose {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Pose {
            pos: [
                l(self.pos[0], other.pos[0]),
                l(self.pos[1], other.pos[1]),
                l(self.pos[2], other.pos[2]),
            ],
            yaw: lerp_angle(self.yaw, other.yaw, t),
            pitch: l(self.pitch, other.pitch),
        }
    }

    /// Unit look direction.
    pub fn forward(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [sy * cp, sp, -cy * cp]
    }

    /// Unit vector to the camera's right, horizontal.
    pub fn right(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        [cy, 0.0, sy]
    }

    /// Column-major view matrix (world to eye space, eye looks down -Z).
    pub fn view_matrix(&self) -> [f32; 16] {
        let f = self.forward();
        let r = self.right();
        let u = [
            r[1] * f[2] - r[2] * f[1],
            r[2] * f[0] - r[0] * f[2],
            r[0] * f[1] - r[1] * f[0],
        ];
        let p = self.pos;
        let dot = |a: [f32; 3]| a[0] * p[0] + a[1] * p[1] + a[2] * p[2];
        [
            r[0],
            u[0],
            -f[0],
            0.0, //
            r[1],
            u[1],
            -f[1],
            0.0, //
            r[2],
            u[2],
            -f[2],
            0.0, //
            -dot(r),
            -dot(u),
            dot(f),
            1.0,
        ]
    }
}

/// Previous and current tick poses, plus the projection's field of view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    prev: Pose,
    cur: Pose,
    fov_degrees: f32,
}

impl Camera {
    /// `fov_degrees` is the vertical field of view, `look.fov_degrees` in
    /// `data/input.toml` (70 by default).
    pub fn new(pose: Pose, fov_degrees: f32) -> Camera {
        Camera {
            prev: pose,
            cur: pose,
            fov_degrees,
        }
    }

    pub fn fov_degrees(&self) -> f32 {
        self.fov_degrees
    }

    pub fn fov_radians(&self) -> f32 {
        self.fov_degrees.to_radians()
    }

    /// The latest tick's pose.
    pub fn current(&self) -> Pose {
        self.cur
    }

    /// Record a new tick's pose; the old current becomes the previous.
    pub fn push(&mut self, pose: Pose) {
        self.prev = self.cur;
        self.cur = pose;
    }

    /// The pose to render at `alpha` (0..1) between the last two ticks.
    pub fn view(&self, alpha: f32) -> Pose {
        self.prev.lerp(self.cur, alpha.clamp(0.0, 1.0))
    }

    /// Like [`Camera::view`], but with the per-frame look (`yaw`, `pitch`)
    /// replacing the interpolated look (design 2.2: look is applied every
    /// frame, the sim reads it at the tick boundary). Position still
    /// interpolates between ticks.
    pub fn view_with_look(&self, alpha: f32, yaw: f32, pitch: f32) -> Pose {
        Pose {
            yaw,
            pitch: pitch.clamp(-FRAC_PI_2, FRAC_PI_2),
            ..self.view(alpha)
        }
    }

    /// Debug fly step: take look from `input`, move along the look direction
    /// (forward), strafe, and up (jump) / down (sneak). No collision.
    pub fn fly_tick(&mut self, input: &TickInput) {
        let mut next = self.cur;
        next.yaw = input.yaw;
        next.pitch = input.pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
        let f = next.forward();
        let r = next.right();
        let up = f32::from(input.jump) - f32::from(input.sneak);
        // Clamp each axis to -1..1, then limit the combined magnitude to 1 so
        // diagonals are no faster than straight lines.
        let (mf, ms) = (
            clamp_axis(input.move_forward),
            clamp_axis(input.move_strafe),
        );
        let len = (mf * mf + ms * ms).sqrt();
        let scale = if len > 1.0 { 1.0 / len } else { 1.0 };
        let (mf, ms) = (mf * scale, ms * scale);
        for i in 0..3 {
            next.pos[i] += FLY_SPEED * (f[i] * mf + r[i] * ms);
        }
        next.pos[1] += FLY_SPEED * up;
        self.push(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn yaw_interpolates_the_short_way_across_the_wrap() {
        let a = PI - 0.1;
        let b = -PI + 0.1;
        let mid = lerp_angle(a, b, 0.5);
        assert!(close(mid.cos(), -1.0), "mid {mid}");
        assert!(close(lerp_angle(b, a, 0.25), b - 0.05));
        assert!(close(lerp_angle(0.0, 1.0, 0.5), 0.5));
        assert!(close(lerp_angle(1.0, 1.0, 0.7), 1.0));
    }

    #[test]
    fn view_interpolates_between_ticks() {
        let mut c = Camera::new(Pose::default(), 70.0);
        c.push(Pose {
            pos: [2.0, 4.0, 6.0],
            yaw: 1.0,
            pitch: 0.5,
        });
        let v = c.view(0.5);
        assert_eq!(v.pos, [1.0, 2.0, 3.0]);
        assert!(close(v.yaw, 0.5) && close(v.pitch, 0.25));
        assert_eq!(c.view(0.0), Pose::default());
        assert_eq!(c.view(1.0), c.current());
        assert_eq!(c.fov_degrees(), 70.0);
    }

    #[test]
    fn view_matrix_at_origin_is_identity_and_moves_the_world() {
        let m = Pose::default().view_matrix();
        let id = [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ];
        assert!(m.iter().zip(id).all(|(a, b)| close(*a, b)));
        // A camera at (0,0,5) sees the origin 5 blocks ahead: eye z = -5.
        let m = Pose {
            pos: [0.0, 0.0, 5.0],
            ..Pose::default()
        }
        .view_matrix();
        assert!(close(m[14], -5.0));
    }

    #[test]
    fn fly_moves_in_the_look_direction() {
        let mut c = Camera::new(Pose::default(), 70.0);
        let input = TickInput {
            move_forward: 1.0,
            yaw: FRAC_PI_2,
            pitch: FRAC_PI_2 / 2.0,
            ..TickInput::default()
        };
        c.fly_tick(&input);
        let p = c.current().pos;
        let f = c.current().forward();
        assert!(f[0] > 0.0 && f[1] > 0.0);
        for i in 0..3 {
            assert!(close(p[i], f[i] * FLY_SPEED));
        }
        assert!(close(c.view(0.0).pos[0], 0.0));
    }

    #[test]
    fn fly_strafes_and_goes_up_and_down() {
        let mut c = Camera::new(Pose::default(), 70.0);
        c.fly_tick(&TickInput {
            move_strafe: 1.0,
            jump: true,
            ..TickInput::default()
        });
        assert_eq!(c.current().pos, [FLY_SPEED, FLY_SPEED, 0.0]);
        c.fly_tick(&TickInput {
            sneak: true,
            ..TickInput::default()
        });
        assert_eq!(c.current().pos, [FLY_SPEED, 0.0, 0.0]);
    }

    #[test]
    fn view_with_look_uses_the_frame_look_and_interpolated_position() {
        let mut c = Camera::new(Pose::default(), 70.0);
        c.push(Pose {
            pos: [2.0, 0.0, 0.0],
            yaw: 1.0,
            pitch: 0.5,
        });
        let v = c.view_with_look(0.5, 0.9, 9.0);
        assert_eq!(v.pos, [1.0, 0.0, 0.0]);
        assert_eq!(v.yaw, 0.9);
        assert_eq!(v.pitch, FRAC_PI_2);
    }

    #[test]
    fn fly_diagonal_is_no_faster_and_inputs_are_clamped() {
        let dist = |mf: f32, ms: f32| {
            let mut c = Camera::new(Pose::default(), 70.0);
            c.fly_tick(&TickInput {
                move_forward: mf,
                move_strafe: ms,
                ..TickInput::default()
            });
            let p = c.current().pos;
            (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
        };
        assert!(close(dist(1.0, 1.0), FLY_SPEED));
        assert!(close(dist(1.0, 0.0), FLY_SPEED));
        assert!(close(dist(5.0, 0.0), FLY_SPEED));
        assert!(close(dist(-9.0, 9.0), FLY_SPEED));
        assert!(close(dist(0.5, 0.0), FLY_SPEED * 0.5));
        assert_eq!(dist(f32::NAN, 0.0), 0.0);
    }

    #[test]
    fn fly_is_deterministic() {
        let run = || {
            let mut c = Camera::new(Pose::default(), 70.0);
            for i in 0..200 {
                let x = i as f32;
                c.fly_tick(&TickInput {
                    move_forward: (x * 0.1).sin(),
                    move_strafe: (x * 0.07).cos(),
                    yaw: x * 0.05,
                    pitch: (x * 0.03).sin(),
                    jump: i % 5 == 0,
                    sneak: i % 7 == 0,
                    ..TickInput::default()
                });
            }
            c
        };
        assert_eq!(run(), run());
    }
}
