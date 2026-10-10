//! Billboard particles (blood, sparks, dust, flames, embers...).

use crate::assets::textures::Tex;
use ashen_engine::math::*;
use ashen_engine::render::{Blend, DynQuad};
use ashen_engine::rng::Rng;

#[derive(Clone, Debug)]
pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub size_end: f32,
    pub color: Vec4,
    pub color_end: Vec4,
    pub tex: Tex,
    pub blend: Blend,
    pub gravity: f32,
    pub drag: f32,
    pub spin: f32,
    pub angle: f32,
    /// Moves towards this point (cinders flowing into the player).
    pub home: Option<Vec3>,
    /// Lie flat on the ground instead of facing the camera.
    pub flat: bool,
}

impl Particle {
    fn new(pos: Vec3, vel: Vec3, life: f32, size: f32, color: Vec4, tex: Tex, blend: Blend) -> Self {
        Particle {
            pos,
            vel,
            life,
            max_life: life,
            size,
            size_end: size,
            color,
            color_end: color.with_w(0.0),
            tex,
            blend,
            gravity: 0.0,
            drag: 0.0,
            spin: 0.0,
            angle: 0.0,
            home: None,
            flat: false,
        }
    }
}

pub struct Fx {
    pub parts: Vec<Particle>,
    rng: Rng,
}

impl Default for Fx {
    fn default() -> Self {
        Fx { parts: Vec::new(), rng: Rng::new(42) }
    }
}

fn rgba(r: f32, g: f32, b: f32, a: f32) -> Vec4 {
    Vec4::new(r, g, b, a)
}

impl Fx {
    pub fn clear(&mut self) {
        self.parts.clear();
    }

    pub fn blood(&mut self, pos: Vec3, dir: Vec3, amount: u32) {
        for _ in 0..amount {
            let v = (dir * self.rng.range(1.0, 3.5) + self.rng.unit_vec3() * 1.8 + Vec3::Y * self.rng.range(0.5, 2.5)) * 1.0;
            let mut p = Particle::new(pos, v, self.rng.range(0.35, 0.7), self.rng.range(0.06, 0.14), rgba(0.55, 0.04, 0.03, 1.0), Tex::Blood, Blend::Alpha);
            p.gravity = 9.0;
            p.drag = 1.0;
            p.size_end = p.size * 0.6;
            p.color_end = rgba(0.3, 0.02, 0.02, 0.0);
            self.parts.push(p);
        }
    }

    pub fn sparks(&mut self, pos: Vec3, dir: Vec3, amount: u32) {
        for _ in 0..amount {
            let v = dir * self.rng.range(1.0, 4.0) + self.rng.unit_vec3() * 3.5 + Vec3::Y * 1.5;
            let mut p = Particle::new(pos, v, self.rng.range(0.15, 0.4), self.rng.range(0.05, 0.1), rgba(1.0, 0.85, 0.5, 1.0), Tex::Spark, Blend::Additive);
            p.gravity = 8.0;
            p.size_end = 0.02;
            p.color_end = rgba(1.0, 0.4, 0.1, 0.0);
            self.parts.push(p);
        }
    }

    pub fn dust(&mut self, pos: Vec3, size: f32) {
        let n = (8.0 * size) as u32 + 6;
        for _ in 0..n {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let r = self.rng.range(0.2, 1.0) * size;
            let v = Vec3::new(a.cos(), self.rng.range(0.1, 0.6), a.sin()) * self.rng.range(1.0, 3.0) * size.sqrt();
            let mut p = Particle::new(pos + Vec3::new(a.cos() * r, 0.15, a.sin() * r), v, self.rng.range(0.8, 1.6), 0.25 * size.sqrt(), rgba(0.45, 0.42, 0.38, 0.38), Tex::Soft, Blend::Alpha);
            p.size_end = 0.75 * size.sqrt();
            p.drag = 2.0;
            p.gravity = -0.3;
            p.spin = self.rng.signed();
            self.parts.push(p);
        }
        for _ in 0..(n / 2) {
            let v = self.rng.unit_vec3() * 3.0 + Vec3::Y * 4.0;
            let mut p = Particle::new(pos + Vec3::Y * 0.2, v, self.rng.range(0.5, 1.0), 0.06, rgba(0.35, 0.33, 0.3, 1.0), Tex::Rubble, Blend::Alpha);
            p.gravity = 14.0;
            p.size_end = 0.05;
            p.color_end = rgba(0.35, 0.33, 0.3, 1.0);
            self.parts.push(p);
        }
    }

