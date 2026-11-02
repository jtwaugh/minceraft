//! The player's fishing line: at most one bobber, cast and reeled in.
//! See docs/design/fish-and-fishing.md sections 4.2 and 4.6.
//!
//! No bites yet: every reel brings in an empty line.

use crate::bobber::Bobber;
use crate::collision::CollisionWorld;
use crate::events::GameEvent;
use crate::fishing_config::FishingConfig;
use crate::item_entities::ItemEntityConfig;

#[derive(Clone, Debug)]
pub struct Fishing {
    cfg: FishingConfig,
    item_cfg: ItemEntityConfig,
    bobber: Option<Bobber>,
}

impl Fishing {
    pub fn new(cfg: FishingConfig, item_cfg: ItemEntityConfig) -> Fishing {
        Fishing {
            cfg,
            item_cfg,
            bobber: None,
        }
    }

    /// Whether a bobber is out.
    pub fn line_out(&self) -> bool {
        self.bobber.is_some()
    }

    pub fn bobber(&self) -> Option<&Bobber> {
        self.bobber.as_ref()
    }

    /// Cast from `eye` along the unit vector `look`. Does nothing with a
    /// line already out.
    pub fn cast(&mut self, eye: [f64; 3], look: [f64; 3], events: &mut Vec<GameEvent>) {
        if self.bobber.is_some() {
            return;
        }
        self.bobber = Some(Bobber::cast(self.item_cfg, &self.cfg, eye, look));
        events.push(GameEvent::LineCast {
            player: "player".to_string(),
            position: eye,
        });
    }

    /// Bring the line in with nothing on it.
    pub fn reel(&mut self, events: &mut Vec<GameEvent>) {
        if self.bobber.take().is_some() {
            events.push(GameEvent::LineReeled {
                player: "player".to_string(),
                caught: false,
            });
        }
    }

    /// Advance the bobber one tick, then reel in if the rod is no longer in
    /// hand or the player at `player` is beyond `max_distance`.
    pub fn tick(
        &mut self,
        world: &impl CollisionWorld,
        rod_in_hand: bool,
        player: [f64; 3],
        events: &mut Vec<GameEvent>,
    ) {
        let Some(b) = &mut self.bobber else { return };
        b.tick(world);
        let d: f64 = (0..3).map(|i| (b.pos()[i] - player[i]).powi(2)).sum();
        if !rod_in_hand || d > self.cfg.max_distance * self.cfg.max_distance {
            self.reel(events);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;

    fn fishing() -> Fishing {
        Fishing::new(
            FishingConfig::default_from_data().unwrap(),
            ItemEntityConfig::default_from_data(),
        )
    }

    #[test]
    fn one_bobber_at_a_time() {
        let mut f = fishing();
        let mut ev = Vec::new();
        f.cast([0.0, 5.0, 0.0], [1.0, 0.0, 0.0], &mut ev);
        f.cast([9.0, 5.0, 0.0], [1.0, 0.0, 0.0], &mut ev);
        assert_eq!(ev.len(), 1);
        assert!(f.line_out());
        f.reel(&mut ev);
        f.reel(&mut ev);
        assert!(!f.line_out());
        assert_eq!(ev.len(), 2);
        assert_eq!(
            ev[1],
            GameEvent::LineReeled {
                player: "player".into(),
                caught: false
            }
        );
    }

    #[test]
    fn putting_the_rod_away_or_walking_off_reels_in() {
        let w = MemoryWorld::new();
        let mut f = fishing();
        let mut ev = Vec::new();
        f.cast([0.0, 5.0, 0.0], [1.0, 0.0, 0.0], &mut ev);
        f.tick(&w, true, [0.0, 5.0, 0.0], &mut ev);
        assert!(f.line_out());
        f.tick(&w, false, [0.0, 5.0, 0.0], &mut ev);
        assert!(!f.line_out());
        f.cast([0.0, 5.0, 0.0], [1.0, 0.0, 0.0], &mut ev);
        f.tick(&w, true, [500.0, 5.0, 0.0], &mut ev);
        assert!(!f.line_out());
    }
}
