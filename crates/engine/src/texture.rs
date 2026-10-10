//! CPU-side texture data and helpers for procedural texture generation.
//!
//! All world textures share one size so the renderer can put them in a single
//! `TEXTURE_2D_ARRAY` (one texture page, like the PSX VRAM).

use crate::math::{Vec3, Vec4};

pub const TEX_SIZE: usize = 64;

#[derive(Clone, Debug)]
pub struct TextureData {
    pub width: usize,
    pub height: usize,
    /// RGBA8, row-major, top row first.
    pub pixels: Vec<[u8; 4]>,
}

impl TextureData {
    pub fn new(width: usize, height: usize) -> Self {
        TextureData { width, height, pixels: vec![[0, 0, 0, 255]; width * height] }
    }

    pub fn solid(c: [u8; 4]) -> Self {
        TextureData { width: TEX_SIZE, height: TEX_SIZE, pixels: vec![c; TEX_SIZE * TEX_SIZE] }
    }

    /// Fills every pixel from a function of normalised coordinates `(u, v)` in `[0, 1)`.
    /// Returned colour is linear-ish RGBA in `[0, 1]`.
    pub fn from_fn(size: usize, f: impl Fn(f32, f32) -> Vec4) -> Self {
        let mut t = TextureData::new(size, size);
        for y in 0..size {
            for x in 0..size {
                let c = f(x as f32 / size as f32, y as f32 / size as f32);
                t.pixels[y * size + x] = to_rgba8(c);
            }
        }
        t
    }

    pub fn get(&self, x: i32, y: i32) -> [u8; 4] {
        let x = x.rem_euclid(self.width as i32) as usize;
        let y = y.rem_euclid(self.height as i32) as usize;
        self.pixels[y * self.width + x]
    }

    pub fn set(&mut self, x: i32, y: i32, c: [u8; 4]) {
        let x = x.rem_euclid(self.width as i32) as usize;
        let y = y.rem_euclid(self.height as i32) as usize;
        self.pixels[y * self.width + x] = c;
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.pixels.iter().flat_map(|p| p.iter().copied()).collect()
    }

    /// Reduces the image to at most `n` colours using k-means, which mimics a
    /// 4-bit (16 colour) or 8-bit CLUT texture. Alpha is kept binary.
    pub fn quantize(&mut self, n: usize) {
        let opaque: Vec<Vec3> = self
            .pixels
            .iter()
            .filter(|p| p[3] >= 128)
            .map(|p| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32))
            .collect();
        if opaque.is_empty() {
            return;
        }
        // Initialise centroids on a luminance-sorted spread.
        let mut sorted = opaque.clone();
        sorted.sort_by(|a, b| luma(*a).partial_cmp(&luma(*b)).unwrap());
        let k = n.min(sorted.len()).max(1);
        let mut cents: Vec<Vec3> = (0..k).map(|i| sorted[(i * (sorted.len() - 1)) / k.max(2).saturating_sub(1).max(1)]).collect();
        let mut assign = vec![0usize; opaque.len()];
        for _ in 0..10 {
            for (i, p) in opaque.iter().enumerate() {
                let mut best = 0;
                let mut bd = f32::MAX;
                for (j, c) in cents.iter().enumerate() {
                    let d = (*p - *c).length_squared();
                    if d < bd {
                        bd = d;
                        best = j;
                    }
                }
                assign[i] = best;
            }
            let mut sums = vec![Vec3::ZERO; k];
            let mut counts = vec![0u32; k];
            for (i, p) in opaque.iter().enumerate() {
                sums[assign[i]] += *p;
                counts[assign[i]] += 1;
            }
            for j in 0..k {
                if counts[j] > 0 {
                    cents[j] = sums[j] / counts[j] as f32;
                }
            }
        }
        let mut oi = 0;
        for p in self.pixels.iter_mut() {
            if p[3] >= 128 {
                let c = cents[assign[oi]];
                oi += 1;
                *p = [to_5bit(c.x), to_5bit(c.y), to_5bit(c.z), 255];
            } else {
                *p = [0, 0, 0, 0];
            }
        }
    }
}

fn luma(c: Vec3) -> f32 {
    c.x * 0.299 + c.y * 0.587 + c.z * 0.114
}

/// Snaps an 8-bit channel to the nearest 5-bit level (15-bit colour).
fn to_5bit(v: f32) -> u8 {
    let q = (v.clamp(0.0, 255.0) / 255.0 * 31.0).round();
    (q / 31.0 * 255.0).round() as u8
}

pub fn to_rgba8(c: Vec4) -> [u8; 4] {
    [
        (c.x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        (c.y.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        (c.z.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        (c.w.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
    ]
}

/// Hex colour to linear `Vec3` in `[0, 1]`.
pub const fn hex3(h: u32) -> Vec3 {
    Vec3::new(
        ((h >> 16) & 0xff) as f32 / 255.0,
        ((h >> 8) & 0xff) as f32 / 255.0,
        (h & 0xff) as f32 / 255.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_limits_colours() {
        let mut t = TextureData::from_fn(32, |u, v| Vec4::new(u, v, (u * v).sqrt(), 1.0));
        t.quantize(16);
        let mut set: Vec<[u8; 4]> = t.pixels.clone();
        set.sort();
        set.dedup();
        assert!(set.len() <= 16, "{}", set.len());
    }
}
