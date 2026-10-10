//! Weapons, items and level props.

use super::textures::Tex;
use ashen_engine::math::{Mat4, Quat, Vec2, Vec3};
use ashen_engine::mesh::{MeshBuilder, MeshData, VF_NOFOG, VF_UNLIT, rgb};
use ashen_engine::rng::Rng;
use std::f32::consts::TAU;

const WHITE: [u8; 4] = [255, 255, 255, 255];

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn v2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// Surface of revolution around +Y from a `(radius, height)` profile.
pub fn lathe(b: &mut MeshBuilder, profile: &[(f32, f32)], segs: u32, v_scale: f32) {
    let base = b.mesh.vertices.len() as u32;
    let rows = profile.len() as u32;
    for (j, &(r, y)) in profile.iter().enumerate() {
        // Approximate normal from neighbouring profile points.
        let prev = profile[j.saturating_sub(1)];
        let next = profile[(j + 1).min(profile.len() - 1)];
        let dr = next.0 - prev.0;
        let dy = next.1 - prev.1;
        for i in 0..=segs {
            let a = i as f32 / segs as f32 * TAU;
            let d = v(a.cos(), 0.0, a.sin());
            let n = (d * dy - Vec3::Y * dr).normalize_or(d);
            b.vertex(d * r + Vec3::Y * y, n, v2(i as f32 / segs as f32, 1.0 - y * v_scale));
        }
    }
    let row = segs + 1;
    for j in 0..rows - 1 {
        for i in 0..segs {
            let a = base + j * row + i;
            let c = a + row;
            b.tri_idx(a, c, c + 1);
            b.tri_idx(a, c + 1, a + 1);
        }
    }
}

/// Longsword held along +Z from the fist (blade width along local Y).
pub fn longsword() -> MeshData {
    let mut b = MeshBuilder::new();
    b.push(Mat4::from_translation(crate::rig::humanoid::FIST));
    b.material(Tex::Steel.layer(), rgb(200, 190, 170));
    b.ellipsoid(v(0.0, 0.0, -0.11), v(0.025, 0.025, 0.025), 5, 3);
    b.material(Tex::Leather.layer(), WHITE);
    b.cylinder(v(0.0, 0.0, -0.09), v(0.0, 0.0, 0.06), 0.018, 0.018, 5, false, false);
    b.material(Tex::Steel.layer(), rgb(180, 170, 150));
    b.box_c(v(0.0, 0.0, 0.07), v(0.014, 0.11, 0.014));
    b.material(Tex::Steel.layer(), rgb(255, 255, 255));
    let base = 0.085;
    let tip = 0.98;
    b.hexa(
        [v(-0.007, -0.028, base), v(0.007, -0.028, base), v(0.007, 0.028, base), v(-0.007, 0.028, base)],
        [v(-0.003, -0.004, tip - 0.06), v(0.003, -0.004, tip - 0.06), v(0.003, 0.004, tip - 0.06), v(-0.003, 0.004, tip - 0.06)],
        v2(1.0, 4.0),
    );
    b.hexa(
        [v(-0.003, -0.004, tip - 0.06), v(0.003, -0.004, tip - 0.06), v(0.003, 0.004, tip - 0.06), v(-0.003, 0.004, tip - 0.06)],
        [v(-0.0005, -0.0005, tip), v(0.0005, -0.0005, tip), v(0.0005, 0.0005, tip), v(-0.0005, 0.0005, tip)],
        Vec2::ONE,
    );
    b.pop();
    b.finish()
}

