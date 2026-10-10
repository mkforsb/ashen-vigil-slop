//! Character models: rigid mesh pieces per bone (PSX style, no skinning).

use super::textures::Tex;
use super::{AssetBuilder, ModelDef, Part};
use crate::rig::{demon as db, humanoid as hb};
use ashen_engine::math::{Mat4, Vec2, Vec3};
use ashen_engine::mesh::{MeshBuilder, rgb};

const WHITE: [u8; 4] = [255, 255, 255, 255];

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn v2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// A single textured quad facing +Z (front decals such as faces).
pub fn front_quad(b: &mut MeshBuilder, center: Vec3, half: Vec2, layer: Tex) {
    let prev = b.layer;
    b.layer = layer.layer();
    b.quad(
        [
            center + v(half.x, -half.y, 0.0),
            center + v(-half.x, -half.y, 0.0),
            center + v(-half.x, half.y, 0.0),
            center + v(half.x, half.y, 0.0),
        ],
        // Texture u runs from the character's right (-X) to left (+X) when viewed from the front.
        [v2(1.0, 1.0), v2(0.0, 1.0), v2(0.0, 0.0), v2(1.0, 0.0)],
    );
    b.layer = prev;
}

/// Appearance knobs for humanoid knights.
#[derive(Clone, Copy)]
pub struct KnightLook {
    pub cloth: Tex,
    pub cloth_color: [u8; 4],
    pub metal_color: [u8; 4],
    pub crest: bool,
}

impl KnightLook {
    pub const PLAYER: KnightLook = KnightLook { cloth: Tex::ClothRed, cloth_color: WHITE, metal_color: WHITE, crest: false };
    /// Sir Corvin: older, darker steel with a blue-grey surcoat.
    pub const CORVIN: KnightLook = KnightLook { cloth: Tex::ClothDark, cloth_color: rgb(150, 170, 230), metal_color: rgb(200, 195, 175), crest: true };
}

