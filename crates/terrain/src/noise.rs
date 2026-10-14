//! Noise primitives: seeded RNG, improved Perlin noise, octave Perlin sums,
//! 2D simplex noise and octave simplex (SPEC.md section 1).

use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// RNG
// ---------------------------------------------------------------------------

/// SplitMix64 finaliser; a good 64-bit mixing function.
#[inline]
pub fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// SplitMix64 pseudo random generator (SPEC 1.1 allows any good PRNG).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    /// Independent stream for `(seed, generator_id)` (SPEC 1.1, last paragraph).
    pub fn for_generator(seed: u64, id: u64) -> Self {
        Rng(mix64(seed ^ mix64(id.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(0x1234_5678))))
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix64(self.0)
    }

    /// Uniform double in [0, 1).
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform integer in [0, n).
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        (((self.next_u64() >> 32) * n as u64) >> 32) as u32
    }
}

/// Uniform double in [0,1) from a position hash (SPEC Q9 / 6.2).
#[inline]
pub fn hash_f64(seed: u64, x: i32, z: i32, y: i32, k: u64) -> f64 {
    let h = mix64(
        seed ^ mix64(
            (x as u32 as u64) | ((z as u32 as u64) << 32),
        ) ^ mix64(((y as u32 as u64) << 8 | k).wrapping_add(0xD1B5_4A32_D192_ED03)),
    );
    (h >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// 64-bit floor (SPEC Q1: no 32-bit saturation).
#[inline(always)]
pub fn floor(x: f64) -> f64 {
    let t = x as i64 as f64;
    if t > x {
        t - 1.0
    } else {
        t
    }
}

#[inline(always)]
pub fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline(always)]
pub fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

/// Precomputed per-axis data for one Perlin coordinate:
/// wrapped cell index, fractional part, faded fraction.
#[derive(Clone, Copy, Default)]
pub struct Axis {
    pub i: usize,
    pub f: f64,
    pub s: f64,
}

impl Axis {
    #[inline(always)]
    pub fn new(c: f64) -> Axis {
        let fl = floor(c);
        let f = c - fl;
        Axis { i: ((fl as i64) & 255) as usize, f, s: fade(f) }
    }
}

fn permutation(rng: &mut Rng) -> [u8; 512] {
    let mut p = [0u8; 512];
    for (i, v) in p.iter_mut().take(256).enumerate() {
        *v = i as u8;
    }
    for i in 0..256usize {
        let j = i + rng.below(256 - i as u32) as usize;
        p.swap(i, j);
    }
    for i in 0..256 {
        p[i + 256] = p[i];
    }
    p
}

// ---------------------------------------------------------------------------
// Improved Perlin noise (SPEC 1.2)
// ---------------------------------------------------------------------------

/// Gradient vectors for `h & 15`, exactly the set defined in SPEC 1.2 step 5
/// (u = x if h < 8 else y; v = y if h < 4, x if h is 12/14, else z; bit 0
/// negates u, bit 1 negates v). A table avoids unpredictable branches.
const GRAD3: [[f64; 3]; 16] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
    [1.0, 1.0, 0.0],
    [0.0, -1.0, 1.0],
    [-1.0, 1.0, 0.0],
    [0.0, -1.0, -1.0],
];

#[inline(always)]
fn grad(h: u8, x: f64, y: f64, z: f64) -> f64 {
    let g = &GRAD3[(h & 15) as usize];
    g[0] * x + g[1] * y + g[2] * z
}

/// Reference (branchy) form of the gradient function, used by tests.
#[cfg(test)]
pub fn grad_reference(h: u8, x: f64, y: f64, z: f64) -> f64 {
    let h = h & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 != 0 { -u } else { u }) + (if h & 2 != 0 { -v } else { v })
}

#[cfg(test)]
pub fn grad_table(h: u8, x: f64, y: f64, z: f64) -> f64 {
    grad(h, x, y, z)
}

pub struct Perlin {
    p: [u8; 512],
    pub ox: f64,
    pub oy: f64,
    pub oz: f64,
}