/// Kite shield; face normal +Z, top towards +Y, held at its centre.
pub fn kite_shield() -> MeshData {
    let mut b = MeshBuilder::new();
    b.push(Mat4::from_translation(crate::rig::humanoid::FIST + v(0.0, 0.05, 0.05)));
    let outline = [v(-0.25, 0.30, 0.0), v(0.25, 0.30, 0.0), v(0.23, -0.12, 0.0), v(0.0, -0.50, 0.0), v(-0.23, -0.12, 0.0)];
    let uv = |p: Vec3| v2((0.25 - p.x) / 0.5, (0.30 - p.y) / 0.8);
    let front = 0.03;
    let back = -0.01;
    // Slight curvature: centre bulges forward.
    let bulge = 0.025;
    let center_f = v(0.0, 0.0, front + bulge);
    b.material(Tex::KiteShield.layer(), WHITE);
    for i in 0..outline.len() {
        let a = outline[i] + v(0.0, 0.0, front);
        let c = outline[(i + 1) % outline.len()] + v(0.0, 0.0, front);
        // Front fan, counter-clockwise seen from +Z.
        b.tri([center_f, c, a], [uv(center_f), uv(c), uv(a)]);
    }
    b.material(Tex::DarkWood.layer(), rgb(150, 140, 130));
    let center_b = v(0.0, 0.0, back);
    for i in 0..outline.len() {
        let a = outline[i] + v(0.0, 0.0, back);
        let c = outline[(i + 1) % outline.len()] + v(0.0, 0.0, back);
        b.tri([center_b, a, c], [uv(center_b), uv(a), uv(c)]);
    }
    b.material(Tex::Steel.layer(), rgb(170, 170, 170));
    for i in 0..outline.len() {
        let a = outline[i];
        let c = outline[(i + 1) % outline.len()];
        b.quad_out(
            [a + v(0.0, 0.0, back), c + v(0.0, 0.0, back), c + v(0.0, 0.0, front), a + v(0.0, 0.0, front)],
            [v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 0.2), v2(0.0, 0.2)],
            v(0.0, 0.0, 0.01),
        );
    }
    b.pop();
    b.finish()
}

pub fn broken_sword() -> MeshData {
    let mut b = MeshBuilder::new();
    b.push(Mat4::from_translation(crate::rig::humanoid::FIST));
    b.material(Tex::Leather.layer(), rgb(120, 100, 80));
    b.cylinder(v(0.0, 0.0, -0.09), v(0.0, 0.0, 0.06), 0.018, 0.018, 5, false, false);
    b.material(Tex::Iron.layer(), WHITE);
    b.box_c(v(0.0, 0.0, 0.07), v(0.012, 0.09, 0.012));
    b.hexa(
        [v(-0.006, -0.026, 0.08), v(0.006, -0.026, 0.08), v(0.006, 0.026, 0.08), v(-0.006, 0.026, 0.08)],
        [v(-0.005, -0.024, 0.38), v(0.005, -0.024, 0.38), v(0.005, 0.01, 0.42), v(-0.005, 0.01, 0.42)],
        v2(1.0, 2.0),
    );
    b.pop();
    b.finish()
}

/// Bow held in the left fist; arrow direction +Z, limbs along Y.
pub fn bow() -> MeshData {
    let mut b = MeshBuilder::new();
    b.push(Mat4::from_translation(crate::rig::humanoid::FIST));
    b.material(Tex::DarkWood.layer(), WHITE);
    let pts: Vec<Vec3> = (0..=8)
        .map(|i| {
            let t = i as f32 / 8.0 * 2.0 - 1.0;
            v(0.0, t * 0.62, -0.14 * t * t)
        })
        .collect();
    for w in pts.windows(2) {
        b.cylinder(w[0], w[1], 0.018, 0.018, 4, false, false);
    }
    b.material(Tex::White.layer(), rgb(180, 170, 150));
    b.cylinder(pts[0], pts[8], 0.003, 0.003, 3, false, false);
    b.pop();
    b.finish()
}

pub fn arrow() -> MeshData {
    let mut b = MeshBuilder::new();
    b.material(Tex::DarkWood.layer(), rgb(200, 180, 150));
    b.cylinder(v(0.0, 0.0, -0.4), v(0.0, 0.0, 0.3), 0.008, 0.008, 4, false, false);
    b.material(Tex::Iron.layer(), WHITE);
    b.cylinder(v(0.0, 0.0, 0.3), v(0.0, 0.0, 0.38), 0.018, 0.0, 4, false, false);
    b.material(Tex::Feather.layer(), rgb(200, 200, 200));
    b.quad_n([v(0.0, -0.03, -0.4), v(0.0, -0.03, -0.28), v(0.0, 0.03, -0.28), v(0.0, 0.03, -0.4)], [v2(0.0, 1.0), v2(1.0, 1.0), v2(1.0, 0.0), v2(0.0, 0.0)], Vec3::X);
    b.quad_n([v(0.0, 0.03, -0.4), v(0.0, 0.03, -0.28), v(0.0, -0.03, -0.28), v(0.0, -0.03, -0.4)], [v2(0.0, 0.0), v2(1.0, 0.0), v2(1.0, 1.0), v2(0.0, 1.0)], Vec3::NEG_X);
    b.finish()
}