pub fn knight(ab: &mut AssetBuilder, look: KnightLook) -> ModelDef {
    let mut parts = Vec::new();
    let steel = |b: &mut MeshBuilder| { b.material(Tex::Steel.layer(), look.metal_color); };
    let cloth = |b: &mut MeshBuilder| { b.material(look.cloth.layer(), look.cloth_color); };

    // Pelvis: fauld + mail skirt + tabard flaps.
    let mut b = MeshBuilder::new();
    steel(&mut b);
    b.taper(v(0.0, 0.08, 0.0), v(0.0, -0.10, 0.0), v2(0.17, 0.11), v2(0.19, 0.12));
    b.material(Tex::Chainmail.layer(), WHITE);
    b.taper(v(0.0, -0.06, 0.0), v(0.0, -0.30, 0.0), v2(0.19, 0.12), v2(0.21, 0.14));
    cloth(&mut b);
    b.box_c(v(0.0, -0.20, 0.14), v(0.11, 0.22, 0.012));
    b.box_c(v(0.0, -0.20, -0.14), v(0.11, 0.22, 0.012));
    b.material(Tex::Leather.layer(), WHITE);
    b.box_c(v(0.0, 0.03, 0.0), v(0.195, 0.03, 0.13));
    parts.push(Part::new(hb::PELVIS, ab.add(b.finish())));

    // Abdomen.
    let mut b = MeshBuilder::new();
    b.material(Tex::Chainmail.layer(), WHITE);
    b.taper(v(0.0, 0.27, 0.0), v(0.0, 0.0, 0.0), v2(0.18, 0.12), v2(0.16, 0.11));
    cloth(&mut b);
    b.box_c(v(0.0, 0.13, 0.12), v(0.13, 0.14, 0.012));
    parts.push(Part::new(hb::SPINE, ab.add(b.finish())));

    // Chest: breastplate, surcoat panel, gorget.
    let mut b = MeshBuilder::new();
    steel(&mut b);
    b.taper(v(0.0, 0.25, 0.0), v(0.0, -0.02, 0.0), v2(0.23, 0.14), v2(0.19, 0.12));
    b.cylinder(v(0.0, 0.20, 0.0), v(0.0, 0.30, 0.0), 0.10, 0.08, 6, true, false);
    cloth(&mut b);
    b.taper(v(0.0, 0.20, 0.13), v(0.0, -0.02, 0.12), v2(0.14, 0.014), v2(0.13, 0.012));
    parts.push(Part::new(hb::CHEST, ab.add(b.finish())));

    // Great helm.
    let mut b = MeshBuilder::new();
    steel(&mut b);
    b.taper(v(0.0, 0.26, -0.005), v(0.0, 0.0, 0.0), v2(0.115, 0.125), v2(0.11, 0.12));
    b.ellipsoid_part(v(0.0, 0.26, -0.005), v(0.115, 0.07, 0.125), 8, 3, 0.0, 0.5);
    b.set_color(look.metal_color);
    front_quad(&mut b, v(0.0, 0.13, 0.121), v2(0.11, 0.13), Tex::HelmFace);
    if look.crest {
        b.material(Tex::ClothDark.layer(), rgb(120, 140, 200));
        b.box_c(v(0.0, 0.36, -0.02), v(0.015, 0.04, 0.12));
    }
    parts.push(Part::new(hb::HEAD, ab.add(b.finish())));

    for (upper, fore, hand, side) in [(hb::L_UPPER, hb::L_FORE, hb::L_HAND, 1.0f32), (hb::R_UPPER, hb::R_FORE, hb::R_HAND, -1.0)] {
        let mut b = MeshBuilder::new();
        steel(&mut b);
        b.ellipsoid_part(v(side * 0.02, 0.02, 0.0), v(0.13, 0.12, 0.13), 8, 4, 0.0, 0.62);
        b.material(Tex::Chainmail.layer(), WHITE);
        b.taper(v(0.0, -0.02, 0.0), v(0.0, -hb::UPPER_LEN, 0.0), v2(0.065, 0.065), v2(0.055, 0.055));
        parts.push(Part::new(upper, ab.add(b.finish())));

        let mut b = MeshBuilder::new();
        steel(&mut b);
        b.ellipsoid(v(0.0, 0.0, -0.01), v(0.065, 0.06, 0.065), 6, 4);
        b.taper(v(0.0, -0.02, 0.0), v(0.0, -hb::FORE_LEN, 0.0), v2(0.055, 0.055), v2(0.048, 0.045));
        parts.push(Part::new(fore, ab.add(b.finish())));

        let mut b = MeshBuilder::new();
        b.material(Tex::Leather.layer(), shade_metal(look.metal_color));
        b.box_c(v(0.0, -0.065, 0.005), v(0.045, 0.065, 0.05));
        steel(&mut b);
        b.box_c(v(side * 0.01, -0.02, 0.0), v(0.05, 0.03, 0.055));
        parts.push(Part::new(hand, ab.add(b.finish())));
    }

    for (thigh, shin, foot) in [(hb::L_THIGH, hb::L_SHIN, hb::L_FOOT), (hb::R_THIGH, hb::R_SHIN, hb::R_FOOT)] {
        let mut b = MeshBuilder::new();
        b.material(Tex::Chainmail.layer(), WHITE);
        b.taper(v(0.0, 0.02, 0.0), v(0.0, -hb::THIGH_LEN, 0.0), v2(0.085, 0.085), v2(0.065, 0.07));
        steel(&mut b);
        b.taper(v(0.0, -0.05, 0.05), v(0.0, -0.38, 0.06), v2(0.07, 0.03), v2(0.055, 0.025));
        parts.push(Part::new(thigh, ab.add(b.finish())));

        let mut b = MeshBuilder::new();
        steel(&mut b);
        b.ellipsoid(v(0.0, 0.0, 0.03), v(0.065, 0.06, 0.06), 6, 4);
        b.taper(v(0.0, -0.02, 0.0), v(0.0, -hb::SHIN_LEN, 0.0), v2(0.068, 0.072), v2(0.05, 0.055));
        parts.push(Part::new(shin, ab.add(b.finish())));

        let mut b = MeshBuilder::new();
        steel(&mut b);
        b.hexa(
            [v(-0.055, -0.05, -0.06), v(0.055, -0.05, -0.06), v(0.05, -0.05, 0.17), v(-0.05, -0.05, 0.17)],
            [v(-0.05, 0.03, -0.05), v(0.05, 0.03, -0.05), v(0.035, -0.01, 0.16), v(-0.035, -0.01, 0.16)],
            Vec2::ONE,
        );
        parts.push(Part::new(foot, ab.add(b.finish())));
    }
    ModelDef { parts, radius: 1.2 }
}

