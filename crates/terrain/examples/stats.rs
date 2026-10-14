//! Timing + terrain statistics + optional PPM heightmap render.
//!
//! cargo run -p terrain --profile bench-native --example stats -- [seed] [x0] [z0] [out.ppm]

use std::time::Instant;
use terrain::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(12345);
    let x0: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let z0: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let out = args.get(4).cloned();
    let (w, d) = (512u32, 512u32);

    let t = Instant::now();
    let hm = generate_heightmap(seed, x0, z0, w, d);
    println!("heightmap 512x512: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    let r = generate_region(seed, x0, z0, w, d);
    println!("generate_region 512x512: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);

    let n = (w * d) as usize;
    let mut water_cols = 0;
    let mut hist = [0usize; 16];
    let mut top_counts = [0usize; 7];
    let mut overhang_cols = 0;
    let mut oh_bins = [0usize; 64];
    let mut sand_band = 0;
    let (mut hmin, mut hmax, mut hsum) = (255u32, 0u32, 0u64);
    let mut blocks = [0usize; 7];
    for b in &r.blocks {
        blocks[*b as usize] += 1;
    }
    for z in 0..d {
        for x in 0..w {
            let i = r.index(x, 0, z);
            let col = &r.blocks[i..i + 128];
            let (tb, ty, _fy, _) = column_summary(col);
            top_counts[tb as usize] += 1;
            if tb == Block::Water as u8 {
                water_cols += 1;
            }
            let ty = ty as u32;
            hist[(ty / 8) as usize] += 1;
            hmin = hmin.min(ty);
            hmax = hmax.max(ty);
            hsum += ty as u64;
            // Overhang: an air block above y=64 that has a solid block above it.
            let mut seen_solid = false;
            let mut oh = false;
            for y in (64..128).rev() {
                let b = col[y];
                if b != 0 && b != 6 {
                    seen_solid = true;
                } else if b == 0 && seen_solid {
                    oh = true;
                    break;
                }
            }
            if oh {
                overhang_cols += 1;
                oh_bins[(z / 64 * 8 + x / 64) as usize] += 1;
            }
            if tb == Block::Sand as u8 && (60..=65).contains(&ty) {
                sand_band += 1;
            }
        }
    }
    println!("water columns: {:.1}%", 100.0 * water_cols as f64 / n as f64);
    println!(
        "surface height: min {hmin} max {hmax} mean {:.1}",
        hsum as f64 / n as f64
    );
    print!("height histogram (8-block bins from 0):");
    for (i, h) in hist.iter().enumerate() {
        if *h > 0 {
            print!(" [{}-{}]:{:.1}%", i * 8, i * 8 + 7, 100.0 * *h as f64 / n as f64);
        }
    }
    println!();
    let names = ["air", "stone", "dirt", "grass", "sand", "gravel", "water"];
    print!("top block:");
    for i in 0..7 {
        print!(" {}={:.1}%", names[i], 100.0 * top_counts[i] as f64 / n as f64);
    }
    println!();
    print!("all blocks:");
    for i in 0..7 {
        print!(" {}={:.2}%", names[i], 100.0 * blocks[i] as f64 / r.blocks.len() as f64);
    }
    println!();
    println!(
        "overhang columns (air under solid above y=64): {} ({:.2}%)",
        overhang_cols,
        100.0 * overhang_cols as f64 / n as f64
    );
    let (bi, bn) = oh_bins.iter().enumerate().max_by_key(|e| e.1).unwrap();
    println!(
        "densest overhang 64x64 tile: region offset ({}, {}) with {} columns",
        bi % 8 * 64,
        bi / 8 * 64,
        bn
    );
    println!("sand-topped columns at y 60..65: {sand_band}");

    if let Some(path) = out {
        let mut img = Vec::with_capacity(n * 3 + 32);
        img.extend_from_slice(format!("P6 {w} {d} 255\n").as_bytes());
        for z in 0..d as usize {
            for x in 0..w as usize {
                let i = (z * w as usize + x) * 4;
                let (tb, ty, fy) = (hm[i], hm[i + 1] as f64, hm[i + 2] as f64);
                let west = if x > 0 { hm[i - 4 + 1] as f64 } else { ty };
                let north = if z > 0 { hm[i - 4 * w as usize + 1] as f64 } else { ty };
                let shade = (1.0 + (ty - west) * 0.08 + (ty - north) * 0.08).clamp(0.5, 1.4)
                    * (0.7 + (ty - 40.0) / 160.0);
                let base: [f64; 3] = match tb {
                    1 => [128.0, 128.0, 128.0],
                    2 => [134.0, 96.0, 67.0],
                    3 => [95.0, 159.0, 53.0],
                    4 => [219.0, 207.0, 142.0],
                    5 => [136.0, 126.0, 126.0],
                    6 => {
                        let depth = (ty - fy).min(30.0) / 30.0;
                        [40.0 - 20.0 * depth, 90.0 - 50.0 * depth, 200.0 - 70.0 * depth]
                    }
                    _ => [0.0, 0.0, 0.0],
                };
                let s = if tb == 6 { 1.0 } else { shade };
                for c in base {
                    img.push((c * s).clamp(0.0, 255.0) as u8);
                }
            }
        }
        std::fs::write(&path, img).unwrap();
        println!("wrote {path}");
    }
}
