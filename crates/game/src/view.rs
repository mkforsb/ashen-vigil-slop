//! Builds the render list for a world.

use crate::actor::{Action, Actor};
use crate::assets::Assets;
use crate::assets::textures::Tex;
use crate::level::{EnvKind, LightKind};
use crate::props::PropKind;
use crate::rig::{demon as db, humanoid as hb};
use crate::world::{BOULDER_R, BoulderState, World};
use ashen_engine::math::*;
use ashen_engine::render::{Blend, Camera, DrawItem, DynQuad, PointLight, RenderList};
use ashen_engine::rng::value_noise;

/// Camera override (title screen).
#[derive(Clone, Copy, Debug)]
pub struct FixedCam {
    pub eye: Vec3,
    pub target: Vec3,
}

pub fn build(w: &mut World, assets: &Assets, aspect: f32, fixed: Option<FixedCam>, list: &mut RenderList) {
    list.clear();
    let (view, cam_pos) = match fixed {
        Some(f) => (look_at(f.eye, f.target, Vec3::Y), f.eye),
        None => (w.camera.view(), w.camera.pos),
    };
    let proj = perspective(w.camera.fov, aspect, 0.08, 160.0);
    list.camera = Some(Camera { view, proj, pos: cam_pos });
    let frustum = Frustum::from_matrix(&(proj * view));
    list.env = w.env;
    let far = w.env.fog_far + 8.0;
    let right = view.row(0).truncate();
    let up = view.row(1).truncate();
    let t = w.time;

    // Lights.
    for (i, l) in w.lights.iter().enumerate() {
        if (l.pos - cam_pos).length() > far + l.radius {
            continue;
        }
        let mut intensity = l.intensity;
        match l.kind {
            LightKind::Torch => {
                intensity *= 0.82 + 0.3 * value_noise(t * 7.0 + i as f32 * 13.0, i as f32, 0, 3);
            }
            LightKind::Pyre => {
                let lit = w.props.iter().any(|p| matches!(p.kind, PropKind::Pyre { lit: true, .. }) && (p.pos - l.pos).length() < 2.0);
                intensity = if lit { 2.2 * (0.85 + 0.25 * value_noise(t * 9.0, i as f32, 0, 5)) } else { 0.25 };
            }
            LightKind::Static => {}
        }
        list.lights.push(PointLight { pos: l.pos, radius: l.radius, color: l.color, intensity });
    }
    // A faint glow around the player keeps dark corridors readable.
    let ppos = w.player().pos;
    let dark = matches!(w.env_kind, EnvKind::Dungeon | EnvKind::Cell);
    list.lights.push(PointLight { pos: ppos + Vec3::new(0.0, 2.2, 0.0), radius: 5.5, color: Vec3::new(1.0, 0.85, 0.7), intensity: if dark { 0.35 } else { 0.1 } });

    // Sky.
    let outdoor = !dark;
    if outdoor {
        let mut sky = DrawItem::new(assets.props.sky, Mat4::from_translation(cam_pos), cam_pos, 100.0);
        sky.uv_offset = Vec2::new(t * 0.004, t * 0.002);
        list.sky.push(sky);
    }

    // Level.
    for (mesh, b) in &w.chunks {
        if b.distance_sq(cam_pos) > far * far || !frustum.aabb_visible(b) {
            continue;
        }
        let c = b.center();
        list.draws.push(DrawItem::new(*mesh, Mat4::IDENTITY, c, b.half().length()));
    }

    // Props.
    for p in &w.props {
        let dist2 = (p.pos - cam_pos).length_squared();
        if dist2 > far * far {
            continue;
        }
        match &p.kind {
            PropKind::Decor { mesh, model, radius } => {
                if frustum.sphere_visible(p.pos, *radius) {
                    list.draws.push(DrawItem::new(*mesh, *model, p.pos, *radius));
                }
            }
            PropKind::Pyre { .. } => list.draws.push(DrawItem::new(assets.props.pyre, Mat4::from_translation(p.pos), p.pos, 1.5)),
            PropKind::Door(d) => {
                for leaf in &d.leaves {
                    let yaw = leaf.closed_yaw + leaf.swing * ease_in_out(d.open);
                    let m = Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), leaf.hinge);
                    list.draws.push(DrawItem::new(leaf.mesh, m, leaf.hinge, 6.0));
                }
            }
            PropKind::Gate(g) => {
                let m = Mat4::from_rotation_translation(Quat::from_rotation_y(g.yaw), g.base + Vec3::Y * g.lift);
                let mut d = DrawItem::new(g.mesh, m, g.base, 4.0);
                d.double_sided = true;
                list.draws.push(d);
            }
            PropKind::Fog(f) => {
                if f.active {
                    let m = Mat4::from_rotation_translation(Quat::from_rotation_y(f.yaw), p.pos);
                    let mut d = DrawItem::new(f.mesh, m, p.pos, 4.0);
                    d.blend = Blend::Additive;
                    d.double_sided = true;
                    d.tint = Vec4::new(1.0, 1.0, 1.0, 0.75 + 0.15 * (t * 1.3).sin());
                    d.uv_offset = Vec2::new(t * 0.05, t * 0.11);
                    list.draws.push(d);
                }
            }
            PropKind::Message { .. } => {
                let mut d = DrawItem::new(assets.props.rune, Mat4::from_rotation_translation(Quat::from_rotation_y(t * 0.2), p.pos), p.pos, 1.0);
                d.blend = Blend::Additive;
                d.tint = Vec4::new(1.0, 1.0, 1.0, 0.6 + 0.2 * (t * 2.0).sin());
                list.draws.push(d);
            }
            PropKind::Urn { mesh, broken, .. } => {
                if !broken && frustum.sphere_visible(p.pos, 1.0) {
                    list.draws.push(DrawItem::new(*mesh, Mat4::from_rotation_translation(Quat::from_rotation_y(p.yaw), p.pos), p.pos, 1.0));
                }
            }
            PropKind::Breakable { mesh, broken, .. } => {
                if !broken {
                    list.draws.push(DrawItem::new(*mesh, Mat4::IDENTITY, p.pos, 3.0));
                }
            }
            PropKind::Water { mesh } => {
                let mut d = DrawItem::new(*mesh, Mat4::IDENTITY, p.pos, 8.0);
                d.uv_offset = Vec2::new(t * 0.03, t * 0.017);
                list.draws.push(d);
            }
            PropKind::Item { .. } | PropKind::Torch => {}
        }
    }
    if matches!(w.boulder.state, BoulderState::Idle | BoulderState::Arming(_) | BoulderState::Rolling) {
        let m = Mat4::from_translation(w.boulder.pos) * Mat4::from_rotation_x(-w.boulder.roll) * Mat4::from_scale(Vec3::splat(BOULDER_R));
        list.draws.push(DrawItem::new(assets.props.boulder, m, w.boulder.pos, BOULDER_R));
    }
    for pr in &w.projectiles {
        let dir = pr.vel.normalize_or(Vec3::Z);
        let m = Mat4::from_rotation_translation(look_rotation(dir, Vec3::Y), pr.pos);
        list.draws.push(DrawItem::new(assets.items.arrow, m, pr.pos, 0.5));
    }

    // Actors.
    for a in &w.actors {
        if a.remove {
            continue;
        }
        let model = a.body.model(assets);
        let c = a.center();
        if (c - cam_pos).length() > far + model.radius || !frustum.sphere_visible(c, model.radius) {
            continue;
        }
        let mut fade = 0.0;
        if a.id == 0 && fixed.is_none() {
            let d = (w.camera.pos - (a.pos + Vec3::Y * 1.2)).length();
            fade = smoothstep(1.3, 0.6, d) * 0.85;
        }
        draw_actor(a, assets, list, fade);
    }

    // Dynamic quads: shadows, bloodstain, lock-on, particles.
    for a in &w.actors {
        if a.remove || (a.dead && a.dissolve > 0.5) {
            continue;
        }
        let r = if a.body.is_demon() { 2.6 } else { 0.5 };
        let ground = if a.grounded {
            Some(a.pos.y)
        } else {
            w.col.raycast(a.pos + Vec3::Y * 0.2, Vec3::NEG_Y, 30.0).map(|h| h.point.y)
        };
        if let Some(gy) = ground {
            let h = (a.pos.y - gy).max(0.0);
            let k = (1.0 - h / 8.0).clamp(0.2, 1.0);
            let c = Vec3::new(a.pos.x, gy + 0.03, a.pos.z);
            flat_quad(list, c, r * (0.6 + 0.4 * k), [0, 0, 0, (120.0 * k) as u8], Tex::Shadow, Blend::Alpha);
        }
    }
    if let Some((pos, _)) = w.progress.bloodstain {
        let p = Vec3::from(pos) + Vec3::Y * 0.5;
        let s = 0.25 + 0.05 * (t * 4.0).sin();
        billboard(list, p, s, right, up, [90, 255, 140, 220], Tex::Glow, Blend::Additive);
        billboard(list, p, s * 2.4, right, up, [40, 140, 70, 120], Tex::Glow, Blend::Additive);
    }
    if let Some(lp) = w.lock_target_pos() {
        billboard(list, lp, 0.09, right, up, [255, 240, 220, 255], Tex::Glow, Blend::Additive);
        billboard(list, lp, 0.22, right, up, [255, 120, 80, 140], Tex::Glow, Blend::Additive);
    }
    w.fx.emit_quads(right, up, &mut list.quads);
    list.fade = Vec4::new(0.0, 0.0, 0.0, w.fade);
}