/// Ember flask held in the fist; glows.
pub fn flask() -> MeshData {
    let mut b = MeshBuilder::new();
    b.push(Mat4::from_translation(crate::rig::humanoid::FIST) * Mat4::from_rotation_x(-1.2));
    b.flags = VF_UNLIT;
    b.material(Tex::Glow.layer(), rgb(255, 150, 60));
    lathe(&mut b, &[(0.0, -0.06), (0.05, -0.05), (0.055, 0.02), (0.02, 0.06), (0.018, 0.1), (0.0, 0.1)], 6, 1.0);
    b.pop();
    b.finish()
}

/// The demon's stone club; handle along +Z from the fist.
pub fn demon_club() -> MeshData {
    let mut b = MeshBuilder::new();
    b.push(Mat4::from_translation(v(0.0, -0.25, 0.0)));
    b.material(Tex::DarkWood.layer(), rgb(160, 140, 120));
    b.cylinder(v(0.0, 0.0, -0.5), v(0.0, 0.0, 1.5), 0.13, 0.15, 6, true, false);
    b.material(Tex::StoneClub.layer(), WHITE);
    b.cylinder(v(0.0, 0.0, 1.3), v(0.0, 0.0, crate::rig::demon::CLUB_LEN), 0.42, 0.55, 7, true, false);
    let mut r = Rng::new(17);
    for _ in 0..6 {
        let z = r.range(1.5, 3.2);
        let a = r.range(0.0, TAU);
        b.ellipsoid(v(a.cos() * 0.42, a.sin() * 0.42, z), Vec3::splat(r.range(0.15, 0.25)), 5, 3);
    }
    b.material(Tex::Iron.layer(), WHITE);
    b.cylinder(v(0.0, 0.0, 1.25), v(0.0, 0.0, 1.38), 0.47, 0.47, 7, false, false);
    b.pop();
    b.finish()
}

/// Bonfire-like pyre: ash mound, stone ring, coiled blade.
pub fn pyre() -> MeshData {
    let mut b = MeshBuilder::new();
    b.material(Tex::Ash.layer(), WHITE);
    b.ellipsoid_part(v(0.0, 0.0, 0.0), v(0.75, 0.28, 0.75), 9, 3, 0.0, 0.5);
    b.material(Tex::Rock.layer(), rgb(170, 160, 150));
    let mut r = Rng::new(5);
    for i in 0..9 {
        let a = i as f32 / 9.0 * TAU + r.range(-0.1, 0.1);
        let c = v(a.cos() * 0.85, 0.06, a.sin() * 0.85);
        b.push(Mat4::from_translation(c) * Mat4::from_rotation_y(-a));
        b.box_c(Vec3::ZERO, v(0.1, 0.08 + r.range(0.0, 0.04), 0.14));
        b.pop();
    }
    b.material(Tex::Bone.layer(), WHITE);
    for _ in 0..5 {
        let a = r.range(0.0, TAU);
        let d = r.range(0.2, 0.55);
        b.push(Mat4::from_translation(v(a.cos() * d, 0.15, a.sin() * d)) * Mat4::from_rotation_y(r.range(0.0, TAU)));
        b.box_c(Vec3::ZERO, v(0.12, 0.02, 0.025));
        b.pop();
    }
    // Coiled blade, slightly tilted.
    b.push(Mat4::from_translation(v(0.0, 0.1, 0.0)) * Mat4::from_rotation_z(0.12));
    b.material(Tex::Iron.layer(), rgb(150, 110, 90));
    b.hexa(
        [v(-0.01, 0.0, -0.035), v(0.01, 0.0, -0.035), v(0.01, 0.0, 0.035), v(-0.01, 0.0, 0.035)],
        [v(-0.008, 0.9, -0.03), v(0.008, 0.9, -0.03), v(0.008, 0.9, 0.03), v(-0.008, 0.9, 0.03)],
        v2(1.0, 3.0),
    );
    b.box_c(v(0.0, 0.92, 0.0), v(0.012, 0.012, 0.11));
    b.cylinder(v(0.0, 0.93, 0.0), v(0.0, 1.08, 0.0), 0.02, 0.02, 4, false, false);
    b.material(Tex::Iron.layer(), rgb(110, 80, 60));
    for i in 0..18 {
        let t = i as f32 / 18.0;
        let a = t * TAU * 3.0;
        let y = 0.15 + t * 0.7;
        b.push(Mat4::from_translation(v(a.cos() * 0.05, y, a.sin() * 0.05)) * Mat4::from_rotation_y(-a));
        b.box_c(Vec3::ZERO, v(0.012, 0.015, 0.03));
        b.pop();
    }
    b.pop();
    b.finish()
}

