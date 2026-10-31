//! Timing + terrain / biome / vegetation statistics + optional PPM render.
//!
//! cargo run -p terrain --profile bench-native --example stats -- [seed] [x0] [z0] [out.ppm]

use std::time::Instant;
use terrain::*;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(12345);
    let x0: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let z0: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let out = args.get(4).cloned();
    let (w, d) = (512u32, 512u32);
    let n = (w * d) as usize;

    // ---- timings ----
    let t = Instant::now();
    let _ = generate_heightmap(seed, x0, z0, w, d);
    println!("heightmap 512x512 (terrain only): {:.1} ms", ms(t));
    let t = Instant::now();
    let terrain = generate_region(seed, x0, z0, w, d);
    println!("generate_region 512x512 (terrain only): {:.1} ms", ms(t));
    let t = Instant::now();
    let (r, _) = generate_decorated_with_stats(seed, x0, z0, w, d, &Biases::default());
    println!(
        "generate_decorated 512x512 (terrain + margin + decoration): {:.1} ms",
        ms(t)
    );
    let t = Instant::now();
    let hm = generate_heightmap_with_biases(seed, x0, z0, w, d, &Biases::default(), true);
    println!("heightmap 512x512 (decorated): {:.1} ms", ms(t));
    drop(terrain);

    // ---- surface ----
    let mut top_counts = [0usize; Block::COUNT];
    let mut all = [0usize; Block::COUNT];
    let mut water_cols = 0;
    for b in &r.blocks {
        all[*b as usize] += 1;
    }
    for z in 0..d {
        for x in 0..w {
            let i = r.index(x, 0, z);
            let (tb, _, _, _) = column_summary(&r.blocks[i..i + 128]);
            top_counts[tb as usize] += 1;
            if tb == Block::Water as u8 || tb == Block::Ice as u8 || tb == Block::FloeIce as u8 {
                water_cols += 1;
            }
        }
    }
    println!(
        "water/ice columns: {:.1}%",
        100.0 * water_cols as f64 / n as f64
    );
    println!(
        "sea ice columns: ice={:.2}% floe_ice={:.2}%",
        100.0 * top_counts[Block::Ice as usize] as f64 / n as f64,
        100.0 * top_counts[Block::FloeIce as usize] as f64 / n as f64
    );
    print!("top block:");
    for (i, c) in top_counts.iter().enumerate() {
        if *c > 0 {
            print!(
                " {}={:.2}%",
                BLOCK_INFO[i].name,
                100.0 * *c as f64 / n as f64
            );
        }
    }
    println!();
    print!("block counts (decorated region):");
    for (i, c) in all.iter().enumerate().skip(7) {
        if *c > 0 {
            print!(" {}={}", BLOCK_INFO[i].name, c);
        }
    }
    println!();

    // ---- biome distribution over a 4096 x 4096 climate grid ----
    let t = Instant::now();
    let climate = Climate::new(seed);
    let mut hist = [0u64; Biome::COUNT];
    let side = 4096;
    for z in 0..side {
        for x in 0..side {
            let (tt, hh) = climate.raw(x0 + x, z0 + z);
            hist[biome_for(tt, hh) as usize] += 1;
        }
    }
    let total = (side * side) as f64;
    println!(
        "biome distribution over {side}x{side} columns ({:.0} ms):",
        ms(t)
    );
    let mut order: Vec<usize> = (0..Biome::COUNT).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(hist[i]));
    for i in order {
        println!(
            "  {:<16} {:6.2}%",
            BIOME_INFO[i].name,
            100.0 * hist[i] as f64 / total
        );
    }

    // ---- trees per chunk per biome over a 2048 x 2048 decorated area ----
    let t = Instant::now();
    let mut stats = DecorationStats::default();
    let flora_blocks: Vec<(&str, u8)> = flora::FLORA
        .iter()
        .map(|f| (f.name, f.block as u8))
        .collect();
    let mut flora_counts = vec![[0u64; Biome::COUNT]; flora_blocks.len()];
    for tz in 0..4 {
        for tx in 0..4 {
            let (reg, s) = generate_decorated_with_stats(
                seed,
                x0 + tx * 512,
                z0 + tz * 512,
                512,
                512,
                &Biases::default(),
            );
            for (i, &b) in reg.blocks.iter().enumerate() {
                if let Some(f) = flora_blocks.iter().position(|e| e.1 == b) {
                    let col = i / HEIGHT;
                    let (x, z) = (col as i32 % 512, col as i32 / 512);
                    let (tt, hh) = climate.raw(x0 + tx * 512 + x, z0 + tz * 512 + z);
                    flora_counts[f][biome_for(tt, hh) as usize] += 1;
                }
            }
            for b in 0..Biome::COUNT {
                let (a, c) = (&mut stats.biomes[b], s.biomes[b]);
                a.columns += c.columns;
                a.tree_attempts += c.tree_attempts;
                a.trees += c.trees;
                a.plants += c.plants;
                a.snow += c.snow;
            }
        }
    }
    println!(
        "vegetation over 2048x2048 ({:.0} ms), per 256 columns of each biome:",
        ms(t)
    );
    println!(
        "  {:<16} {:>8} {:>9} {:>7} {:>8} {:>7}",
        "biome", "chunks", "attempts", "trees", "plants", "snow%"
    );
    for b in Biome::ALL {
        let s = stats.biomes[b as usize];
        if s.columns == 0 {
            continue;
        }
        let per = 256.0 / s.columns as f64;
        println!(
            "  {:<16} {:>8.0} {:>9.2} {:>7.2} {:>8.1} {:>6.1}%",
            b.name(),
            s.columns as f64 / 256.0,
            stats.attempts_per_chunk(b),
            stats.trees_per_chunk(b),
            s.plants as f64 * per,
            100.0 * s.snow as f64 / s.columns as f64
        );
    }

    println!("flora blocks per biome over 2048x2048 (lower half of tall plants; fern also counts Beta's):");
    for (f, (name, _)) in flora_blocks.iter().enumerate() {
        print!("  {name:<18}");
        for b in Biome::ALL {
            let c = flora_counts[f][b as usize];
            if c > 0 {
                print!(" {}={}", b.name(), c);
            }
        }
        println!();
    }

    if let Some(path) = out {
        let mut img = Vec::with_capacity(n * 3 + 32);
        img.extend_from_slice(format!("P6 {w} {d} 255\n").as_bytes());
        for z in 0..d as usize {
            for x in 0..w as usize {
                let i = (z * w as usize + x) * 4;
                let (tb, ty) = (hm[i], hm[i + 1] as f64);
                let west = if x > 0 { hm[i - 4 + 1] as f64 } else { ty };
                let shade = (1.0 + (ty - west) * 0.08).clamp(0.6, 1.3);
                let c = BLOCK_INFO[tb as usize].color;
                for v in c {
                    img.push((v as f64 * shade).clamp(0.0, 255.0) as u8);
                }
            }
        }
        std::fs::write(&path, img).unwrap();
        println!("wrote {path}");
    }
}
