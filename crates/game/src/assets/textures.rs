//! Procedural 64x64 textures, quantised to small palettes like PSX CLUT textures.
//!
//! The order of [`Tex`] is the layer order in the renderer's texture array.

use ashen_engine::math::{Vec3, Vec4};
use ashen_engine::rng::{fbm, hash2, value_noise, worley};
use ashen_engine::texture::{TEX_SIZE, TextureData, hex3};

macro_rules! tex_enum {
    ($($name:ident),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum Tex { $($name),* }
        impl Tex {
            pub const ALL: &'static [Tex] = &[$(Tex::$name),*];
        }
    };
}

tex_enum!(
    White,
    StoneBrick,
    MossBrick,
    Flagstone,
    ArenaFloor,
    WoodPlank,
    IronBars,
    Rock,
    Grass,
    Dirt,
    Steel,
    ClothRed,
    Chainmail,
    Leather,
    HuskSkin,
    DemonSkin,
    DemonBelly,
    StoneClub,
    Flame,
    Soft,
    Blood,
    Spark,
    FogVeil,
    Clouds,
    Ash,
    Shadow,
    Water,
    Ceramic,
    Iron,
    DarkWood,
    KiteShield,
    Glow,
    RuneRing,
    Gravestone,
    HelmFace,
    HuskFace,
    DemonFace,
    Rubble,
    ClothDark,
    Bone,
    Feather,
    Plaster,
    Stairs,
    Moss,
);

impl Tex {
    pub fn layer(self) -> u8 {
        self as u8
    }
}

fn rgba(c: Vec3) -> Vec4 {
    c.extend(1.0)
}

fn n2(u: f32, v: f32, p: i32, s: u32) -> f32 {
    value_noise(u * p as f32, v * p as f32, p, s)
}

/// Running-bond brick layout: returns (mortar distance 0..1, brick id hash, (bu, bv) inside brick).
fn bricks(u: f32, v: f32, cols: f32, rows: f32, jitter: f32) -> (f32, f32, f32, f32) {
    let row = (v * rows).floor();
    let off = if (row as i32) % 2 == 0 { 0.0 } else { 0.5 };
    let x = u * cols + off + hash2(row as i32, 7, 3) * jitter;
    let col = x.floor();
    let bu = x - col;
    let bv = v * rows - row;
    let edge = (bu.min(1.0 - bu) * 2.0 / cols * cols).min(bv.min(1.0 - bv) * 2.0);
    let id = hash2(col.rem_euclid(cols) as i32, row as i32, 11);
    (edge, id, bu, bv)
}

fn stone_brick(u: f32, v: f32, moss: f32) -> Vec4 {
    let (edge, id, _bu, _bv) = bricks(u, v, 2.0, 4.0, 0.0);
    let grain = fbm(u, v, 8, 3, 21);
    let base = hex3(0x6f6458).lerp(hex3(0x8a7f70), id) * (0.75 + grain * 0.45);
    let mortar = hex3(0x2e2a26);
    let mut c = if edge < 0.09 { mortar } else { base * (0.85 + 0.15 * (edge * 3.0).min(1.0)) };
    // Chips and cracks.
    let (w, _) = worley(u, v, 6, 77);
    if w < 0.08 && edge >= 0.09 {
        c *= 0.6;
    }
    // Moss grows from the bottom of bricks and in mortar.
    let m = fbm(u, v, 4, 4, 99);
    if moss > 0.0 && m > 0.62 - moss * 0.2 {
        let k = ((m - 0.5) * 3.0).clamp(0.0, 1.0) * moss;
        c = c.lerp(hex3(0x4a5a26) * (0.7 + grain * 0.5), k);
    }
    // Grime streaks.
    let streak = value_noise(u * 16.0, v * 2.0, 16, 5);
    c *= 0.85 + streak * 0.2;
    rgba(c)
}

