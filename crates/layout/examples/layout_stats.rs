//! Measures the v0 layout over seeds 1..=N (default 200): island centre
//! height and water depth under the raft. Used to tune `builder.rs`.
use layout::{build_layout, default_v0_level, is_sea_ice, WorldConfig};
use terrain::{column_summary, Block, HEIGHT, SEA_LEVEL};

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(200);
    let (mut tops, mut land, mut deep) = (Vec::new(), 0, 0);
    for seed in 1..=n {
        let level = default_v0_level(seed);
        let l = build_layout(&WorldConfig { seed }, std::slice::from_ref(&level));
        let [ix, iz] = level.island_center;
        let r = l.generate_region(ix, iz, 1, 1, false);
        let (top, ty, _, _) = column_summary(&r.blocks[..HEIGHT]);
        let wet = top == Block::Water as u8 || is_sea_ice(top);
        if !wet && ty as i32 >= SEA_LEVEL {
            land += 1;
        }
        tops.push(ty as i32);
        // Water under the 5x5 raft footprint, ignoring the stamped deck
        // (y >= SEA_LEVEL): depth is the water run below the deck.
        let r = l.generate_region(-2, -2, 5, 5, false);
        let ok = (0..5).all(|z| {
            (0..5).all(|x| {
                let i = r.index(x, 0, z);
                let mut col = r.blocks[i..i + HEIGHT].to_vec();
                col[SEA_LEVEL as usize..].fill(Block::Air as u8);
                let (top, ty, floor, _) = column_summary(&col);
                top == Block::Water as u8
                    && ty as i32 == SEA_LEVEL - 1
                    && SEA_LEVEL - 1 - floor as i32 >= 3
            })
        });
        deep += ok as u32;
    }
    tops.sort_unstable();
    println!("seeds 1..={n}");
    println!("island centre land: {land}/{n}");
    println!(
        "centre top y: min {} p10 {} median {} (target {}) p90 {} max {}",
        tops[0],
        tops[tops.len() / 10],
        tops[tops.len() / 2],
        SEA_LEVEL + 6,
        tops[tops.len() * 9 / 10],
        tops[tops.len() - 1]
    );
    println!("raft footprint >=3 deep: {deep}/{n}");
}