pub fn urn(seed: u64) -> MeshData {
    let mut r = Rng::new(seed);
    let w = r.range(0.85, 1.15);
    let h = r.range(0.85, 1.25);
    let mut b = MeshBuilder::new();
    b.material(Tex::Ceramic.layer(), [(200.0 * r.range(0.85, 1.1)) as u8, (190.0 * r.range(0.85, 1.05)) as u8, 170, 255]);
    let prof: Vec<(f32, f32)> = [(0.0, 0.0), (0.2, 0.0), (0.3, 0.15), (0.36, 0.4), (0.3, 0.68), (0.16, 0.82), (0.15, 0.92), (0.2, 0.98), (0.17, 1.0)]
        .iter()
        .map(|&(rr, y)| (rr * w, y * h))
        .collect();
    lathe(&mut b, &prof, 8, 1.0);
    b.finish()
}

pub fn shard() -> MeshData {
    let mut b = MeshBuilder::new();
    b.material(Tex::Ceramic.layer(), WHITE);
    let p = [v(-0.08, 0.0, 0.0), v(0.09, 0.02, 0.0), v(0.0, 0.1, 0.01)];
    b.tri(p, [v2(0.0, 0.0), v2(1.0, 0.0), v2(0.5, 1.0)]);
    b.tri([p[0], p[2], p[1]], [v2(0.0, 0.0), v2(0.5, 1.0), v2(1.0, 0.0)]);
    b.finish()
}

/// Single door leaf hinged at its local origin, extending along +X.
pub fn door_leaf(width: f32, height: f32, thickness: f32, big: bool) -> MeshData {
    let mut b = MeshBuilder::new();
    b.material(if big { Tex::DarkWood } else { Tex::WoodPlank }.layer(), WHITE);
    b.cube(v(0.0, 0.0, -thickness * 0.5), v(width, height, thickness * 0.5), 1.0);
    b.material(Tex::Iron.layer(), WHITE);
    let bands = if big { 5 } else { 3 };
    for i in 0..bands {
        let y = height * (i as f32 + 0.5) / bands as f32;
        b.cube(v(0.02, y - 0.06, -thickness * 0.5 - 0.02), v(width - 0.02, y + 0.06, thickness * 0.5 + 0.02), 2.0);
    }
    if big {
        let mut r = Rng::new(3);
        for i in 0..14 {
            let x = 0.3 + (i % 4) as f32 * (width - 0.6) / 3.0;
            let y = 0.5 + (i / 4) as f32 * (height - 1.0) / 3.0 + r.range(-0.1, 0.1);
            b.ellipsoid(v(x, y, thickness * 0.5 + 0.02), Vec3::splat(0.06), 4, 3);
        }
    } else {
        b.cylinder(v(width - 0.2, 1.1, thickness * 0.5), v(width - 0.2, 1.1, thickness * 0.5 + 0.06), 0.05, 0.05, 6, true, false);
    }
    b.finish()
}

/// Barred gate panel centred on X, from y=0 up, see-through (cutout texture).
pub fn bars(width: f32, height: f32) -> MeshData {
    let mut b = MeshBuilder::new();
    b.material(Tex::IronBars.layer(), WHITE);
    let t = 0.04;
    let uw = width / 0.8;
    let vh = height / 1.2;
    for (z, n) in [(t, 1.0f32), (-t, -1.0)] {
        let p = [v(-width / 2.0, 0.0, z), v(width / 2.0, 0.0, z), v(width / 2.0, height, z), v(-width / 2.0, height, z)];
        let uv = [v2(0.0, vh), v2(uw, vh), v2(uw, 0.0), v2(0.0, 0.0)];
        if n > 0.0 {
            b.quad(p, uv);
        } else {
            b.quad([p[1], p[0], p[3], p[2]], [uv[1], uv[0], uv[3], uv[2]]);
        }
    }
    b.material(Tex::Iron.layer(), WHITE);
    b.cube(v(-width / 2.0, height - 0.1, -0.06), v(width / 2.0, height, 0.06), 2.0);
    b.cube(v(-width / 2.0, 0.0, -0.06), v(width / 2.0, 0.08, 0.06), 2.0);
    b.finish()
}

