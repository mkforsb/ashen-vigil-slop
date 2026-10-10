//! Level construction toolkit: rooms with openings, stairs, blocks, lights
//! and environment zones. Produces render chunks and collision.

use crate::assets::textures::Tex;
use ashen_engine::collision::{CollisionWorld, GROUP_STATIC};
use ashen_engine::math::*;
use ashen_engine::mesh::{MeshBuilder, MeshData, Vertex};
use ashen_engine::rng::hash2;
use std::collections::HashMap;

/// Footstep surface types stored in collision triangles.
pub mod surface {
    pub const STONE: u8 = 0;
    pub const WATER: u8 = 1;
    pub const EARTH: u8 = 2;
    pub const WOOD: u8 = 3;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Wall at max Z (faces -Z).
    ZMax,
    ZMin,
    XMax,
    XMin,
}

/// A hole in a room wall; `a..b` is along the wall axis (absolute X for Z
/// walls, absolute Z for X walls), `y0..y1` absolute heights.
#[derive(Clone, Copy, Debug)]
pub struct Opening {
    pub side: Side,
    pub a: f32,
    pub b: f32,
    pub y0: f32,
    pub y1: f32,
}

pub const fn op(side: Side, a: f32, b: f32, y0: f32, y1: f32) -> Opening {
    Opening { side, a, b, y0, y1 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvKind {
    Dungeon,
    Cell,
    Courtyard,
    Arena,
    Outside,
}

#[derive(Clone, Copy, Debug)]
pub struct Zone {
    pub bounds: Aabb,
    pub env: EnvKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKind {
    Torch,
    Pyre,
    Static,
}

#[derive(Clone, Copy, Debug)]
pub struct LevelLight {
    pub pos: Vec3,
    pub color: Vec3,
    pub radius: f32,
    pub intensity: f32,
    pub kind: LightKind,
}

/// Room description.
#[derive(Clone, Debug)]
pub struct Room {
    pub min: Vec2,
    pub max: Vec2,
    pub floor: f32,
    pub ceil: Option<f32>,
    pub wall_top: f32,
    pub wall: Tex,
    pub floor_tex: Tex,
    pub ceil_tex: Tex,
    pub openings: Vec<Opening>,
    pub has_floor: bool,
    pub walls: [bool; 4],
    pub surface: u8,
    pub tint: [u8; 4],
}

impl Room {
    pub fn new(x0: f32, z0: f32, x1: f32, z1: f32, floor: f32, height: f32) -> Self {
        Room {
            min: Vec2::new(x0, z0),
            max: Vec2::new(x1, z1),
            floor,
            ceil: Some(floor + height),
            wall_top: floor + height,
            wall: Tex::StoneBrick,
            floor_tex: Tex::Flagstone,
            ceil_tex: Tex::Plaster,
            openings: Vec::new(),
            has_floor: true,
            walls: [true; 4],
            surface: surface::STONE,
            tint: [255; 4],
        }
    }

    pub fn open_sky(mut self, wall_top: f32) -> Self {
        self.ceil = None;
        self.wall_top = wall_top;
        self
    }

    pub fn tex(mut self, wall: Tex, floor: Tex, ceil: Tex) -> Self {
        self.wall = wall;
        self.floor_tex = floor;
        self.ceil_tex = ceil;
        self
    }

    pub fn holes(mut self, o: &[Opening]) -> Self {
        self.openings.extend_from_slice(o);
        self
    }

    pub fn no_floor(mut self) -> Self {
        self.has_floor = false;
        self
    }

    pub fn tint(mut self, t: [u8; 4]) -> Self {
        self.tint = t;
        self
    }

    pub fn surface(mut self, s: u8) -> Self {
        self.surface = s;
        self
    }

    /// Disable walls: [ZMax, ZMin, XMax, XMin].
    pub fn walls(mut self, w: [bool; 4]) -> Self {
        self.walls = w;
        self
    }
}

pub struct LevelBuilder {
    pub mesh: MeshBuilder,
    pub col: CollisionWorld,
    pub lights: Vec<LevelLight>,
    pub zones: Vec<Zone>,
    next_group: u16,
    /// Metres per texture repeat.
    pub tile: f32,
}

/// Finished level geometry ready for the world.
pub struct LevelGeometry {
    pub chunks: Vec<(MeshData, Aabb)>,
    pub col: CollisionWorld,
    pub lights: Vec<LevelLight>,
    pub zones: Vec<Zone>,
}

fn vary(p: Vec3, base: [u8; 4], floor: f32) -> [u8; 4] {
    // Per-vertex grime and fake ambient occlusion near the floor.
    let h = hash2((p.x * 1.7) as i32, (p.z * 1.7 + p.y * 3.1) as i32, 7);
    let ao = 0.62 + 0.38 * smoothstep(0.0, 1.6, p.y - floor);
    let k = ao * (0.86 + h * 0.2);
    [
        (base[0] as f32 * k).min(255.0) as u8,
        (base[1] as f32 * k).min(255.0) as u8,
        (base[2] as f32 * k).min(255.0) as u8,
        255,
    ]
}

impl Default for LevelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LevelBuilder {
    pub fn new() -> Self {
        LevelBuilder { mesh: MeshBuilder::new(), col: CollisionWorld::new(2.0), lights: Vec::new(), zones: Vec::new(), next_group: 1, tile: 1.3 }
    }

    /// Allocates a collision group for a dynamic obstacle (door, gate...).
    pub fn new_group(&mut self) -> u16 {
        let g = self.next_group;
        self.next_group += 1;
        g
    }

    /// Subdivided planar quad: `o + u*su + v*sv`, front face along `su x sv`.
    #[allow(clippy::too_many_arguments)]
    pub fn surface(&mut self, o: Vec3, su: Vec3, sv: Vec3, tex: Tex, tint: [u8; 4], floor: f32, collide: Option<(u16, u8)>) {
        let lu = su.length();
        let lv = sv.length();
        if lu < 1e-3 || lv < 1e-3 {
            return;
        }
        let nu = (lu / 1.1).ceil().max(1.0) as u32;
        let nv = (lv / 1.1).ceil().max(1.0) as u32;
        let n = su.cross(sv).normalize();
        // Planar world-space UVs so textures line up across surfaces.
        let (ax_u, ax_v) = if n.y.abs() > 0.7 {
            (Vec3::X, Vec3::Z)
        } else {
            let horiz = Vec3::new(-n.z, 0.0, n.x).normalize();
            (horiz, Vec3::NEG_Y)
        };
        let tile = self.tile;
        self.mesh.layer = tex.layer();
        self.mesh.flags = 0;
        self.mesh.grid_quad(o, su, sv, nu, nv, &|p| Vec2::new(p.dot(ax_u), p.dot(ax_v)) / tile, &|p| vary(p, tint, floor));
        if let Some((group, surf)) = collide {
            self.col.add_quad([o, o + su, o + su + sv, o + sv], group, surf);
        }
    }

    /// Vertical wall from `a` to `b` (XZ), facing `(b - a) x Y`, with holes.
    #[allow(clippy::too_many_arguments)]
    pub fn wall(&mut self, a: Vec2, b: Vec2, y0: f32, y1: f32, tex: Tex, tint: [u8; 4], holes: &[(f32, f32, f32, f32)], floor: f32) {
        let len = (b - a).length();
        if len < 1e-3 || y1 <= y0 {
            return;
        }
        let dir = (b - a) / len;
        // Collect breakpoints along the wall.
        let mut cuts = vec![0.0, len];
        for &(h0, h1, _, _) in holes {
            cuts.push(h0.clamp(0.0, len));
            cuts.push(h1.clamp(0.0, len));
        }
        cuts.sort_by(|x, y| x.partial_cmp(y).unwrap());
        cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-4);
        for w in cuts.windows(2) {
            let (s0, s1) = (w[0], w[1]);
            if s1 - s0 < 1e-4 {
                continue;
            }
            let mid = (s0 + s1) * 0.5;
            // Vertical intervals that are solid in this span.
            let mut solid = vec![(y0, y1)];
            for &(h0, h1, hy0, hy1) in holes {
                if mid > h0 && mid < h1 {
                    let mut next = Vec::new();
                    for (s, e) in solid {
                        if hy1 <= s || hy0 >= e {
                            next.push((s, e));
                        } else {
                            if hy0 > s {
                                next.push((s, hy0));
                            }
                            if hy1 < e {
                                next.push((hy1, e));
                            }
                        }
                    }
                    solid = next;
                }
            }
            let p0 = a + dir * s0;
            let p1 = a + dir * s1;
            for (s, e) in solid {
                if e - s < 1e-3 {
                    continue;
                }
                let o = Vec3::new(p0.x, s, p0.y);
                let su = Vec3::new(p1.x - p0.x, 0.0, p1.y - p0.y);
                let sv = Vec3::new(0.0, e - s, 0.0);
                self.surface(o, su, sv, tex, tint, floor, Some((GROUP_STATIC, surface::STONE)));
            }
        }
    }

    pub fn floor_rect(&mut self, min: Vec2, max: Vec2, y: f32, tex: Tex, tint: [u8; 4], surf: u8) {
        let o = Vec3::new(min.x, y, min.y);
        // su x sv must point up: su = +Z, sv = +X.
        self.surface(o, Vec3::new(0.0, 0.0, max.y - min.y), Vec3::new(max.x - min.x, 0.0, 0.0), tex, tint, y, Some((GROUP_STATIC, surf)));
    }

    pub fn ceiling_rect(&mut self, min: Vec2, max: Vec2, y: f32, tex: Tex, tint: [u8; 4]) {
        let o = Vec3::new(min.x, y, min.y);
        self.surface(o, Vec3::new(max.x - min.x, 0.0, 0.0), Vec3::new(0.0, 0.0, max.y - min.y), tex, tint, y - 3.0, Some((GROUP_STATIC, surface::STONE)));
    }

    pub fn room(&mut self, r: &Room) {
        let (x0, z0, x1, z1) = (r.min.x, r.min.y, r.max.x, r.max.y);
        if r.has_floor {
            self.floor_rect(r.min, r.max, r.floor, r.floor_tex, r.tint, r.surface);
        }
        if let Some(c) = r.ceil {
            self.ceiling_rect(r.min, r.max, c, r.ceil_tex, r.tint);
        }
        let holes = |side: Side, origin: f32, sign: f32| -> Vec<(f32, f32, f32, f32)> {
            r.openings
                .iter()
                .filter(|o| o.side == side)
                .map(|o| {
                    let (a, b) = ((o.a - origin) * sign, (o.b - origin) * sign);
                    (a.min(b), a.max(b), o.y0, o.y1)
                })
                .collect()
        };
        let top = r.wall_top;
        if r.walls[1] {
            self.wall(Vec2::new(x0, z0), Vec2::new(x1, z0), r.floor, top, r.wall, r.tint, &holes(Side::ZMin, x0, 1.0), r.floor);
        }
        if r.walls[0] {
            self.wall(Vec2::new(x1, z1), Vec2::new(x0, z1), r.floor, top, r.wall, r.tint, &holes(Side::ZMax, x1, -1.0), r.floor);
        }
        if r.walls[3] {
            self.wall(Vec2::new(x0, z1), Vec2::new(x0, z0), r.floor, top, r.wall, r.tint, &holes(Side::XMin, z1, -1.0), r.floor);
        }
        if r.walls[2] {
            self.wall(Vec2::new(x1, z0), Vec2::new(x1, z1), r.floor, top, r.wall, r.tint, &holes(Side::XMax, z0, 1.0), r.floor);
        }
    }

    /// Solid box with all faces rendered and collidable.
    pub fn block(&mut self, min: Vec3, max: Vec3, tex: Tex, tint: [u8; 4]) {
        let s = max - min;
        let f = min.y;
        // Top, bottom.
        self.surface(Vec3::new(min.x, max.y, min.z), Vec3::new(0.0, 0.0, s.z), Vec3::new(s.x, 0.0, 0.0), tex, tint, f, None);
        self.surface(Vec3::new(min.x, min.y, min.z), Vec3::new(s.x, 0.0, 0.0), Vec3::new(0.0, 0.0, s.z), tex, tint, f, None);
        // Sides (outward).
        self.surface(Vec3::new(min.x, min.y, min.z), Vec3::new(0.0, 0.0, s.z), Vec3::new(0.0, s.y, 0.0), tex, tint, f, None);
        self.surface(Vec3::new(max.x, min.y, max.z), Vec3::new(0.0, 0.0, -s.z), Vec3::new(0.0, s.y, 0.0), tex, tint, f, None);
        self.surface(Vec3::new(max.x, min.y, min.z), Vec3::new(-s.x, 0.0, 0.0), Vec3::new(0.0, s.y, 0.0), tex, tint, f, None);
        self.surface(Vec3::new(min.x, min.y, max.z), Vec3::new(s.x, 0.0, 0.0), Vec3::new(0.0, s.y, 0.0), tex, tint, f, None);
        self.col.add_box(min, max, GROUP_STATIC, surface::STONE);
    }

    /// Invisible collision box.
    pub fn blocker(&mut self, min: Vec3, max: Vec3) {
        self.col.add_box(min, max, GROUP_STATIC, surface::STONE);
    }

    /// Straight staircase. The run goes along Z if `along_z`, rising from
    /// `y0` at coordinate `from` to `y1` at coordinate `to` (either direction).
    /// `lo..hi` is the extent across the stairs.
    #[allow(clippy::too_many_arguments)]
    pub fn stairs(&mut self, along_z: bool, from: f32, to: f32, lo: f32, hi: f32, y0: f32, y1: f32, tex: Tex, tint: [u8; 4]) {
        let rise = y1 - y0;
        let steps = ((rise.abs() / 0.24).round() as i32).max(1);
        let run = to - from;
        let p = |along: f32, across: f32, y: f32| if along_z { Vec3::new(across, y, along) } else { Vec3::new(along, y, across) };
        let step_run = run / steps as f32;
        let step_rise = rise / steps as f32;
        self.mesh.layer = Tex::Stairs.layer();
        for i in 0..steps {
            let a0 = from + step_run * i as f32;
            let a1 = a0 + step_run;
            let ytop = y0 + step_rise * (i + 1) as f32;
            let ybot = y0 + step_rise * i as f32;
            // Tread.
            let tread = [p(a0, lo, ytop), p(a0, hi, ytop), p(a1, hi, ytop), p(a1, lo, ytop)];
            let c = vary(tread[0], tint, ytop - 1.0);
            self.quad_up(tread, c, Tex::Stairs);
            // Riser facing back towards the bottom.
            let riser = [p(a0, lo, ybot), p(a0, hi, ybot), p(a0, hi, ytop), p(a0, lo, ytop)];
            self.quad_facing(riser, -Vec3::from(if along_z { Vec3::new(0.0, 0.0, run.signum()) } else { Vec3::new(run.signum(), 0.0, 0.0) }), c, tex);
            // Side stringers down to the base.
            let base = y0.min(y1) - 0.01;
            for across in [lo, hi] {
                let s = [p(a0, across, base), p(a1, across, base), p(a1, across, ytop), p(a0, across, ytop)];
                let outward = if along_z { Vec3::new((across - (lo + hi) * 0.5).signum(), 0.0, 0.0) } else { Vec3::new(0.0, 0.0, (across - (lo + hi) * 0.5).signum()) };
                self.quad_facing(s, outward, c, tex);
            }
        }
        // Collision ramp (slightly below the step nosings).
        let mut ramp = [p(from, lo, y0), p(from, hi, y0), p(to, hi, y1), p(to, lo, y1)];
        if (ramp[1] - ramp[0]).cross(ramp[2] - ramp[0]).y < 0.0 {
            ramp.swap(1, 3);
        }
        self.col.add_quad(ramp, GROUP_STATIC, surface::STONE);
    }

    fn quad_up(&mut self, mut q: [Vec3; 4], c: [u8; 4], tex: Tex) {
        let n = (q[1] - q[0]).cross(q[2] - q[0]);
        if n.y < 0.0 {
            q.swap(1, 3);
        }
        self.raw_quad(q, c, tex);
    }

    fn quad_facing(&mut self, mut q: [Vec3; 4], dir: Vec3, c: [u8; 4], tex: Tex) {
        let n = (q[1] - q[0]).cross(q[2] - q[0]);
        if n.dot(dir) < 0.0 {
            q.swap(1, 3);
        }
        self.raw_quad(q, c, tex);
    }

    fn raw_quad(&mut self, q: [Vec3; 4], c: [u8; 4], tex: Tex) {
        let n = (q[1] - q[0]).cross(q[2] - q[0]).normalize_or(Vec3::Y);
        let (au, av) = if n.y.abs() > 0.7 { (Vec3::X, Vec3::Z) } else { (Vec3::new(-n.z, 0.0, n.x).normalize_or(Vec3::X), Vec3::NEG_Y) };
        let uv = |p: Vec3| Vec2::new(p.dot(au), p.dot(av)) / self.tile;
        self.mesh.layer = tex.layer();
        self.mesh.set_color(c);
        self.mesh.quad(q, [uv(q[0]), uv(q[1]), uv(q[2]), uv(q[3])]);
    }

    pub fn light(&mut self, pos: Vec3, color: Vec3, radius: f32, intensity: f32, kind: LightKind) {
        self.lights.push(LevelLight { pos, color, radius, intensity, kind });
    }

    pub fn zone(&mut self, min: Vec3, max: Vec3, env: EnvKind) {
        self.zones.push(Zone { bounds: Aabb::new(min, max), env });
    }

    /// Splits geometry into chunks for culling and light selection.
    pub fn finish(self, chunk: f32) -> LevelGeometry {
        let mesh = self.mesh.finish();
        let mut buckets: HashMap<(i32, i32, i32), (Vec<Vertex>, Vec<u32>, HashMap<u32, u32>)> = HashMap::new();
        for t in mesh.indices.chunks_exact(3) {
            let c = (mesh.vertices[t[0] as usize].position() + mesh.vertices[t[1] as usize].position() + mesh.vertices[t[2] as usize].position()) / 3.0;
            let key = ((c.x / chunk).floor() as i32, (c.y / (chunk * 0.75)).floor() as i32, (c.z / chunk).floor() as i32);
            let (verts, idx, remap) = buckets.entry(key).or_default();
            for &i in t {
                let ni = *remap.entry(i).or_insert_with(|| {
                    verts.push(mesh.vertices[i as usize]);
                    verts.len() as u32 - 1
                });
                idx.push(ni);
            }
        }
        let mut keys: Vec<_> = buckets.keys().copied().collect();
        keys.sort();
        let chunks = keys
            .into_iter()
            .map(|k| {
                let (vertices, indices, _) = buckets.remove(&k).unwrap();
                let m = MeshData { vertices, indices };
                let b = m.bounds();
                (m, b)
            })
            .collect();
        LevelGeometry { chunks, col: self.col, lights: self.lights, zones: self.zones }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ashen_engine::collision::CharacterShape;

    #[test]
    fn room_walls_face_inward_and_collide() {
        let mut b = LevelBuilder::new();
        b.room(&Room::new(0.0, 0.0, 6.0, 4.0, 0.0, 3.0).holes(&[op(Side::ZMax, 2.0, 4.0, 0.0, 2.2)]));
        let center = Vec3::new(3.0, 1.5, 2.0);
        let mesh = b.mesh.mesh.clone();
        for [a, bb, c] in mesh.triangles() {
            let n = (bb - a).cross(c - a);
            if n.length_squared() < 1e-9 {
                continue;
            }
            assert!(n.dot(center - (a + bb + c) / 3.0) > 0.0, "faces inward");
        }
        let geo = b.finish(8.0);
        let shape = CharacterShape { radius: 0.35, height: 1.8, step: 0.4 };
        // Walk into the solid part of the +Z wall: blocked.
        let mut p = Vec3::new(1.0, 0.0, 2.0);
        for _ in 0..60 {
            p = geo.col.move_character(p, Vec3::new(0.0, -0.01, 0.1), shape, true).pos;
        }
        assert!(p.z < 4.0 - 0.3, "{p:?}");
        // Walk through the doorway: passes.
        let mut p = Vec3::new(3.0, 0.0, 2.0);
        for _ in 0..60 {
            p = geo.col.move_character(p, Vec3::new(0.0, 0.0, 0.1), shape, true).pos;
        }
        assert!(p.z > 5.0, "{p:?}");
    }

    #[test]
    fn stairs_walkable() {
        let mut b = LevelBuilder::new();
        b.floor_rect(Vec2::new(-2.0, -4.0), Vec2::new(2.0, 0.0), 0.0, Tex::Flagstone, [255; 4], 0);
        b.stairs(true, 0.0, 6.0, -1.5, 1.5, 0.0, 3.0, Tex::StoneBrick, [255; 4]);
        b.floor_rect(Vec2::new(-2.0, 6.0), Vec2::new(2.0, 10.0), 3.0, Tex::Flagstone, [255; 4], 0);
        let geo = b.finish(8.0);
        let shape = CharacterShape { radius: 0.35, height: 1.8, step: 0.45 };
        let mut p = Vec3::new(0.0, 0.0, -2.0);
        let mut g = true;
        for _ in 0..200 {
            let r = geo.col.move_character(p, Vec3::new(0.0, -0.05, 0.06), shape, g);
            p = r.pos;
            g = r.grounded;
        }
        assert!(g && (p.y - 3.0).abs() < 0.05 && p.z > 7.0, "{p:?}");
        // And back down without falling.
        for _ in 0..200 {
            let r = geo.col.move_character(p, Vec3::new(0.0, -0.05, -0.06), shape, g);
            p = r.pos;
            g = r.grounded;
            assert!(g, "stays grounded on the way down at {p:?}");
        }
    }
}
