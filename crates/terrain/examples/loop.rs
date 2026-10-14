fn main() { let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20);
 let t = std::time::Instant::now();
 for i in 0..n { let h = terrain::generate_heightmap(12345 + i as u64, 0, 0, 512, 512); std::hint::black_box(h); }
 println!("avg heightmap {:.1} ms", t.elapsed().as_secs_f64()*1e3 / n as f64); }