pub fn boulder() -> MeshData {
    let mut b = MeshBuilder::new();
    b.material(Tex::Rock.layer(), WHITE);
    b.ellipsoid(Vec3::ZERO, Vec3::splat(1.0), 8, 6);
    // Jitter vertices for a rough, lumpy look.
    let mut r = Rng::new(9);
    for vtx in b.mesh.vertices.iter_mut() {
        let p = Vec3::from(vtx.pos);
        let k = 1.0 + r.range(-0.08, 0.08);
        vtx.pos = (p * k).to_array();
    }
    b.finish()
}

/// Vertical fog veil plane centred on X, from y=0 up, spanning `width`.
pub fn fog_veil(width: f32, height: f32) -> MeshData {
    let mut b = MeshBuilder::new();
    b.flags = VF_UNLIT;
    b.material(Tex::FogVeil.layer(), rgb(200, 205, 215));
    let nx = (width / 1.0).ceil() as u32;
    let ny = (height / 1.0).ceil() as u32;
    let o = v(-width / 2.0, 0.0, 0.0);
    b.grid_quad(o, v(width, 0.0, 0.0), v(0.0, height, 0.0), nx, ny, &|p| v2(p.x / 3.0, -p.y / 3.0), &|p| {
        // Fade the edges a little through vertex colour.
        let ex = 1.0 - ((p.x.abs() / (width / 2.0)).powi(4));
        let ey = 1.0 - ((p.y / height - 0.5).abs() * 2.0).powi(4);
        let k = (ex * ey).clamp(0.2, 1.0);
        [(200.0 * k) as u8, (205.0 * k) as u8, (215.0 * k) as u8, 255]
    });
    b.finish()
}

pub fn torch() -> MeshData {
    let mut b = MeshBuilder::new();
    b.material(Tex::Iron.layer(), WHITE);
    b.box_c(v(0.0, 0.0, 0.03), v(0.06, 0.1, 0.03));
    b.cylinder(v(0.0, -0.02, 0.05), v(0.0, 0.1, 0.25), 0.02, 0.02, 4, false, false);
    b.cylinder(v(0.0, 0.08, 0.25), v(0.0, 0.18, 0.25), 0.06, 0.07, 5, true, false);
    b.material(Tex::DarkWood.layer(), WHITE);
    b.cylinder(v(0.0, 0.0, 0.25), v(0.0, 0.32, 0.27), 0.035, 0.05, 5, true, false);
    b.finish()
}

/// Generic fallen-body pile (decoration / item holder).
pub fn corpse(seed: u64) -> MeshData {
    let mut r = Rng::new(seed);
    let mut b = MeshBuilder::new();
    b.material(Tex::ClothDark.layer(), rgb(140, 125, 105));
    b.push(Mat4::from_rotation_y(r.range(0.0, TAU)));
    b.ellipsoid(v(0.0, 0.12, 0.0), v(0.22, 0.13, 0.42), 6, 4);
    b.material(Tex::HuskSkin.layer(), rgb(170, 150, 130));
    b.ellipsoid(v(0.05, 0.12, 0.5), v(0.1, 0.1, 0.12), 5, 3);
    b.cylinder(v(0.15, 0.08, 0.2), v(0.45, 0.05, 0.35), 0.04, 0.03, 4, true, false);
    b.cylinder(v(-0.1, 0.08, -0.3), v(-0.15, 0.05, -0.8), 0.05, 0.04, 4, true, false);
    b.cylinder(v(0.1, 0.08, -0.3), v(0.25, 0.05, -0.75), 0.05, 0.04, 4, true, false);
    b.pop();
    b.finish()
}

pub fn gravestone(seed: u64) -> MeshData {
    let mut r = Rng::new(seed);
    let mut b = MeshBuilder::new();
    b.material(Tex::Gravestone.layer(), WHITE);
    let h = r.range(0.7, 1.1);
    b.push(Mat4::from_rotation_z(r.range(-0.15, 0.15)) * Mat4::from_rotation_y(r.range(-0.2, 0.2)));
    b.hexa(
        [v(-0.3, 0.0, -0.08), v(0.3, 0.0, -0.08), v(0.3, 0.0, 0.08), v(-0.3, 0.0, 0.08)],
        [v(-0.28, h, -0.07), v(0.28, h, -0.07), v(0.28, h, 0.07), v(-0.28, h, 0.07)],
        Vec2::ONE,
    );
    b.cylinder(v(0.0, h, -0.07), v(0.0, h, 0.07), 0.28, 0.28, 6, true, false);
    b.pop();
    b.finish()
}

