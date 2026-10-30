//! Streaming of terrain regions around the player.
//! See docs/design/world-layout.md Phase D (Multi-region world).

use crate::walk::RegionSet;
use layout::{CacheChange, Corridor, Layout, RegionCache, RegionCoord};

const DEFAULT_STREAMING: &str = include_str!("../../../data/streaming.toml");

/// Load and unload radii (in regions, Chebyshev) and whether to decorate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamConfig {
    pub load: i32,
    pub unload: i32,
    pub decorate: bool,
}

impl StreamConfig {
    /// The shipped defaults from `data/streaming.toml`.
    pub fn default_from_data() -> Result<StreamConfig, String> {
        StreamConfig::parse(DEFAULT_STREAMING)
    }

    /// Parse streaming text; every key is required and `unload >= load >= 0`.
    pub fn parse(text: &str) -> Result<StreamConfig, String> {
        let root: toml::Value = text
            .parse()
            .map_err(|e: toml::de::Error| format!("streaming: {e}"))?;
        let radius = |key: &str| -> Result<i32, String> {
            let v = root
                .get("radii")
                .and_then(|t| t.get(key))
                .and_then(|v| v.as_integer())
                .ok_or_else(|| format!("streaming: missing integer radii.{key}"))?;
            i32::try_from(v).map_err(|_| format!("streaming: radii.{key} out of range"))
        };
        let decorate = root
            .get("generation")
            .and_then(|t| t.get("decorate"))
            .and_then(|v| v.as_bool())
            .ok_or("streaming: missing boolean generation.decorate")?;
        let (load, unload) = (radius("load")?, radius("unload")?);
        if load < 0 || unload < load {
            return Err(format!(
                "streaming: need 0 <= load <= unload, got {load} and {unload}"
            ));
        }
        Ok(StreamConfig {
            load,
            unload,
            decorate,
        })
    }
}

/// Keeps a [`RegionSet`] in step with a [`RegionCache`] as the player moves.
pub struct Streamer {
    cache: RegionCache,
    config: StreamConfig,
    centre: Option<RegionCoord>,
    pending: CacheChange,
}

impl Streamer {
    pub fn new(layout: Layout, config: StreamConfig) -> Streamer {
        Streamer {
            cache: RegionCache::new(layout, config.decorate),
            config,
            centre: None,
            pending: CacheChange::default(),
        }
    }

    /// Regions loaded and unloaded since the last call, for a renderer that
    /// mirrors what is loaded. A region loaded and unloaded in between is in
    /// neither list. Changes accumulate until this is called, so a caller
    /// that streams for long without a renderer should drain now and then.
    pub fn drain_changes(&mut self) -> CacheChange {
        std::mem::take(&mut self.pending)
    }

    /// Streams only regions inside `corridor` (all regions with `None`). Takes
    /// effect at the next `update`, even if the player has not changed region.
    pub fn set_corridor(&mut self, corridor: Option<Corridor>) {
        self.cache.set_corridor(corridor);
        self.centre = None;
    }

    pub fn cache(&self) -> &RegionCache {
        &self.cache
    }

    pub fn centre(&self) -> Option<RegionCoord> {
        self.centre
    }