    /// One frame's worth of flame for a torch or pyre (call each tick).
    pub fn flame(&mut self, pos: Vec3, scale: f32, dt: f32) {
        let rate = 26.0 * scale;
        let n = self.rng.f32() < (rate * dt).fract() || (rate * dt) >= 1.0;
        let count = (rate * dt).floor() as u32 + n as u32;
        for _ in 0..count {
            let off = Vec3::new(self.rng.signed(), 0.0, self.rng.signed()) * 0.08 * scale;
            let v = Vec3::new(self.rng.signed() * 0.15, self.rng.range(0.6, 1.2), self.rng.signed() * 0.15) * scale.sqrt();
            let mut p = Particle::new(pos + off, v, self.rng.range(0.3, 0.55), 0.2 * scale, rgba(1.0, 0.75, 0.35, 1.0), Tex::Flame, Blend::Additive);
            p.size_end = 0.05 * scale;
            p.color_end = rgba(0.7, 0.15, 0.05, 0.0);
            self.parts.push(p);
        }
        if self.rng.f32() < dt * 3.0 * scale {
            let mut p = Particle::new(pos, Vec3::new(self.rng.signed() * 0.3, self.rng.range(0.8, 1.5), self.rng.signed() * 0.3), self.rng.range(1.0, 2.0), 0.03, rgba(1.0, 0.6, 0.2, 1.0), Tex::Glow, Blend::Additive);
            p.color_end = rgba(1.0, 0.3, 0.1, 0.0);
            self.parts.push(p);
        }
    }

    /// Soft orange swirl rising from an item on the ground.
    pub fn item_glint(&mut self, pos: Vec3, dt: f32) {
        if self.rng.f32() < dt * 4.0 {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let mut p = Particle::new(pos + Vec3::new(a.cos() * 0.2, 0.05, a.sin() * 0.2), Vec3::Y * 0.5, 1.2, 0.08, rgba(1.0, 0.9, 0.7, 0.9), Tex::Glow, Blend::Additive);
            p.size_end = 0.02;
            self.parts.push(p);
        }
    }

    pub fn heal(&mut self, pos: Vec3) {
        for _ in 0..24 {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let r = self.rng.range(0.2, 0.6);
            let mut p = Particle::new(pos + Vec3::new(a.cos() * r, self.rng.range(0.0, 1.6), a.sin() * r), Vec3::Y * self.rng.range(0.4, 1.0), self.rng.range(0.6, 1.2), 0.1, rgba(1.0, 0.65, 0.3, 1.0), Tex::Glow, Blend::Additive);
            p.size_end = 0.02;
            self.parts.push(p);
        }
    }

    /// Cinders flowing from a corpse into the player.
    pub fn cinders(&mut self, from: Vec3, to: Vec3) {
        for _ in 0..30 {
            let mut p = Particle::new(from + self.rng.unit_vec3() * 0.4, self.rng.unit_vec3() * 2.0 + Vec3::Y * 2.0, self.rng.range(1.0, 1.8), 0.09, rgba(0.9, 0.95, 1.0, 1.0), Tex::Glow, Blend::Additive);
            p.home = Some(to);
            p.size_end = 0.03;
            p.color_end = rgba(0.6, 0.8, 1.0, 0.3);
            self.parts.push(p);
        }
    }