fn flagstone(u: f32, v: f32, _scale: i32, tint: Vec3) -> Vec4 {
    // Two rows of irregular slabs per tile, staggered.
    let rows = 2.0;
    let row = (v * rows).floor();
    let off = if row as i32 % 2 == 0 { 0.0 } else { 0.37 };
    let cols = if row as i32 % 2 == 0 { 2.0 } else { 3.0 };
    let x = u * cols + off;
    let col = x.floor();
    let fu = x - col;
    let fv = v * rows - row;
    let id = hash2(col.rem_euclid(cols) as i32, row as i32, 41);
    let edge = (fu.min(1.0 - fu) / cols * 2.0).min(fv.min(1.0 - fv) / rows);
    let grain = fbm(u, v, 8, 4, 13);
    let mut c = tint * (0.72 + id * 0.32) * (0.78 + grain * 0.4);
    // Worn, rounded slab edges and dark joints.
    if edge < 0.012 {
        c = hex3(0x24201c);
    } else if edge < 0.03 {
        c *= 0.75;
    }
    let (w, _) = worley(u, v, 7, 43);
    if w < 0.06 {
        c *= 0.7;
    }
    let dirt = fbm(u, v, 3, 3, 44);
    c = c.lerp(hex3(0x3a3428), ((dirt - 0.55) * 2.5).clamp(0.0, 0.5));
    rgba(c)
}

fn arena_floor(u: f32, v: f32) -> Vec4 {
    // Large square slabs, slightly irregular.
    let su = u * 2.0;
    let sv = v * 2.0;
    let fu = su.fract();
    let fv = sv.fract();
    let id = hash2(su.floor() as i32, sv.floor() as i32, 3);
    let edge = fu.min(1.0 - fu).min(fv.min(1.0 - fv));
    let grain = fbm(u, v, 8, 4, 55);
    let mut c = hex3(0x77746c).lerp(hex3(0x5d5a52), id) * (0.75 + grain * 0.4);
    if edge < 0.03 {
        c = hex3(0x2a2824);
    }
    let dirt = fbm(u, v, 4, 3, 66);
    c = c.lerp(hex3(0x4b4536), ((dirt - 0.55) * 3.0).clamp(0.0, 0.6));
    rgba(c)
}

fn wood(u: f32, v: f32, dark: bool) -> Vec4 {
    let planks = 4.0;
    let p = (u * planks).floor();
    let pu = (u * planks).fract();
    let id = hash2(p as i32, 1, 9);
    let ring = ((v * 3.0 + n2(u, v, 8, 17) * 2.0 + id * 5.0) * 6.0).sin() * 0.5 + 0.5;
    let base = if dark { hex3(0x3a2a1c) } else { hex3(0x6b4a2c) };
    let mut c = base * (0.75 + ring * 0.25 + id * 0.15);
    if pu < 0.06 || pu > 0.96 {
        c *= 0.45;
    }
    // Nail heads.
    let nv = (v * 2.0).fract();
    if (pu - 0.5).abs() < 0.06 && (nv - 0.1).abs() < 0.03 {
        c = hex3(0x2a2a2a);
    }
    rgba(c)
}

fn iron_bars(u: f32, v: f32) -> Vec4 {
    let bars = 4.0;
    let bu = (u * bars).fract();
    let cross = (v * 2.0).fract();
    let is_bar = (bu - 0.5).abs() < 0.11;
    let is_cross = (cross - 0.5).abs() < 0.05;
    if !(is_bar || is_cross) {
        return Vec4::ZERO;
    }
    let rust = fbm(u, v, 8, 3, 31);
    let c = hex3(0x3a3836).lerp(hex3(0x5e3a22), (rust - 0.4).clamp(0.0, 1.0) * 1.5);
    let shade = if is_bar { 0.7 + 0.3 * (1.0 - ((bu - 0.5).abs() / 0.11)) } else { 0.8 };
    rgba(c * shade)
}

fn rock(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 4, 5, 71);
    let (w, id) = worley(u, v, 5, 72);
    let c = hex3(0x5a5650).lerp(hex3(0x7a756a), n) * (0.7 + id * 0.3) * (0.75 + w * 0.4);
    let lichen = fbm(u, v, 8, 3, 73);
    let c = if lichen > 0.66 { c.lerp(hex3(0x6a7040), 0.5) } else { c };
    rgba(c)
}

fn grass(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 4, 81);
    let blade = value_noise(u * 64.0, v * 16.0, 64, 82);
    let c = hex3(0x3d4a22).lerp(hex3(0x6a7036), n) * (0.75 + blade * 0.35);
    let dirt = fbm(u, v, 4, 3, 83);
    rgba(c.lerp(hex3(0x4a3e2a), ((dirt - 0.6) * 3.0).clamp(0.0, 0.7)))
}

