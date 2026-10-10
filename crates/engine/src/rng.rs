//! Deterministic random numbers and hash-based noise.

/// Small, fast, seedable PRNG (SplitMix64).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: seed ^ 0x9E37_79B9_7F4A_7C15 }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Uniform in `[0, 1)`.
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in `[a, b)`.
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }

    /// Uniform integer in `[a, b)`.
    pub fn range_i(&mut self, a: i32, b: i32) -> i32 {
        if b <= a { a } else { a + (self.next_u32() % (b - a) as u32) as i32 }
    }

    /// Symmetric in `[-1, 1)`.
    pub fn signed(&mut self) -> f32 {
        self.f32() * 2.0 - 1.0
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next_u32() as usize) % items.len()]
    }

    pub fn unit_vec3(&mut self) -> glam::Vec3 {
        loop {
            let v = glam::Vec3::new(self.signed(), self.signed(), self.signed());
            let l = v.length_squared();
            if l > 1e-4 && l <= 1.0 {
                return v / l.sqrt();
            }
        }
    }
}

/// Integer hash to `[0, 1)`.
#[inline]
pub fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 16_777_216.0
}

#[inline]
pub fn hash1(x: i32, seed: u32) -> f32 {
    hash2(x, 0x51ED_270B_u32 as i32, seed ^ (x as u32).rotate_left(7))
}

/// Tileable 2D value noise; `period` is in lattice cells.
pub fn value_noise(x: f32, y: f32, period: i32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let fx = x - xi as f32;
    let fy = y - yi as f32;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let w = |v: i32| if period > 0 { v.rem_euclid(period) } else { v };
    let a = hash2(w(xi), w(yi), seed);
    let b = hash2(w(xi + 1), w(yi), seed);
    let c = hash2(w(xi), w(yi + 1), seed);
    let d = hash2(w(xi + 1), w(yi + 1), seed);
    let ab = a + (b - a) * sx;
    let cd = c + (d - c) * sx;
    ab + (cd - ab) * sy
}

/// Tileable fractal noise in `[0, 1]`. `x`/`y` are in `[0, 1)` texture space.
pub fn fbm(x: f32, y: f32, base_period: i32, octaves: u32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut norm = 0.0;
    let mut period = base_period;
    for o in 0..octaves {
        sum += value_noise(x * period as f32, y * period as f32, period, seed.wrapping_add(o * 1013)) * amp;
        norm += amp;
        amp *= 0.5;
        period *= 2;
    }
    sum / norm
}

/// Tileable cellular noise: distance to the nearest feature point in `[0, ~1]`
/// plus the id hash of that cell.
pub fn worley(x: f32, y: f32, period: i32, seed: u32) -> (f32, f32) {
    let px = x * period as f32;
    let py = y * period as f32;
    let xi = px.floor() as i32;
    let yi = py.floor() as i32;
    let mut best = f32::MAX;
    let mut id = 0.0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let cx = xi + dx;
            let cy = yi + dy;
            let wx = cx.rem_euclid(period);
            let wy = cy.rem_euclid(period);
            let fx = cx as f32 + hash2(wx, wy, seed);
            let fy = cy as f32 + hash2(wx, wy, seed ^ 0xA5A5);
            let d = ((fx - px).powi(2) + (fy - py).powi(2)).sqrt();
            if d < best {
                best = d;
                id = hash2(wx, wy, seed ^ 0x3C3C);
            }
        }
    }
    (best, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_range() {
        let mut r = Rng::new(3);
        for _ in 0..1000 {
            let v = r.f32();
            assert!((0.0..1.0).contains(&v));
            let i = r.range_i(2, 5);
            assert!((2..5).contains(&i));
        }
    }

    #[test]
    fn noise_tiles() {
        let a = fbm(0.0, 0.3, 4, 3, 9);
        let b = fbm(1.0, 0.3, 4, 3, 9);
        assert!((a - b).abs() < 1e-5);
    }
}