    /// Ash motes rising from a dissolving body.
    pub fn motes(&mut self, center: Vec3, radius: f32, dt: f32) {
        let n = (60.0 * radius * dt) as u32 + 1;
        for _ in 0..n {
            let p0 = center + self.rng.unit_vec3() * radius * self.rng.f32().sqrt();
            let mut p = Particle::new(p0, Vec3::Y * self.rng.range(0.3, 1.2) + self.rng.unit_vec3() * 0.2, self.rng.range(1.0, 2.0), 0.07 * radius.sqrt(), rgba(0.85, 0.85, 0.8, 0.9), Tex::Glow, Blend::Additive);
            p.size_end = 0.01;
            self.parts.push(p);
        }
    }

    pub fn shards(&mut self, pos: Vec3) {
        for _ in 0..14 {
            let v = self.rng.unit_vec3() * 2.5 + Vec3::Y * 3.0;
            let mut p = Particle::new(pos + Vec3::Y * 0.4, v, self.rng.range(0.6, 1.0), 0.09, rgba(0.6, 0.45, 0.3, 1.0), Tex::Ceramic, Blend::Alpha);
            p.gravity = 12.0;
            p.spin = self.rng.signed() * 10.0;
            p.color_end = rgba(0.6, 0.45, 0.3, 1.0);
            self.parts.push(p);
        }
        self.dust(pos, 0.4);
    }

    pub fn splash(&mut self, pos: Vec3) {
        for _ in 0..6 {
            let v = self.rng.unit_vec3() * 0.8 + Vec3::Y * 1.5;
            let mut p = Particle::new(pos + Vec3::Y * 0.2, v, 0.4, 0.06, rgba(0.6, 0.7, 0.7, 0.8), Tex::Soft, Blend::Alpha);
            p.gravity = 8.0;
            self.parts.push(p);
        }
    }

    pub fn mist(&mut self, pos: Vec3, across: Vec3, width: f32, height: f32, dt: f32) {
        if self.rng.f32() < dt * 6.0 {
            let p0 = pos + across * self.rng.signed() * width * 0.5 + Vec3::Y * self.rng.range(0.0, height);
            let mut p = Particle::new(p0, Vec3::Y * 0.2 + self.rng.unit_vec3() * 0.1, 2.5, 0.4, rgba(0.8, 0.82, 0.86, 0.35), Tex::Soft, Blend::Additive);
            p.size_end = 1.0;
            self.parts.push(p);
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in self.parts.iter_mut() {
            p.life -= dt;
            if let Some(h) = p.home {
                let to = h - p.pos;
                p.vel = p.vel.lerp(to.normalize_or_zero() * 8.0, (dt * 3.0).min(1.0));
                if to.length() < 0.3 {
                    p.life = 0.0;
                }
            }
            p.vel.y -= p.gravity * dt;
            p.vel *= 1.0 / (1.0 + p.drag * dt);
            p.pos += p.vel * dt;
            p.angle += p.spin * dt;
        }
        self.parts.retain(|p| p.life > 0.0);
        if self.parts.len() > 2500 {
            let n = self.parts.len() - 2500;
            self.parts.drain(0..n);
        }
    }

    pub fn emit_quads(&self, right: Vec3, up: Vec3, out: &mut Vec<DynQuad>) {
        for p in &self.parts {
            let t = 1.0 - p.life / p.max_life;
            let s = lerp(p.size, p.size_end, t);
            let c = p.color.lerp(p.color_end, t);
            let (r, u) = if p.flat {
                (Vec3::X * s, Vec3::Z * s)
            } else {
                let (sa, ca) = p.angle.sin_cos();
                (right * (ca * s) + up * (sa * s), up * (ca * s) - right * (sa * s))
            };
            let color = [
                (c.x.clamp(0.0, 1.0) * 255.0) as u8,
                (c.y.clamp(0.0, 1.0) * 255.0) as u8,
                (c.z.clamp(0.0, 1.0) * 255.0) as u8,
                (c.w.clamp(0.0, 1.0) * 255.0) as u8,
            ];
            out.push(DynQuad {
                p: [p.pos - r - u, p.pos + r - u, p.pos + r + u, p.pos - r + u],
                uv: [Vec2::new(0.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 0.0)],
                color,
                layer: p.tex.layer(),
                blend: p.blend,
            });
        }
    }
}
