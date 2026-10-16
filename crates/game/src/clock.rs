//! Fixed-step clock: the sim runs at 20 ticks per second whatever the frame rate.
//! See docs/design/basic-gameplay.md section 1, rule 1.

/// Sim ticks per second.
pub const TICKS_PER_SECOND: u32 = 20;

/// Seconds per tick.
pub const TICK_SECONDS: f64 = 1.0 / TICKS_PER_SECOND as f64;

/// Most ticks one frame may run. A frame longer than this many ticks (0.5 s)
/// drops the excess time instead of spiralling: the sim falls behind real time
/// rather than freezing the renderer. Frames up to 100 ms never reach the cap.
pub const MAX_TICKS_PER_FRAME: u32 = 10;

/// What a frame should do: run `ticks` sim ticks, then render at `alpha`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub ticks: u32,
    /// Fraction of the way from the previous tick to the current one, 0..1.
    pub alpha: f32,
}

/// Accumulates real frame time and converts it to whole ticks.
#[derive(Clone, Debug, Default)]
pub struct Clock {
    accumulated: f64,
}

impl Clock {
    pub fn new() -> Clock {
        Clock::default()
    }

    /// Advance by one frame of `dt` seconds. Negative or non-finite `dt`
    /// counts as zero.
    pub fn advance(&mut self, dt: f64) -> Step {
        if dt.is_finite() && dt > 0.0 {
            self.accumulated += dt;
        }
        let mut ticks = 0;
        while self.accumulated >= TICK_SECONDS && ticks < MAX_TICKS_PER_FRAME {
            self.accumulated -= TICK_SECONDS;
            ticks += 1;
        }
        if ticks == MAX_TICKS_PER_FRAME {
            // Hit the cap: forget the backlog, keep the sub-tick remainder.
            self.accumulated %= TICK_SECONDS;
        }
        let alpha = (self.accumulated / TICK_SECONDS).clamp(0.0, 1.0) as f32;
        // f32 rounding can land exactly on 1.0; keep alpha strictly below it.
        let alpha = if alpha >= 1.0 {
            1.0 - f32::EPSILON
        } else {
            alpha
        };
        Step { ticks, alpha }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simulate(frame: f64, frames: u32) -> u64 {
        let mut c = Clock::new();
        let mut ticks = 0u64;
        for _ in 0..frames {
            let s = c.advance(frame);
            assert!((0.0..1.0).contains(&s.alpha), "alpha {}", s.alpha);
            assert!(s.ticks <= MAX_TICKS_PER_FRAME);
            ticks += s.ticks as u64;
        }
        ticks
    }

    #[test]
    fn averages_twenty_ticks_per_second_for_all_frame_times() {
        for ms in 1..=100 {
            let frame = ms as f64 / 1000.0;
            let frames = (100.0 / frame) as u32;
            let ticks = simulate(frame, frames);
            let expected = frames as f64 * frame * TICKS_PER_SECOND as f64;
            assert!(
                (ticks as f64 - expected).abs() <= 2.0,
                "{ms} ms: {ticks} ticks, expected {expected}"
            );
        }
    }

    #[test]
    fn catch_up_is_capped_and_backlog_dropped() {
        let mut c = Clock::new();
        let s = c.advance(5.0);
        assert_eq!(s.ticks, MAX_TICKS_PER_FRAME);
        assert!((0.0..1.0).contains(&s.alpha));
        // No backlog left over for the next frame.
        assert!(c.advance(0.001).ticks <= 1);
    }

    #[test]
    fn short_frame_runs_no_tick_and_alpha_grows() {
        let mut c = Clock::new();
        let a = c.advance(0.01);
        assert_eq!(a.ticks, 0);
        assert!((a.alpha - 0.2).abs() < 1e-5);
        let b = c.advance(0.045);
        assert_eq!(b.ticks, 1);
    }

    #[test]
    fn bad_dt_is_ignored() {
        let mut c = Clock::new();
        for dt in [-1.0, f64::NAN, f64::INFINITY] {
            let s = c.advance(dt);
            assert_eq!((s.ticks, s.alpha), (0, 0.0));
        }
    }
}