impl Perlin {
    pub fn new(rng: &mut Rng) -> Perlin {
        let ox = rng.next_f64() * 256.0;
        let oy = rng.next_f64() * 256.0;
        let oz = rng.next_f64() * 256.0;
        let p = permutation(rng);
        Perlin { p, ox, oy, oz }
    }

    /// Noise at (x, y, z); the octave offsets are added here.
    #[inline]
    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        self.eval(
            Axis::new(x + self.ox),
            Axis::new(y + self.oy),
            Axis::new(z + self.oz),
        )
    }

    /// Evaluate from precomputed axes (offsets already applied).
    #[inline(always)]
    pub fn eval(&self, ax: Axis, ay: Axis, az: Axis) -> f64 {
        let p = &self.p;
        let (fx, fy, fz) = (ax.f, ay.f, az.f);
        let a = p[ax.i & 511] as usize + ay.i;
        let aa = p[a & 511] as usize + az.i;
        let ab = p[(a + 1) & 511] as usize + az.i;
        let b = p[(ax.i + 1) & 511] as usize + ay.i;
        let ba = p[b & 511] as usize + az.i;
        let bb = p[(b + 1) & 511] as usize + az.i;
        let (u, v, w) = (ax.s, ay.s, az.s);
        let x1 = fx - 1.0;
        let y1 = fy - 1.0;
        let z1 = fz - 1.0;
        let lo = lerp(
            v,
            lerp(u, grad(p[aa & 511], fx, fy, fz), grad(p[ba & 511], x1, fy, fz)),
            lerp(u, grad(p[ab & 511], fx, y1, fz), grad(p[bb & 511], x1, y1, fz)),
        );
        let hi = lerp(
            v,
            lerp(u, grad(p[(aa + 1) & 511], fx, fy, z1), grad(p[(ba + 1) & 511], x1, fy, z1)),
            lerp(u, grad(p[(ab + 1) & 511], fx, y1, z1), grad(p[(bb + 1) & 511], x1, y1, z1)),
        );
        lerp(w, lo, hi)
    }
}

// ---------------------------------------------------------------------------
// Octave Perlin (SPEC 1.3)
// ---------------------------------------------------------------------------

pub struct OctavePerlin {
    pub octaves: Vec<Perlin>,
}

impl OctavePerlin {
    pub fn new(rng: &mut Rng, n: usize) -> OctavePerlin {
        let mut octaves = Vec::with_capacity(n);
        for _ in 0..n {
            octaves.push(Perlin::new(rng));
        }
        OctavePerlin { octaves }
    }

    /// Fractal sum at an already-scaled coordinate:
    /// sum_i noise_i(c * 2^-i) * 2^i.
    pub fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut freq = 1.0;
        let mut amp = 1.0;
        let mut sum = 0.0;
        for o in &self.octaves {
            sum += o.noise(x * freq, y * freq, z * freq) * amp;
            freq *= 0.5;
            amp *= 2.0;
        }
        sum
    }

    /// 2D field: the clean "3D noise on the plane y = 0" (SPEC 1.2, Q2).
    pub fn sample2d(&self, x: f64, z: f64) -> f64 {
        self.sample(x, 0.0, z)
    }

    /// Accumulate the octave sum over a 3D grid into `out`.
    ///
    /// `xs`, `ys`, `zs` are the already-scaled base coordinates for each grid
    /// index along each axis. Layout of `out` is `(ix * nz + iz) * ny + iy`.
    /// If `mask` is given, only entries with `mask[idx] == true` are computed.
    pub fn fill_grid(
        &self,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        out: &mut [f64],
        mask: Option<&[bool]>,
        ax: &mut Vec<Axis>,
        ay: &mut Vec<Axis>,
        az: &mut Vec<Axis>,
    ) {
        let (nx, ny, nz) = (xs.len(), ys.len(), zs.len());
        let mut freq = 1.0;
        let mut amp = 1.0;
        for o in &self.octaves {
            ax.clear();
            ay.clear();
            az.clear();
            ax.extend(xs.iter().map(|&c| Axis::new(c * freq + o.ox)));
            ay.extend(ys.iter().map(|&c| Axis::new(c * freq + o.oy)));
            az.extend(zs.iter().map(|&c| Axis::new(c * freq + o.oz)));
            for ix in 0..nx {
                let a_x = ax[ix];
                for iz in 0..nz {
                    let a_z = az[iz];
                    let base = (ix * nz + iz) * ny;
                    let row = &mut out[base..base + ny];
                    match mask {
                        Some(m) => {
                            let mrow = &m[base..base + ny];
                            for iy in 0..ny {
                                if mrow[iy] {
                                    row[iy] += o.eval(a_x, ay[iy], a_z) * amp;
                                }
                            }
                        }
                        None => {
                            for iy in 0..ny {
                                row[iy] += o.eval(a_x, ay[iy], a_z) * amp;
                            }
                        }
                    }
                }
            }
            freq *= 0.5;
            amp *= 2.0;
        }
    }
}