fn dirt(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 4, 91);
    let pebble = worley(u, v, 12, 92).0;
    let mut c = hex3(0x4a3c2c).lerp(hex3(0x6a5844), n);
    if pebble < 0.18 {
        c = c * 1.25;
    }
    rgba(c)
}

fn steel(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 3, 101);
    let scratch = value_noise(u * 64.0, v * 4.0, 64, 102);
    let c = hex3(0x8c8e92).lerp(hex3(0xb8bcc0), n) * (0.85 + scratch * 0.2);
    // Rivet rows.
    let rv = (v * 4.0).fract();
    let ru = (u * 8.0).fract();
    let c = if (rv - 0.5).abs() < 0.08 && (ru - 0.5).abs() < 0.12 { c * 0.6 } else { c };
    rgba(c)
}

fn cloth(u: f32, v: f32, col: Vec3) -> Vec4 {
    let weave = ((u * 64.0).floor() as i32 + (v * 64.0).floor() as i32) % 2;
    let n = fbm(u, v, 4, 3, 111);
    let fold = (u * std::f32::consts::TAU * 3.0 + n * 3.0).sin() * 0.5 + 0.5;
    let c = col * (0.7 + fold * 0.3 + weave as f32 * 0.05) * (0.85 + n * 0.25);
    rgba(c)
}

fn chainmail(u: f32, v: f32) -> Vec4 {
    let ring_u = (u * 16.0 + if ((v * 16.0) as i32) % 2 == 0 { 0.0 } else { 0.5 }).fract();
    let ring_v = (v * 16.0).fract();
    let d = ((ring_u - 0.5).powi(2) + (ring_v - 0.5).powi(2)).sqrt();
    let ring = (d - 0.32).abs() < 0.13;
    let c = if ring { hex3(0x8a8a88) } else { hex3(0x4a4a4c) };
    rgba(c * (0.8 + fbm(u, v, 4, 2, 121) * 0.3))
}

fn leather(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 4, 131);
    let c = hex3(0x4a3220).lerp(hex3(0x6e4a2e), n);
    let stitch = (u * 4.0).fract() < 0.04 && ((v * 32.0) as i32) % 2 == 0;
    rgba(if stitch { hex3(0xa08a60) } else { c })
}

fn husk_skin(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 4, 141);
    let (w, _) = worley(u, v, 10, 142);
    let mut c = hex3(0x6e5a44).lerp(hex3(0x8e7658), n) * (0.8 + w * 0.3);
    // Ribs / sinew stripes.
    let rib = ((v * 10.0 + n * 0.8) * std::f32::consts::TAU).sin();
    if rib > 0.8 {
        c *= 0.75;
    }
    let sore = fbm(u, v, 4, 3, 143);
    if sore > 0.68 {
        c = c.lerp(hex3(0x5a2a1e), 0.6);
    }
    rgba(c)
}

fn demon_skin(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 4, 5, 151);
    let (w, id) = worley(u, v, 8, 152);
    let mut c = hex3(0x4a5560).lerp(hex3(0x6c7884), n) * (0.75 + w * 0.35);
    if w < 0.25 {
        c = c.lerp(hex3(0x8a96a0), 0.35 * id);
    }
    let vein = (fbm(u, v, 6, 3, 153) - 0.5).abs();
    if vein < 0.025 {
        c = hex3(0x30363e);
    }
    rgba(c)
}

fn demon_belly(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 4, 4, 161);
    let band = (v * 6.0 + n * 0.5).fract();
    let mut c = hex3(0x7c7a6c).lerp(hex3(0x9a967e), n);
    if band < 0.12 {
        c *= 0.55;
    }
    rgba(c)
}

fn stone_club(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 4, 5, 171);
    let (w, _) = worley(u, v, 6, 172);
    let c = hex3(0x4a4642).lerp(hex3(0x6e6860), n) * (0.7 + w * 0.4);
    rgba(c)
}

fn radial(u: f32, v: f32) -> f32 {
    ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt() * 2.0
}

