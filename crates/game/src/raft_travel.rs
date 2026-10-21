//! The raft's travel between levels along the pathfound route
//! (world-layout.md Phase E, raft travel).
//!
//! `RaftTravel` walks a path of world points at a constant speed and yields
//! one `RaftFrame` per tick for `Raft::advance`. The raft never turns on the
//! way (quarter turns stay zero) and the frame carries no tilt, so riders are
//! carried by the existing `push_rider` logic and nothing else. The weekend
//! screen is not part of this.

use crate::raft::RaftFrame;

const DEFAULT_RAFT: &str = include_str!("../../../data/raft.toml");

/// Blocks per tick from `[travel] speed` in `data/raft.toml`.
pub fn default_speed() -> Result<f64, String> {
    parse_speed(DEFAULT_RAFT)
}

pub fn parse_speed(text: &str) -> Result<f64, String> {
    let root: toml::Value = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let speed = match root.get("travel").and_then(|t| t.get("speed")) {
        Some(toml::Value::Float(f)) => *f,
        Some(toml::Value::Integer(i)) => *i as f64,
        Some(_) => return Err("travel.speed must be a number".into()),
        None => return Err("missing travel.speed".into()),
    };
    if !speed.is_finite() || speed <= 0.0 {
        return Err("travel.speed must be a positive number".into());
    }
    Ok(speed)
}

/// World positions at the centre of each column of a path from `find_path`.
/// `origin` is the world `[x, z]` of the local column `[0, 0]` of the map the
/// path was found on.
pub fn column_centres(path: &[[i32; 2]], origin: [i32; 2]) -> Vec<[f64; 2]> {
    path.iter()
        .map(|c| {
            [
                (origin[0] + c[0]) as f64 + 0.5,
                (origin[1] + c[1]) as f64 + 0.5,
            ]
        })
        .collect()
}

/// A raft moving along a polyline at constant speed.
#[derive(Clone, Debug)]
pub struct RaftTravel {
    points: Vec<[f64; 2]>,
    speed: f64,
    /// Where the raft is now.
    pos: [f64; 2],
    /// Index of the point it is heading for.
    next: usize,
}

impl RaftTravel {
    /// Travel along `points` at `speed` blocks per tick, starting at the
    /// first point.
    pub fn new(points: Vec<[f64; 2]>, speed: f64) -> RaftTravel {
        let pos = points.first().copied().unwrap_or([0.0; 2]);
        RaftTravel {
            points,
            speed,
            pos,
            next: 1,
        }
    }

    /// Travel at the speed in `data/raft.toml`.
    pub fn from_data(points: Vec<[f64; 2]>) -> Result<RaftTravel, String> {
        Ok(RaftTravel::new(points, default_speed()?))
    }

    pub fn position(&self) -> [f64; 2] {
        self.pos
    }

    pub fn is_done(&self) -> bool {
        self.next >= self.points.len()
    }

    /// The motion for the next tick, or `None` once the last point is reached.
    /// Corners are passed without losing speed; the last tick is as short as
    /// it needs to be to stop on the final point.
    pub fn step(&mut self) -> Option<RaftFrame> {
        if self.is_done() {
            return None;
        }
        let start = self.pos;
        let mut left = self.speed;
        while let Some(&target) = self.points.get(self.next) {
            let (dx, dz) = (target[0] - self.pos[0], target[1] - self.pos[1]);
            let dist = dx.hypot(dz);
            if dist <= left {
                self.pos = target;
                left -= dist;
                self.next += 1;
            } else {
                self.pos = [
                    self.pos[0] + dx / dist * left,
                    self.pos[1] + dz / dist * left,
                ];
                break;
            }
        }
        Some(RaftFrame {
            displacement: [self.pos[0] - start[0], 0.0, self.pos[1] - start[1]],
            ..RaftFrame::default()
        })
    }
}

#[cfg(test)]
#[path = "raft_travel_tests.rs"]
mod tests;