fn shade_metal(c: [u8; 4]) -> [u8; 4] {
    [c[0] / 2 + 64, c[1] / 2 + 60, c[2] / 2 + 56, 255]
}

pub fn husk(ab: &mut AssetBuilder, skin_tint: [u8; 4]) -> ModelDef {
    let mut parts = Vec::new();
    let skin = |b: &mut MeshBuilder| { b.material(Tex::HuskSkin.layer(), skin_tint); };

    let mut b = MeshBuilder::new();
    skin(&mut b);
    b.taper(v(0.0, 0.08, 0.0), v(0.0, -0.08, 0.0), v2(0.13, 0.09), v2(0.15, 0.10));
    b.material(Tex::ClothDark.layer(), rgb(170, 150, 120));
    b.taper(v(0.0, 0.0, 0.0), v(0.0, -0.26, 0.02), v2(0.16, 0.11), v2(0.18, 0.12));
    parts.push(Part::new(hb::PELVIS, ab.add(b.finish())));

    let mut b = MeshBuilder::new();
    skin(&mut b);
    b.taper(v(0.0, 0.26, 0.0), v(0.0, 0.0, 0.0), v2(0.12, 0.08), v2(0.11, 0.075));
    parts.push(Part::new(hb::SPINE, ab.add(b.finish())));

    let mut b = MeshBuilder::new();
    skin(&mut b);
    b.taper(v(0.0, 0.24, 0.0), v(0.0, -0.02, 0.0), v2(0.18, 0.10), v2(0.13, 0.08));
    b.cylinder(v(0.0, 0.18, 0.0), v(0.0, 0.30, 0.02), 0.045, 0.04, 5, false, true);
    parts.push(Part::new(hb::CHEST, ab.add(b.finish())));

    let mut b = MeshBuilder::new();
    skin(&mut b);
    b.ellipsoid(v(0.0, 0.14, 0.01), v(0.085, 0.11, 0.10), 7, 5);
    b.set_color(skin_tint);
    front_quad(&mut b, v(0.0, 0.12, 0.095), v2(0.075, 0.09), Tex::HuskFace);
    b.material(Tex::ClothDark.layer(), rgb(90, 80, 70));
    b.ellipsoid_part(v(0.0, 0.17, -0.01), v(0.095, 0.10, 0.11), 7, 3, 0.0, 0.45);
    parts.push(Part::new(hb::HEAD, ab.add(b.finish())));

    for (upper, fore, hand) in [(hb::L_UPPER, hb::L_FORE, hb::L_HAND), (hb::R_UPPER, hb::R_FORE, hb::R_HAND)] {
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.ellipsoid(v(0.0, 0.0, 0.0), v(0.06, 0.06, 0.06), 5, 3);
        b.taper(v(0.0, 0.0, 0.0), v(0.0, -hb::UPPER_LEN, 0.0), v2(0.042, 0.042), v2(0.03, 0.03));
        parts.push(Part::new(upper, ab.add(b.finish())));
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.taper(v(0.0, 0.0, 0.0), v(0.0, -hb::FORE_LEN, 0.0), v2(0.035, 0.035), v2(0.026, 0.026));
        parts.push(Part::new(fore, ab.add(b.finish())));
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.box_c(v(0.0, -0.06, 0.01), v(0.035, 0.06, 0.035));
        parts.push(Part::new(hand, ab.add(b.finish())));
    }
    for (thigh, shin, foot) in [(hb::L_THIGH, hb::L_SHIN, hb::L_FOOT), (hb::R_THIGH, hb::R_SHIN, hb::R_FOOT)] {
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.taper(v(0.0, 0.02, 0.0), v(0.0, -hb::THIGH_LEN, 0.0), v2(0.06, 0.06), v2(0.04, 0.045));
        parts.push(Part::new(thigh, ab.add(b.finish())));
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.ellipsoid(v(0.0, 0.0, 0.01), v(0.045, 0.045, 0.045), 5, 3);
        b.taper(v(0.0, 0.0, 0.0), v(0.0, -hb::SHIN_LEN, 0.0), v2(0.042, 0.045), v2(0.03, 0.032));
        parts.push(Part::new(shin, ab.add(b.finish())));
        let mut b = MeshBuilder::new();
        b.material(Tex::Leather.layer(), rgb(120, 100, 80));
        b.hexa(
            [v(-0.04, -0.05, -0.05), v(0.04, -0.05, -0.05), v(0.04, -0.05, 0.15), v(-0.04, -0.05, 0.15)],
            [v(-0.035, 0.02, -0.04), v(0.035, 0.02, -0.04), v(0.03, -0.02, 0.14), v(-0.03, -0.02, 0.14)],
            Vec2::ONE,
        );
        parts.push(Part::new(foot, ab.add(b.finish())));
    }
    ModelDef { parts, radius: 1.2 }
}