fn flame(u: f32, v: f32) -> Vec4 {
    // Teardrop flame shape, white-yellow core to orange-red edge.
    let x = (u - 0.5) * 2.0;
    let y = v; // 0 top, 1 bottom
    let width = (y * 1.2).min(1.0).sqrt() * (1.0 - y * 0.25) * 0.85;
    let wob = (n2(u, v, 4, 181) - 0.5) * 0.25;
    let d = (x + wob).abs() / width.max(0.01);
    if d > 1.0 || y < 0.05 {
        return Vec4::ZERO;
    }
    let heat = (1.0 - d) * (0.4 + y * 0.6);
    let c = hex3(0xb02810).lerp(hex3(0xff8a20), (heat * 2.0).min(1.0)).lerp(hex3(0xfff0a0), ((heat - 0.5) * 2.0).clamp(0.0, 1.0));
    rgba(c)
}

fn soft(u: f32, v: f32) -> Vec4 {
    let r = radial(u, v) + (n2(u, v, 8, 191) - 0.5) * 0.3;
    if r > 1.0 { Vec4::ZERO } else { rgba(Vec3::splat(1.0 - r * 0.5)) }
}

fn blood(u: f32, v: f32) -> Vec4 {
    let r = radial(u, v) + (n2(u, v, 6, 201) - 0.5) * 0.6;
    if r > 0.85 { Vec4::ZERO } else { rgba(hex3(0x7a0a08).lerp(hex3(0xb01a10), 1.0 - r)) }
}

fn spark(u: f32, v: f32) -> Vec4 {
    let x = (u - 0.5).abs();
    let y = (v - 0.5).abs();
    let star = (x * y * 60.0).min(1.0) + radial(u, v) * 0.6;
    if star > 0.55 { Vec4::ZERO } else { rgba(Vec3::new(1.0, 0.85, 0.5).lerp(Vec3::ONE, 1.0 - star * 1.5)) }
}

fn fog_veil(u: f32, v: f32) -> Vec4 {
    let swirl = fbm(u + (v * 6.0).sin() * 0.05, v, 4, 4, 211);
    let c = Vec3::splat(0.55 + swirl * 0.45);
    rgba(c)
}

fn clouds(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 4, 6, 221);
    let c = hex3(0x5c6068).lerp(hex3(0xa8acb0), ((n - 0.35) * 1.8).clamp(0.0, 1.0));
    rgba(c)
}

fn ash(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 4, 231);
    let (w, _) = worley(u, v, 10, 232);
    let mut c = hex3(0x3a3632).lerp(hex3(0x8a8478), n);
    if w < 0.15 {
        c = hex3(0xc8c0b0) * (0.8 + n * 0.2); // bone fragments
    }
    rgba(c)
}

fn shadow(u: f32, v: f32) -> Vec4 {
    if radial(u, v) > 0.95 { Vec4::ZERO } else { rgba(Vec3::ZERO) }
}

fn water(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 4, 4, 241);
    let ripple = ((u + v + n * 0.3) * std::f32::consts::TAU * 4.0).sin() * 0.5 + 0.5;
    rgba(hex3(0x1a2a28).lerp(hex3(0x3a5450), n * 0.6 + ripple * 0.3))
}

fn ceramic(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 3, 251);
    let mut c = hex3(0x7a5a3a).lerp(hex3(0x9a7448), n);
    // Painted bands.
    let band = v.fract();
    if (band - 0.3).abs() < 0.03 || (band - 0.7).abs() < 0.02 {
        c = hex3(0x3a2a1a);
    }
    let chip = worley(u, v, 6, 252).0;
    if chip < 0.06 {
        c = hex3(0x5a4a3a);
    }
    rgba(c)
}

fn iron(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 4, 261);
    let rust = fbm(u, v, 4, 3, 262);
    let c = hex3(0x3c3a38).lerp(hex3(0x585450), n).lerp(hex3(0x6a3a1c), ((rust - 0.5) * 2.5).clamp(0.0, 0.8));
    rgba(c)
}

