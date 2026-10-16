//! Samples the shipped habitat rules over seeds 1..=N (default 50) on the v0
//! layout: per habitat, its share of water columns and the number of seeds
//! it appears in, plus the lowest biased temperature seen in `warm_shallows`
//! columns. Usage: `habitat_stats [N] [SIZE]` (SIZE is the square side).
use layout::{build_layout, default_v0_level, Habitats, WorldConfig, NO_HABITAT};

fn main() {
    let mut args = std::env::args().skip(1);
    let n: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(50);
    let size: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(96);
    let habitats = Habitats::shipped();
    let rules = habitats.rules.len();
    let warm = habitats.id_of("warm_shallows").map(|h| h.0);
    let mut columns = vec![0u64; rules];
    let mut seeds = vec![0u64; rules];
    let mut warm_min = f32::INFINITY;
    let mut water = 0u64;
    for seed in 1..=n {
        let level = default_v0_level(seed);
        let l = build_layout(&WorldConfig { seed }, std::slice::from_ref(&level));
        let half = (size / 2) as i32;
        let m = l.habitat_map_with(&habitats, -half, -half, size, size);
        let mut seen = vec![false; rules];
        for (i, &h) in m.habitat.iter().enumerate() {
            if h == NO_HABITAT {
                continue;
            }
            water += 1;
            columns[h as usize] += 1;
            seen[h as usize] = true;
            if Some(h as usize) == warm {
                warm_min = warm_min.min(m.temperature[i]);
            }
        }
        for (s, &v) in seeds.iter_mut().zip(&seen) {
            *s += v as u64;
        }
    }
    println!("seeds 1..={n}, {size}x{size} columns each, {water} water columns");
    for (i, r) in habitats.rules.iter().enumerate() {
        println!(
            "{:<15} {:>6.2}% of water, in {}/{} seeds",
            r.id,
            100.0 * columns[i] as f64 / water.max(1) as f64,
            seeds[i],
            n
        );
    }
    if warm_min.is_finite() {
        println!("lowest warm_shallows temperature: {warm_min:.3}");
    }
}