fn draw_actor(a: &Actor, assets: &Assets, list: &mut RenderList, fade: f32) {
    let model = a.body.model(assets);
    let world = a.world();
    let c = a.center();
    let flash = Vec4::new(1.0, 0.92, 0.85, a.flash * 0.55);
    let push = |list: &mut RenderList, mesh, m: Mat4, r: f32| {
        let mut d = DrawItem::new(mesh, m, c, r);
        d.flash = flash;
        d.dissolve = a.dissolve.max(fade);
        list.draws.push(d);
    };
    for p in &model.parts {
        push(list, p.mesh, world * a.mats[p.bone] * p.offset, model.radius);
    }
    let (rh, lh) = if a.body.is_demon() { (db::R_HAND, db::L_HAND) } else { (hb::R_HAND, hb::L_HAND) };
    if let Some(m) = a.right_mesh {
        push(list, m, world * a.mats[rh], model.radius);
    }
    if let Some(m) = a.left_mesh {
        push(list, m, world * a.mats[lh], model.radius);
    }
    if matches!(a.action, Action::Drink { .. }) && a.action_tn() > 0.1 && a.action_tn() < 0.85 {
        // The flask sits in the fist next to the sword grip.
        let m = world * a.mats[hb::R_HAND] * Mat4::from_translation(Vec3::new(0.04, 0.0, 0.04));
        push(list, assets.items.flask, m, 1.0);
    }
}

fn flat_quad(list: &mut RenderList, c: Vec3, r: f32, color: [u8; 4], tex: Tex, blend: Blend) {
    list.quads.push(DynQuad {
        p: [c + Vec3::new(-r, 0.0, -r), c + Vec3::new(-r, 0.0, r), c + Vec3::new(r, 0.0, r), c + Vec3::new(r, 0.0, -r)],
        uv: [Vec2::new(0.0, 0.0), Vec2::new(0.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 0.0)],
        color,
        layer: tex.layer(),
        blend,
    });
}

#[allow(clippy::too_many_arguments)]
fn billboard(list: &mut RenderList, c: Vec3, s: f32, right: Vec3, up: Vec3, color: [u8; 4], tex: Tex, blend: Blend) {
    let r = right * s;
    let u = up * s;
    list.quads.push(DynQuad {
        p: [c - r - u, c + r - u, c + r + u, c - r + u],
        uv: [Vec2::new(0.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 0.0)],
        color,
        layer: tex.layer(),
        blend,
    });
}