fn kite_shield(u: f32, v: f32) -> Vec4 {
    // Heater/kite shield face: silver rim, red field with a pale cross-and-sun crest.
    let x = (u - 0.5) * 2.0;
    let y = v;
    let rim = x.abs() > 0.86 || y < 0.06 || y > 0.94;
    let n = fbm(u, v, 8, 3, 271);
    if rim {
        return rgba(hex3(0x9a9c9e) * (0.8 + n * 0.3));
    }
    let mut c = hex3(0x7a1c18) * (0.8 + n * 0.3);
    // Chevron.
    let chev = (y - 0.2 - x.abs() * 0.45).abs() < 0.07;
    // Sun disc.
    let sun = ((x).powi(2) + (y - 0.55).powi(2)).sqrt() < 0.18;
    if chev || sun {
        c = hex3(0xd8c89a) * (0.85 + n * 0.2);
    }
    rgba(c)
}

fn glow(u: f32, v: f32) -> Vec4 {
    let r = radial(u, v);
    if r > 1.0 { Vec4::ZERO } else { rgba(Vec3::splat((1.0 - r).powf(0.7))) }
}

fn rune_ring(u: f32, v: f32) -> Vec4 {
    let r = radial(u, v);
    let ang = (v - 0.5).atan2(u - 0.5);
    let glyph = ((ang * 8.0).sin() * (r * 30.0).sin()) > 0.2;
    if (r - 0.8).abs() < 0.06 || (r < 0.7 && r > 0.25 && glyph) || r < 0.12 {
        rgba(hex3(0xff9a40))
    } else {
        Vec4::ZERO
    }
}

fn gravestone(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 4, 281);
    let c = hex3(0x5e5c58).lerp(hex3(0x7e7a72), n);
    let line = (v > 0.25 && v < 0.6) && ((v * 16.0).fract() < 0.15) && (u > 0.25 && u < 0.75) && n > 0.4;
    rgba(if line { c * 0.6 } else { c })
}

fn helm_face(u: f32, v: f32) -> Vec4 {
    // Front of a great helm: dark visor slit and breaths.
    let base = steel(u, v).truncate();
    let slit = (v - 0.4).abs() < 0.06 && (u - 0.5).abs() < 0.38;
    let gap = (u - 0.5).abs() < 0.03 && v > 0.4;
    let holes = v > 0.6 && v < 0.85 && ((u * 10.0).fract() < 0.3) && (u - 0.5).abs() < 0.3 && ((v * 8.0).fract() < 0.4);
    rgba(if slit { hex3(0x080808) } else if holes || gap { base * 0.4 } else { base })
}

fn husk_face(u: f32, v: f32) -> Vec4 {
    let base = husk_skin(u, v).truncate();
    let eye = |cx: f32| ((u - cx).powi(2) * 3.0 + (v - 0.45).powi(2) * 6.0) < 0.012;
    let mouth = (v - 0.72).abs() < 0.04 && (u - 0.5).abs() < 0.14;
    rgba(if eye(0.35) || eye(0.65) { hex3(0x100806) } else if mouth { hex3(0x2a1410) } else { base })
}

fn demon_face(u: f32, v: f32) -> Vec4 {
    let base = demon_skin(u, v).truncate();
    let eye = |cx: f32| ((u - cx).powi(2) + (v - 0.35).powi(2)) < 0.004;
    let mouth = (v - 0.7).abs() < 0.09 && (u - 0.5).abs() < 0.32;
    let teeth = mouth && ((u * 14.0).fract() < 0.45) && (v - 0.7).abs() > 0.03;
    rgba(if eye(0.36) || eye(0.64) {
        hex3(0xd0b040)
    } else if teeth {
        hex3(0xb8b098)
    } else if mouth {
        hex3(0x200808)
    } else {
        base
    })
}

fn rubble(u: f32, v: f32) -> Vec4 {
    let (w, id) = worley(u, v, 8, 291);
    let c = hex3(0x5a544c).lerp(hex3(0x7a7266), id) * (0.6 + w * 0.6);
    rgba(c)
}

fn bone(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 3, 301);
    rgba(hex3(0xa89e86).lerp(hex3(0xd0c8b0), n))
}

fn feather(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 3, 311);
    let barb = ((u * 20.0 + v * 6.0).fract() < 0.5) as i32 as f32;
    rgba(hex3(0x141414).lerp(hex3(0x2a2a30), n * 0.6 + barb * 0.3))
}

