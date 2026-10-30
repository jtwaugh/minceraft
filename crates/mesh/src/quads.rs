//! Growable quad buffer in the packed 16-byte vertex format of `viewer/mesh.js`:
//!
//! - bytes 0-5: position x, y, z, i16 in 1/16 block
//! - bytes 6-7: uv, u8 in 1/16
//! - byte 8: texture layer
//! - byte 9: shade * 128
//! - bytes 10-12: tint rgb
//! - byte 13: ambient occlusion level 0-3
//! - byte 14: sway weight
//! - byte 15: water depth
use crate::js_round;
use alloc::vec::Vec;

pub const VERTEX_BYTES: usize = 16;

/// One tint (0-1 per channel) for all four vertices, or one per vertex.
pub enum Tint {
    One([f64; 3]),
    Four([[f64; 3]; 4]),
}

#[derive(Default)]
pub struct Quads {
    buf: Vec<u8>,
}

impl Quads {
    pub fn new() -> Self {
        Quads {
            buf: Vec::with_capacity(VERTEX_BYTES * 4 * 1024),
        }
    }

    /// Number of quads written.
    pub fn faces(&self) -> usize {
        self.buf.len() / (VERTEX_BYTES * 4)
    }

    /// The vertex bytes written so far.
    pub fn bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Append a quad at block (x, y, z) with corner offsets `p` and texture
    /// coordinates `uv`. `k` is the shade, `top` the sway weights and `ao` the
    /// AO levels (both 0 when `None`).
    pub fn quad(
        &mut self,
        [x, y, z]: [f64; 3],
        p: &[[f64; 3]; 4],
        uv: &[[f64; 2]; 4],
        layer: u8,
        k: f64,
        tint: &Tint,
        top: Option<[u8; 4]>,
        ao: Option<[u8; 4]>,
        depth: u8,
    ) {
        let sh = js_round(k * 128.0) as i64 as u8;
        for j in 0..4 {
            let t = match tint {
                Tint::One(t) => t,
                Tint::Four(t) => &t[j],
            };
            let pos = [x + p[j][0], y + p[j][1], z + p[j][2]];
            for v in pos {
                // wraps like a JS Int16Array store
                let n = js_round(v * 16.0) as i64 as i16;
                self.buf.extend_from_slice(&n.to_le_bytes());
            }
            self.buf.push(js_round(uv[j][0] * 16.0) as i64 as u8);
            self.buf.push(js_round(uv[j][1] * 16.0) as i64 as u8);
            self.buf.push(layer);
            self.buf.push(sh);
            for c in t {
                self.buf.push(js_round(c * 255.0) as i64 as u8);
            }
            self.buf.push(ao.map_or(0, |a| a[j]));
            self.buf.push(top.map_or(0, |a| a[j]));
            self.buf.push(depth);
        }
    }
}