pub fn demon(ab: &mut AssetBuilder) -> ModelDef {
    let mut parts = Vec::new();
    let skin = |b: &mut MeshBuilder| { b.material(Tex::DemonSkin.layer(), WHITE); };

    let mut b = MeshBuilder::new();
    skin(&mut b);
    b.ellipsoid(v(0.0, 0.1, -0.1), v(1.15, 0.8, 0.95), 10, 6);
    parts.push(Part::new(db::PELVIS, ab.add(b.finish())));

    let mut b = MeshBuilder::new();
    skin(&mut b);
    b.ellipsoid(v(0.0, 0.7, 0.2), v(1.45, 1.35, 1.35), 12, 8);
    b.material(Tex::DemonBelly.layer(), WHITE);
    b.ellipsoid(v(0.0, 0.45, 0.65), v(1.15, 1.05, 0.95), 10, 7);
    parts.push(Part::new(db::BELLY, ab.add(b.finish())));

    let mut b = MeshBuilder::new();
    skin(&mut b);
    b.ellipsoid(v(0.0, 0.35, -0.05), v(1.25, 0.85, 0.95), 10, 6);
    for s in [-1.0f32, 1.0] {
        b.ellipsoid(v(s * 1.1, 0.35, 0.05), v(0.6, 0.55, 0.55), 8, 5);
    }
    // Spinal ridges.
    b.material(Tex::Bone.layer(), rgb(170, 160, 140));
    for i in 0..4 {
        let z = -0.85 - i as f32 * 0.04;
        let y = 0.75 - i as f32 * 0.35;
        b.cylinder(v(0.0, y, z), v(0.0, y + 0.25, z - 0.35), 0.12, 0.0, 4, false, false);
    }
    parts.push(Part::new(db::CHEST, ab.add(b.finish())));

    let mut b = MeshBuilder::new();
    skin(&mut b);
    b.ellipsoid(v(0.0, 0.15, 0.25), v(0.6, 0.48, 0.6), 9, 6);
    front_quad(&mut b, v(0.0, 0.18, 0.84), v2(0.42, 0.32), Tex::DemonFace);
    b.material(Tex::Bone.layer(), rgb(200, 190, 160));
    for s in [-1.0f32, 1.0] {
        b.cylinder(v(s * 0.35, 0.45, 0.15), v(s * 0.95, 0.95, -0.35), 0.14, 0.0, 5, false, false);
        b.cylinder(v(s * 0.2, 0.55, 0.45), v(s * 0.3, 0.85, 0.55), 0.06, 0.0, 4, false, false);
    }
    parts.push(Part::new(db::HEAD, ab.add(b.finish())));

    let mut b = MeshBuilder::new();
    skin(&mut b);
    b.ellipsoid(v(0.0, -0.05, 0.25), v(0.48, 0.13, 0.42), 8, 4);
    b.material(Tex::Bone.layer(), rgb(220, 210, 180));
    for i in 0..5 {
        let x = -0.3 + i as f32 * 0.15;
        b.cylinder(v(x, 0.02, 0.55), v(x, 0.2, 0.58), 0.04, 0.0, 3, false, false);
    }
    parts.push(Part::new(db::JAW, ab.add(b.finish())));

    for (upper, fore, hand) in [(db::L_UPPER, db::L_FORE, db::L_HAND), (db::R_UPPER, db::R_FORE, db::R_HAND)] {
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.cylinder(v(0.0, 0.1, 0.0), v(0.0, -db::UPPER_LEN, 0.0), 0.45, 0.32, 8, true, true);
        parts.push(Part::new(upper, ab.add(b.finish())));
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.ellipsoid(v(0.0, 0.0, 0.0), v(0.36, 0.36, 0.36), 7, 4);
        b.cylinder(v(0.0, 0.0, 0.0), v(0.0, -db::FORE_LEN, 0.0), 0.36, 0.28, 8, true, true);
        parts.push(Part::new(fore, ab.add(b.finish())));
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.ellipsoid(v(0.0, -0.22, 0.05), v(0.34, 0.3, 0.34), 7, 5);
        b.material(Tex::Bone.layer(), rgb(60, 55, 50));
        for i in 0..3 {
            let x = -0.15 + i as f32 * 0.15;
            b.cylinder(v(x, -0.4, 0.2), v(x, -0.55, 0.38), 0.06, 0.0, 3, false, false);
        }
        parts.push(Part::new(hand, ab.add(b.finish())));
    }
    for (thigh, shin, foot) in [(db::L_THIGH, db::L_SHIN, db::L_FOOT), (db::R_THIGH, db::R_SHIN, db::R_FOOT)] {
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.cylinder(v(0.0, 0.2, 0.0), v(0.0, -db::THIGH_LEN, 0.0), 0.58, 0.42, 8, true, true);
        parts.push(Part::new(thigh, ab.add(b.finish())));
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.ellipsoid(v(0.0, 0.0, 0.0), v(0.42, 0.42, 0.42), 7, 4);
        b.cylinder(v(0.0, 0.0, 0.0), v(0.0, -db::SHIN_LEN, 0.0), 0.42, 0.3, 8, true, true);
        parts.push(Part::new(shin, ab.add(b.finish())));
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.ellipsoid(v(0.0, -0.05, 0.25), v(0.38, 0.2, 0.55), 7, 4);
        b.material(Tex::Bone.layer(), rgb(60, 55, 50));
        for i in 0..3 {
            let x = -0.18 + i as f32 * 0.18;
            b.cylinder(v(x, -0.1, 0.7), v(x, -0.18, 0.95), 0.07, 0.0, 3, false, false);
        }
        parts.push(Part::new(foot, ab.add(b.finish())));
    }
    for (bone, r0, r1, len) in [(db::TAIL1, 0.42, 0.3, 1.25), (db::TAIL2, 0.3, 0.18, 1.15), (db::TAIL3, 0.18, 0.0, 1.2)] {
        let mut b = MeshBuilder::new();
        skin(&mut b);
        b.cylinder(v(0.0, 0.0, 0.05), v(0.0, 0.0, -len), r0, r1, 7, true, true);
        parts.push(Part::new(bone, ab.add(b.finish())));
    }
    for (bone, side) in [(db::L_WING, 1.0f32), (db::R_WING, -1.0)] {
        let mut b = MeshBuilder::new();
        b.material(Tex::Bone.layer(), rgb(110, 100, 90));
        let root = v(0.0, 0.0, 0.0);
        let elbow = v(side * 1.1, 0.5, -0.1);
        let tips = [v(side * 2.0, 0.9, -0.4), v(side * 2.1, 0.1, -0.5), v(side * 1.6, -0.5, -0.4)];
        b.cylinder(root, elbow, 0.1, 0.07, 4, false, false);
        for t in tips {
            b.cylinder(elbow, t, 0.06, 0.02, 3, false, false);
        }
        // Membrane, both sides.
        b.material(Tex::Feather.layer(), rgb(110, 110, 120));
        let low = v(side * 0.5, -0.4, -0.2);
        let uv = [v2(0.0, 0.0), v2(1.0, 0.0), v2(0.5, 1.0)];
        for tri in [[root, elbow, low], [elbow, tips[0], tips[1]], [elbow, tips[1], tips[2]], [elbow, tips[2], low]] {
            b.tri(tri, uv);
            b.tri([tri[0], tri[2], tri[1]], [uv[0], uv[2], uv[1]]);
        }
        parts.push(Part::new(bone, ab.add(b.finish())));
    }
    ModelDef { parts, radius: 6.0 }
}

/// Model-space offset for an item held in a fist.
pub fn fist_offset() -> Mat4 {
    Mat4::from_translation(hb::FIST)
}