fn plaster(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 4, 5, 321);
    let mut c = hex3(0x6a6458).lerp(hex3(0x8a8272), n);
    let stain = fbm(u, v, 2, 3, 322);
    c = c.lerp(hex3(0x3e3a30), ((stain - 0.55) * 2.0).clamp(0.0, 0.6));
    // Exposed bricks.
    if fbm(u, v, 3, 3, 323) > 0.62 {
        c = stone_brick(u, v, 0.0).truncate() * 0.9;
    }
    rgba(c)
}

fn stairs(u: f32, v: f32) -> Vec4 {
    // Tread with worn front edge.
    let n = fbm(u, v, 8, 3, 331);
    let mut c = hex3(0x6a6258).lerp(hex3(0x847a6c), n);
    if v < 0.08 {
        c *= 1.2;
    }
    if v > 0.92 {
        c *= 0.6;
    }
    rgba(c)
}

fn moss(u: f32, v: f32) -> Vec4 {
    let n = fbm(u, v, 8, 4, 341);
    rgba(hex3(0x2e3a1a).lerp(hex3(0x56602c), n))
}

/// Generates every texture in [`Tex`] order.
pub fn generate_all() -> Vec<TextureData> {
    Tex::ALL
        .iter()
        .map(|t| {
            let (f, colors): (fn(f32, f32) -> Vec4, usize) = match t {
                Tex::White => (|_, _| Vec4::ONE, 2),
                Tex::StoneBrick => (|u, v| stone_brick(u, v, 0.0), 16),
                Tex::MossBrick => (|u, v| stone_brick(u, v, 1.0), 16),
                Tex::Flagstone => (|u, v| flagstone(u, v, 4, hex3(0x6e665a)), 16),
                Tex::ArenaFloor => (arena_floor, 16),
                Tex::WoodPlank => (|u, v| wood(u, v, false), 16),
                Tex::IronBars => (iron_bars, 8),
                Tex::Rock => (rock, 16),
                Tex::Grass => (grass, 16),
                Tex::Dirt => (dirt, 16),
                Tex::Steel => (steel, 16),
                Tex::ClothRed => (|u, v| cloth(u, v, hex3(0x7a2a20)), 16),
                Tex::Chainmail => (chainmail, 8),
                Tex::Leather => (leather, 16),
                Tex::HuskSkin => (husk_skin, 16),
                Tex::DemonSkin => (demon_skin, 16),
                Tex::DemonBelly => (demon_belly, 16),
                Tex::StoneClub => (stone_club, 16),
                Tex::Flame => (flame, 16),
                Tex::Soft => (soft, 8),
                Tex::Blood => (blood, 8),
                Tex::Spark => (spark, 8),
                Tex::FogVeil => (fog_veil, 16),
                Tex::Clouds => (clouds, 16),
                Tex::Ash => (ash, 16),
                Tex::Shadow => (shadow, 2),
                Tex::Water => (water, 16),
                Tex::Ceramic => (ceramic, 16),
                Tex::Iron => (iron, 16),
                Tex::DarkWood => (|u, v| wood(u, v, true), 16),
                Tex::KiteShield => (kite_shield, 16),
                Tex::Glow => (glow, 16),
                Tex::RuneRing => (rune_ring, 4),
                Tex::Gravestone => (gravestone, 16),
                Tex::HelmFace => (helm_face, 16),
                Tex::HuskFace => (husk_face, 16),
                Tex::DemonFace => (demon_face, 16),
                Tex::Rubble => (rubble, 16),
                Tex::ClothDark => (|u, v| cloth(u, v, hex3(0x3a3430)), 16),
                Tex::Bone => (bone, 16),
                Tex::Feather => (feather, 8),
                Tex::Plaster => (plaster, 16),
                Tex::Stairs => (stairs, 16),
                Tex::Moss => (moss, 16),
            };
            let mut tex = TextureData::from_fn(TEX_SIZE, f);
            tex.quantize(colors);
            tex
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_textures_generate() {
        let t = generate_all();
        assert_eq!(t.len(), Tex::ALL.len());
        assert!(t.len() < 256);
        assert_eq!(Tex::Moss.layer() as usize, t.len() - 1);
    }
}