// ---------------------------------------------------------------------------
// Simplex (SPEC 1.4)
// ---------------------------------------------------------------------------

const SQRT3: f64 = 1.732_050_807_568_877_2;
const F2: f64 = 0.5 * (SQRT3 - 1.0);
const G2: f64 = (3.0 - SQRT3) / 6.0;
const GRAD2: [(f64, f64); 12] = [
    (1.0, 1.0),
    (-1.0, 1.0),
    (1.0, -1.0),
    (-1.0, -1.0),
    (1.0, 0.0),
    (-1.0, 0.0),
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (0.0, 1.0),
    (0.0, -1.0),
];

pub struct Simplex {
    p: [u8; 512],
    pub ox: f64,
    pub oy: f64,
}

impl Simplex {
    pub fn new(rng: &mut Rng) -> Simplex {
        let ox = rng.next_f64() * 256.0;
        let oy = rng.next_f64() * 256.0;
        let _oz = rng.next_f64() * 256.0; // drawn but unused (SPEC 1.4)
        let p = permutation(rng);
        Simplex { p, ox, oy }
    }

    /// Raw 2D simplex noise (offsets NOT added), approx. [-1, 1].
    pub fn noise(&self, x: f64, y: f64) -> f64 {
        let p = &self.p;
        let s = (x + y) * F2;
        let i = floor(x + s);
        let j = floor(y + s);
        let t = (i + j) * G2;
        let x0 = x - (i - t);
        let y0 = y - (j - t);
        let (i1, j1) = if x0 > y0 { (1usize, 0usize) } else { (0, 1) };
        let x1 = x0 - i1 as f64 + G2;
        let y1 = y0 - j1 as f64 + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let y2 = y0 - 1.0 + 2.0 * G2;
        let ii = ((i as i64) & 255) as usize;
        let jj = ((j as i64) & 255) as usize;
        let gi = |ci: usize, cj: usize| -> usize {
            (p[(ii + ci + p[(jj + cj) & 511] as usize) & 511] % 12) as usize
        };
        let corner = |g: usize, dx: f64, dy: f64| -> f64 {
            let t = (0.5 - dx * dx - dy * dy).max(0.0);
            let t2 = t * t;
            let (gx, gy) = GRAD2[g];
            t2 * t2 * (gx * dx + gy * dy)
        };
        70.0 * (corner(gi(0, 0), x0, y0) + corner(gi(i1, j1), x1, y1) + corner(gi(1, 1), x2, y2))
    }
}

/// Octave simplex: base scales divided by 1.5, octave i at `scale * fm^i`,
/// weighted `0.55 / am^i` (SPEC 1.4).
pub struct OctaveSimplex {
    pub octaves: Vec<Simplex>,
}

impl OctaveSimplex {
    pub fn new(rng: &mut Rng, n: usize) -> OctaveSimplex {
        let mut octaves = Vec::with_capacity(n);
        for _ in 0..n {
            octaves.push(Simplex::new(rng));
        }
        OctaveSimplex { octaves }
    }

    pub fn sample(&self, x: f64, z: f64, sx: f64, sz: f64, fm: f64, am: f64) -> f64 {
        let sx = sx / 1.5;
        let sz = sz / 1.5;
        let mut f = 1.0;
        let mut w = 0.55;
        let mut sum = 0.0;
        for o in &self.octaves {
            sum += o.noise(x * sx * f + o.ox, z * sz * f + o.oy) * w;
            f *= fm;
            w /= am;
        }
        sum
    }
}