    /// Streams around the region holding `feet`. Does nothing while the
    /// player stays in the same region as last time.
    pub fn update(&mut self, feet: [f64; 3], world: &mut RegionSet) {
        let centre = RegionCoord::containing(feet[0].floor() as i32, feet[2].floor() as i32);
        if self.centre == Some(centre) {
            return;
        }
        self.centre = Some(centre);
        let change = self
            .cache
            .update(centre, self.config.load, self.config.unload);
        for c in &change.unloaded {
            let (x0, z0) = c.origin();
            world.remove(x0, z0);
            match self.pending.loaded.iter().position(|l| l == c) {
                Some(i) => {
                    self.pending.loaded.remove(i);
                }
                None => self.pending.unloaded.push(*c),
            }
        }
        self.pending.loaded.extend(&change.loaded);
        for c in &change.loaded {
            if let Some(r) = self.cache.get(*c) {
                world.push(r.region.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_error::test_support::assert_err_contains;
    use crate::input::TickInput;
    use crate::walk::Walker;
    use layout::REGION_SIZE;

    #[test]
    fn shipped_radii_parse() {
        let c = StreamConfig::default_from_data().unwrap();
        assert_eq!((c.load, c.unload), (1, 2));
        assert!(c.decorate);
    }

    #[test]
    fn bad_radii_are_rejected() {
        let swap = |from: &str, to: &str| DEFAULT_STREAMING.replace(from, to);
        assert_err_contains(
            StreamConfig::parse(&swap("unload = 2", "unload = 0")),
            "need 0 <= load <= unload",
        );
        assert_err_contains(
            StreamConfig::parse(&swap("load = 1", "load = -1")),
            "need 0 <= load <= unload",
        );
        assert_err_contains(
            StreamConfig::parse(&swap("load = 1", "load = \"x\"")),
            "missing integer radii.load",
        );
        assert_err_contains(
            StreamConfig::parse(&swap("decorate = true", "")),
            "missing boolean generation.decorate",
        );
    }

    fn layout() -> Layout {
        Layout {
            seed: 25,
            ..Layout::default()
        }
    }

    /// Feet just above the terrain at (64, 64), so a settling fall is not fatal.
    fn spawn() -> [f64; 3] {
        let ground = terrain::generate_decorated(25, 64, 64, 1, 1);
        let top = (0..terrain::HEIGHT as u32)
            .rev()
            .find(|&y| ground.get(0, y, 0) != terrain::Block::Air)
            .unwrap();
        [64.5, top as f64 + 1.0, 64.5]
    }

    /// Walk towards +x, hopping over small obstacles.
    fn east() -> TickInput {
        TickInput {
            move_forward: 1.0,
            yaw: std::f32::consts::FRAC_PI_2,
            jump: true,
            ..TickInput::default()
        }
    }

    fn blocks_at(w: &Walker, x: i32, y: usize, z: i32) -> u8 {
        let c = RegionCoord::containing(x, z);
        let (x0, z0) = c.origin();
        let size = REGION_SIZE as i32;
        w.region_blocks(c).unwrap()[((z - z0) * size + (x - x0)) as usize * terrain::HEIGHT + y]
    }

    #[test]
    fn region_blocks_show_edits_only_where_they_were_made() {
        let mut w = Walker::with_layout(layout(), [64.5, 120.0, 64.5]).unwrap();
        let before = w.region_blocks(RegionCoord::new(0, 0)).unwrap();
        let other = w.region_blocks(RegionCoord::new(1, 0)).unwrap();
        assert_eq!(w.region_blocks(RegionCoord::new(9, 9)), None);
        let held = blocks_at(&w, 10, 70, 20);
        assert!(w.world.set_block(10, 70, 20, terrain::Block::Dandelion));
        assert_eq!(blocks_at(&w, 10, 70, 20), terrain::Block::Dandelion as u8);
        assert!(w.world.remove_block(10, 70, 20));
        assert_eq!(blocks_at(&w, 10, 70, 20), 0);
        let after = w.region_blocks(RegionCoord::new(0, 0)).unwrap();
        let differing = before.iter().zip(&after).filter(|(a, b)| a != b).count();
        assert_eq!(differing, usize::from(held != 0));
        assert_eq!(w.region_blocks(RegionCoord::new(1, 0)).unwrap(), other);
    }

    #[test]
    fn an_edit_on_a_border_is_seen_by_the_neighbours_padding() {
        let mut w = Walker::with_layout(layout(), [64.5, 120.0, 64.5]).unwrap();
        w.drain_edited_regions();
        w.world.remove_block(127, 70, 5);
        assert_eq!(blocks_at(&w, 127, 70, 5), 0);
        assert_eq!(
            w.drain_edited_regions(),
            vec![RegionCoord::new(0, 0), RegionCoord::new(1, 0)]
        );
        w.world.remove_block(50, 70, 50);
        assert_eq!(w.drain_edited_regions(), vec![RegionCoord::new(0, 0)]);
        assert!(w.drain_edited_regions().is_empty());
    }

    #[test]
    fn region_border_matches_region_blocks_with_border_edits() {
        let mut w = Walker::with_layout(layout(), [64.5, 120.0, 64.5]).unwrap();
        let c = RegionCoord::new(0, 0);
        let corners = [(0, 0), (127, 0), (0, 127), (127, 127)];
        let edges = [(0, 40), (127, 41), (42, 0), (43, 127)];
        for (x, z) in corners.into_iter().chain(edges) {
            assert!(w.world.set_block(x, 120, z, terrain::Block::Dandelion));
        }
        assert!(w.world.set_block(-1, 120, 40, terrain::Block::Dandelion));
        let all = w.region_blocks(c).unwrap();
        let size = REGION_SIZE as i32;
        let h = terrain::HEIGHT;
        for dz in -1..=1 {
            for dx in -1..=1 {
                if (dx, dz) == (0, 0) {
                    assert_eq!(w.region_border(c, dx, dz), None);
                    continue;
                }
                let span = |d: i32| match d {
                    -1 => vec![size - 1],
                    0 => (0..size).collect(),
                    _ => vec![0],
                };
                let mut want = Vec::new();
                for lz in span(dz) {
                    for lx in span(dx) {
                        let i = (lz * size + lx) as usize * h;
                        want.extend_from_slice(&all[i..i + h]);
                    }
                }
                let got = w.region_border(c, dx, dz).unwrap();
                assert_eq!(got.len(), want.len(), "({dx}, {dz})");
                assert!(got == want, "({dx}, {dz})");
                assert!(got.contains(&(terrain::Block::Dandelion as u8)), "({dx}, {dz})");
                assert!(w.region_border(RegionCoord::new(9, 9), dx, dz).is_none());
            }
        }
    }

    #[test]
    fn construction_loads_the_neighbours_and_the_player_lands_on_terrain() {
        let mut w = Walker::with_layout(layout(), [64.5, 120.0, 64.5]).unwrap();
        assert_eq!(w.loaded_region_count(), 9);
        for _ in 0..200 {
            w.tick(&TickInput::default());
        }
        let p = w.player();
        assert!(p.on_ground, "{p:?}");
        let ground = terrain::generate_decorated(25, 64, 64, 1, 1);
        let top = (0..terrain::HEIGHT as u32)
            .rev()
            .find(|&y| ground.get(0, y, 0) != terrain::Block::Air)
            .unwrap();
        assert!(p.pos[1] >= top as f64 && p.pos[1] < 120.0, "{p:?} {top}");
    }

    #[test]
    fn walking_streams_regions_and_stays_within_the_bound() {
        let mut w = Walker::with_layout(layout(), spawn()).unwrap();
        let c = StreamConfig::default_from_data().unwrap();
        let bound = (2 * c.unload as usize + 1).pow(2);
        let mut seen = std::collections::BTreeSet::new();
        let input = east();
        for _ in 0..2500 {
            w.tick(&input);
            assert!(w.loaded_region_count() <= bound);
            seen.insert(w.streamed_centre());
            if w.player().pos[0] > 64.5 + 300.0 {
                break;
            }
        }
        assert!(w.player().pos[0] > 64.5 + 300.0, "{:?}", w.player());
        let centre = w.streamed_centre().unwrap();
        assert_eq!(
            centre.rx,
            ((w.player().pos[0].floor() as i32) / REGION_SIZE as i32)
        );
        assert!(seen.len() >= 3);
        let cache = w.streamer_cache().unwrap();
        assert!(cache.generations() > 9, "new regions loaded ahead");
        assert_eq!(cache.len(), w.loaded_region_count());
        assert!(!cache.contains(RegionCoord::new(centre.rx - 3, centre.rz)));
    }

    #[test]
    fn drained_changes_mirror_the_loaded_regions() {
        let mut w = Walker::with_layout(layout(), [64.5, 120.0, 64.5]).unwrap();
        let first = w.drain_stream_changes();
        assert_eq!((first.loaded.len(), first.unloaded.len()), (9, 0));
        assert_eq!(w.drain_stream_changes(), Default::default());
        let mut mirror: std::collections::BTreeSet<_> = first.loaded.into_iter().collect();
        let input = east();
        for _ in 0..1000 {
            w.tick(&input);
            let change = w.drain_stream_changes();
            for c in change.unloaded {
                assert!(mirror.remove(&c), "{c:?} was not loaded");
            }
            for c in change.loaded {
                assert!(mirror.insert(c), "{c:?} loaded twice");
            }
        }
        let loaded = w.loaded_regions().into_iter().collect::<Vec<_>>();
        let origins = mirror.iter().map(|c| c.origin());
        assert_eq!(
            origins.collect::<std::collections::BTreeSet<_>>().len(),
            loaded.len()
        );
        assert!(mirror.iter().all(|c| loaded.contains(&c.origin())));
    }

    #[test]
    fn streamed_walk_matches_one_large_region() {
        let l = layout();
        let mut streamed = Walker::with_layout(l.clone(), spawn()).unwrap();
        let mut single = Walker::new(spawn()).unwrap();
        single.add_region(l.generate_layout_region(-256, -256, 1024, 640, true).region);
        let input = east();
        for i in 0..1800 {
            streamed.tick(&input);
            single.tick(&input);
            assert_eq!(streamed.player().pos, single.player().pos, "tick {i}");
        }
        assert!(streamed.player().pos[0] > 64.5 + 300.0);
    }

    #[test]
    fn corridor_keeps_streaming_inside_it() {
        let size = REGION_SIZE as f64;
        let corridor = Corridor::new(
            &layout::Route {
                points: vec![[64.5, 64.5], [size * 4.0, 64.5]],
            },
            0,
        );
        let mut world = RegionSet::default();
        let mut s = Streamer::new(layout(), StreamConfig::default_from_data().unwrap());
        s.set_corridor(Some(corridor.clone()));
        for x in (64..(size as i32 * 4)).step_by(32) {
            s.update([x as f64, 100.0, 64.5], &mut world);
            assert!(s.cache().len() <= corridor.regions().len());
        }
        let ch = s.drain_changes();
        assert!(!ch.loaded.is_empty());
        assert!(ch.loaded.iter().all(|&c| corridor.contains(c)));
        assert!(s.cache().generations() <= corridor.regions().len());
    }
}