pub fn rubble(seed: u64, spread: f32) -> MeshData {
    let mut r = Rng::new(seed);
    let mut b = MeshBuilder::new();
    b.material(Tex::Rubble.layer(), WHITE);
    for _ in 0..14 {
        let c = v(r.range(-spread, spread), r.range(0.05, 0.2), r.range(-spread, spread));
        b.push(Mat4::from_rotation_translation(Quat::from_euler(glam::EulerRot::YXZ, r.range(0.0, 6.0), r.range(-0.5, 0.5), r.range(-0.5, 0.5)), c));
        b.box_c(Vec3::ZERO, v(r.range(0.1, 0.35), r.range(0.06, 0.2), r.range(0.1, 0.3)));
        b.pop();
    }
    b.finish()
}

/// Giant twig nest at the cliff top.
pub fn nest() -> MeshData {
    let mut r = Rng::new(77);
    let mut b = MeshBuilder::new();
    b.material(Tex::DarkWood.layer(), rgb(170, 150, 120));
    for i in 0..60 {
        let a = i as f32 / 60.0 * TAU * 3.0 + r.range(-0.2, 0.2);
        let rad = 2.2 + r.range(-0.4, 0.4);
        let y = r.range(0.1, 0.9);
        let p0 = v(a.cos() * rad, y, a.sin() * rad);
        let a2 = a + r.range(0.5, 0.9);
        let p1 = v(a2.cos() * rad, y + r.range(-0.3, 0.3), a2.sin() * rad);
        b.cylinder(p0, p1, 0.06, 0.05, 3, false, false);
    }
    b.material(Tex::Feather.layer(), rgb(80, 80, 80));
    b.ellipsoid_part(v(0.0, 0.1, 0.0), v(2.0, 0.3, 2.0), 8, 2, 0.5, 1.0);
    b.finish()
}

/// Sky dome: gradient vertex colours with a cloud layer; unlit, unfogged.
pub fn sky_dome(top: [u8; 4], horizon: [u8; 4]) -> MeshData {
    let mut b = MeshBuilder::new();
    b.flags = VF_UNLIT | VF_NOFOG;
    b.layer = Tex::Clouds.layer();
    let segs = 12;
    let rings = 5;
    let r = 60.0;
    let base = b.mesh.vertices.len() as u32;
    for j in 0..=rings {
        let el = (j as f32 / rings as f32) * std::f32::consts::FRAC_PI_2 * 1.05 - 0.15;
        for i in 0..=segs {
            let az = i as f32 / segs as f32 * TAU;
            let d = v(az.cos() * el.cos(), el.sin(), az.sin() * el.cos());
            let t = (j as f32 / rings as f32).powf(0.8);
            let c = [
                (horizon[0] as f32 + (top[0] as f32 - horizon[0] as f32) * t) as u8,
                (horizon[1] as f32 + (top[1] as f32 - horizon[1] as f32) * t) as u8,
                (horizon[2] as f32 + (top[2] as f32 - horizon[2] as f32) * t) as u8,
                255,
            ];
            let uv = v2(d.x * 1.5, d.z * 1.5) / (0.3 + d.y.max(0.0));
            b.vertex_c(d * r, -d, uv, c);
        }
    }
    let row = segs + 1;
    for j in 0..rings {
        for i in 0..segs {
            let a = base + j * row + i;
            let c = a + row;
            // Inside-facing.
            b.tri_idx(a, a + 1, c + 1);
            b.tri_idx(a, c + 1, c);
        }
    }
    b.finish()
}

/// Flat circular decal on the floor (rune messages, etc), facing +Y.
pub fn floor_decal(size: f32, tex: Tex, color: [u8; 4]) -> MeshData {
    let mut b = MeshBuilder::new();
    b.flags = VF_UNLIT;
    b.material(tex.layer(), color);
    let h = size / 2.0;
    b.quad([v(-h, 0.02, -h), v(-h, 0.02, h), v(h, 0.02, h), v(h, 0.02, -h)], [v2(0.0, 0.0), v2(0.0, 1.0), v2(1.0, 1.0), v2(1.0, 0.0)]);
    b.finish()
}

/// Small glowing item marker (a corpse with a soft glow is drawn on top).
pub fn item_glint() -> MeshData {
    let mut b = MeshBuilder::new();
    b.flags = VF_UNLIT;
    b.material(Tex::Glow.layer(), rgb(255, 235, 190));
    b.ellipsoid(Vec3::ZERO, Vec3::splat(0.08), 5, 3);
    b.finish()
}
