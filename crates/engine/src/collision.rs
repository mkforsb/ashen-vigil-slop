//! Static triangle collision with a uniform XZ grid, plus a simple kinematic
//! character mover (vertical capsule approximated by stacked spheres and
//! ground rays).
//!
//! Triangles belong to a *group*; groups can be switched on and off at run
//! time, which is how doors, gates and destructible walls are handled.

use crate::math::{Aabb, Vec3};
use std::collections::HashMap;

/// Minimum normal Y for a surface to count as floor.
pub const WALKABLE_NY: f32 = 0.6;

/// Group 0 is always-on static geometry.
pub const GROUP_STATIC: u16 = 0;

#[derive(Clone, Copy, Debug)]
pub struct ColTri {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
    pub n: Vec3,
    pub group: u16,
    /// Game-defined surface type (footstep sounds etc).
    pub surface: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub surface: u8,
    pub group: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub point: Vec3,
    pub normal: Vec3,
    pub depth: f32,
    pub tri_normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct CharacterShape {
    pub radius: f32,
    pub height: f32,
    /// Maximum ledge height climbed without jumping.
    pub step: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MoveResult {
    pub pos: Vec3,
    pub grounded: bool,
    pub ground_normal: Vec3,
    pub ground_surface: u8,
    pub hit_wall: bool,
    pub wall_normal: Vec3,
    pub hit_ceiling: bool,
}

#[derive(Clone, Debug)]
pub struct CollisionWorld {
    tris: Vec<ColTri>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    cell: f32,
    groups: Vec<bool>,
}

impl Default for CollisionWorld {
    fn default() -> Self {
        Self::new(2.0)
    }
}

impl CollisionWorld {
    pub fn new(cell: f32) -> Self {
        CollisionWorld { tris: Vec::new(), grid: HashMap::new(), cell, groups: vec![true] }
    }

    pub fn tri_count(&self) -> usize {
        self.tris.len()
    }

    pub fn add_tri(&mut self, a: Vec3, b: Vec3, c: Vec3, group: u16, surface: u8) {
        let n = (b - a).cross(c - a);
        if n.length_squared() < 1e-12 {
            return;
        }
        let n = n.normalize();
        let idx = self.tris.len() as u32;
        self.tris.push(ColTri { a, b, c, n, group, surface });
        while self.groups.len() <= group as usize {
            self.groups.push(true);
        }
        let mut bb = Aabb::EMPTY;
        bb.grow(a);
        bb.grow(b);
        bb.grow(c);
        let (x0, z0, x1, z1) = self.cell_range(&bb);
        for cz in z0..=z1 {
            for cx in x0..=x1 {
                self.grid.entry((cx, cz)).or_default().push(idx);
            }
        }
    }

    /// Adds a quad (two triangles), counter-clockwise seen from the solid side's outside.
    pub fn add_quad(&mut self, p: [Vec3; 4], group: u16, surface: u8) {
        self.add_tri(p[0], p[1], p[2], group, surface);
        self.add_tri(p[0], p[2], p[3], group, surface);
    }

    /// Adds all faces of an axis-aligned box.
    pub fn add_box(&mut self, min: Vec3, max: Vec3, group: u16, surface: u8) {
        let c = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        let (a, b) = (min, max);
        let faces = [
            [c(b.x, a.y, a.z), c(b.x, b.y, a.z), c(b.x, b.y, b.z), c(b.x, a.y, b.z)], // +x
            [c(a.x, a.y, a.z), c(a.x, a.y, b.z), c(a.x, b.y, b.z), c(a.x, b.y, a.z)], // -x
            [c(a.x, b.y, a.z), c(a.x, b.y, b.z), c(b.x, b.y, b.z), c(b.x, b.y, a.z)], // +y
            [c(a.x, a.y, a.z), c(b.x, a.y, a.z), c(b.x, a.y, b.z), c(a.x, a.y, b.z)], // -y
            [c(a.x, a.y, b.z), c(b.x, a.y, b.z), c(b.x, b.y, b.z), c(a.x, b.y, b.z)], // +z
            [c(a.x, a.y, a.z), c(a.x, b.y, a.z), c(b.x, b.y, a.z), c(b.x, a.y, a.z)], // -z
        ];
        for f in faces {
            self.add_quad(f, group, surface);
        }
    }

    pub fn set_group_enabled(&mut self, group: u16, on: bool) {
        while self.groups.len() <= group as usize {
            self.groups.push(true);
        }
        self.groups[group as usize] = on;
    }

    pub fn group_enabled(&self, group: u16) -> bool {
        self.groups.get(group as usize).copied().unwrap_or(true)
    }

    fn cell_range(&self, b: &Aabb) -> (i32, i32, i32, i32) {
        (
            (b.min.x / self.cell).floor() as i32,
            (b.min.z / self.cell).floor() as i32,
            (b.max.x / self.cell).floor() as i32,
            (b.max.z / self.cell).floor() as i32,
        )
    }

    /// Indices of enabled triangles whose cells overlap the box.
    fn candidates(&self, b: &Aabb, out: &mut Vec<u32>) {
        out.clear();
        let (x0, z0, x1, z1) = self.cell_range(b);
        for cz in z0..=z1 {
            for cx in x0..=x1 {
                if let Some(v) = self.grid.get(&(cx, cz)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out.retain(|&i| self.groups[self.tris[i as usize].group as usize]);
    }

    /// Nearest hit along `dir` (need not be normalised; `t` is in units of `dir`).
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<RayHit> {
        self.raycast_filtered(origin, dir, max_t, &|_| true)
    }

    pub fn raycast_filtered(&self, origin: Vec3, dir: Vec3, max_t: f32, filter: &dyn Fn(&ColTri) -> bool) -> Option<RayHit> {
        let end = origin + dir * max_t;
        let bb = Aabb::new(origin, end).expand(0.01);
        let mut cand = Vec::new();
        self.candidates(&bb, &mut cand);
        let mut best: Option<RayHit> = None;
        for &i in &cand {
            let tri = &self.tris[i as usize];
            if !filter(tri) {
                continue;
            }
            if let Some(t) = ray_triangle(origin, dir, tri.a, tri.b, tri.c) {
                if t <= max_t && best.is_none_or(|b| t < b.t) {
                    best = Some(RayHit { t, point: origin + dir * t, normal: tri.n, surface: tri.surface, group: tri.group });
                }
            }
        }
        best
    }

    /// True if the segment is unobstructed.
    pub fn line_of_sight(&self, a: Vec3, b: Vec3) -> bool {
        self.raycast(a, b - a, 1.0).is_none()
    }

    /// All triangle contacts for a sphere.
    pub fn sphere_contacts(&self, center: Vec3, r: f32, out: &mut Vec<Contact>) {
        out.clear();
        let bb = Aabb::from_center(center, Vec3::splat(r));
        let mut cand = Vec::new();
        self.candidates(&bb, &mut cand);
        for &i in &cand {
            let tri = &self.tris[i as usize];
            let p = closest_point_triangle(center, tri.a, tri.b, tri.c);
            let d = center - p;
            let dist2 = d.length_squared();
            if dist2 < r * r {
                let dist = dist2.sqrt();
                let normal = if dist > 1e-6 { d / dist } else { tri.n };
                out.push(Contact { point: p, normal, depth: r - dist, tri_normal: tri.n });
            }
        }
    }

    /// Walkable ground below `top` (within `max_drop`), probed under a disc of
    /// radius `r` at five points. The centre ray wins unless it misses or
    /// lands more than `lip` below an edge ray, which lets characters stand on
    /// ledge lips without floating above slopes.
    pub fn ground_below(&self, top: Vec3, max_drop: f32, r: f32, lip: f32) -> Option<RayHit> {
        let walk = |t: &ColTri| t.n.y >= WALKABLE_NY;
        let center = self.raycast_filtered(top, Vec3::NEG_Y, max_drop, &walk);
        let mut highest: Option<RayHit> = None;
        for o in [Vec3::new(r, 0.0, 0.0), Vec3::new(-r, 0.0, 0.0), Vec3::new(0.0, 0.0, r), Vec3::new(0.0, 0.0, -r)] {
            if let Some(h) = self.raycast_filtered(top + o, Vec3::NEG_Y, max_drop, &walk) {
                if highest.is_none_or(|b| h.point.y > b.point.y) {
                    highest = Some(h);
                }
            }
        }
        match (center, highest) {
            (Some(c), Some(h)) if h.point.y > c.point.y + lip => Some(RayHit { point: Vec3::new(top.x, h.point.y, top.z), ..h }),
            (Some(c), _) => Some(c),
            (None, Some(h)) => Some(RayHit { point: Vec3::new(top.x, h.point.y, top.z), ..h }),
            (None, None) => None,
        }
    }

    /// Moves a vertical character by `delta`, sliding along walls and
    /// snapping onto floors / stairs.
    pub fn move_character(&self, pos: Vec3, delta: Vec3, shape: CharacterShape, was_grounded: bool) -> MoveResult {
        let mut res = MoveResult { pos, ..Default::default() };
        let horiz = Vec3::new(delta.x, 0.0, delta.z);
        let steps = ((horiz.length() / (shape.radius * 0.5)).ceil() as usize).clamp(1, 16);
        let step_v = horiz / steps as f32;
        let mut contacts = Vec::new();
        let sphere_heights = {
            let lo = shape.step + shape.radius;
            let hi = (shape.height - shape.radius).max(lo);
            let mid = (lo + hi) * 0.5;
            [lo, mid, hi]
        };
        for _ in 0..steps {
            res.pos += step_v;
            for _iter in 0..3 {
                let mut push = Vec3::ZERO;
                for h in sphere_heights {
                    let c = res.pos + Vec3::Y * h;
                    self.sphere_contacts(c, shape.radius, &mut contacts);
                    for ct in &contacts {
                        // Floors and ceilings are handled by the vertical pass.
                        if ct.tri_normal.y.abs() >= WALKABLE_NY {
                            continue;
                        }
                        let n = Vec3::new(ct.normal.x, 0.0, ct.normal.z);
                        let l = n.length();
                        if l < 1e-4 {
                            continue;
                        }
                        let n = n / l;
                        let need = ct.depth;
                        // Accumulate the strongest push per axis direction.
                        let along = push.dot(n);
                        if along < need {
                            push += n * (need - along);
                        }
                        res.hit_wall = true;
                        res.wall_normal = n;
                    }
                }
                if push.length_squared() < 1e-10 {
                    break;
                }
                res.pos += push;
            }
        }

        // Vertical.
        let dy = delta.y;
        res.pos.y += dy;
        if dy > 0.0 {
            let head = res.pos + Vec3::Y * (shape.height - dy).max(0.1);
            if let Some(h) = self.raycast_filtered(head, Vec3::Y, dy + 0.05, &|t| t.n.y < -0.3) {
                res.pos.y = h.point.y - shape.height - 0.01;
                res.hit_ceiling = true;
            }
        }
        let probe_up = shape.step.max(0.05) + if dy < 0.0 { -dy } else { 0.0 };
        let snap_down = if was_grounded && dy <= 0.0 { shape.step + 0.15 } else { 0.0 };
        let top = res.pos + Vec3::Y * probe_up;
        if let Some(g) = self.ground_below(top, probe_up + snap_down + 0.02, shape.radius * 0.45, shape.step * 0.5) {
            if g.point.y >= res.pos.y - snap_down - 0.001 && g.point.y <= res.pos.y + probe_up {
                // Only land when moving down or already grounded.
                if dy <= 0.0 || was_grounded {
                    res.pos.y = g.point.y;
                    res.grounded = true;
                    res.ground_normal = g.normal;
                    res.ground_surface = g.surface;
                }
            }
        }
        res
    }
}

/// Möller–Trumbore, two-sided. Returns `t` along `dir`.
pub fn ray_triangle(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-10 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    if t >= 0.0 { Some(t) } else { None }
}

/// Closest point on triangle (Ericson, RTCD 5.1.5).
pub fn closest_point_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return a + ab * v;
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return a + ac * w;
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return b + (c - b) * w;
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    a + ab * v + ac * w
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHAPE: CharacterShape = CharacterShape { radius: 0.35, height: 1.8, step: 0.4 };

    fn floor_and_wall() -> CollisionWorld {
        let mut w = CollisionWorld::new(2.0);
        // Floor 20x20 at y=0 (CCW from above).
        w.add_quad(
            [Vec3::new(-10.0, 0.0, -10.0), Vec3::new(-10.0, 0.0, 10.0), Vec3::new(10.0, 0.0, 10.0), Vec3::new(10.0, 0.0, -10.0)],
            0,
            0,
        );
        // Wall at z = 3 facing -z.
        w.add_quad(
            [Vec3::new(-10.0, 0.0, 3.0), Vec3::new(-10.0, 4.0, 3.0), Vec3::new(10.0, 4.0, 3.0), Vec3::new(10.0, 0.0, 3.0)],
            1,
            0,
        );
        w
    }

    #[test]
    fn stands_on_floor() {
        let w = floor_and_wall();
        let r = w.move_character(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, -0.1, 0.0), SHAPE, true);
        assert!(r.grounded);
        assert!(r.pos.y.abs() < 1e-4);
    }

    #[test]
    fn blocked_by_wall_and_group_toggle() {
        let mut w = floor_and_wall();
        let mut p = Vec3::ZERO;
        for _ in 0..60 {
            p = w.move_character(p, Vec3::new(0.0, -0.01, 0.1), SHAPE, true).pos;
        }
        assert!(p.z < 3.0 - SHAPE.radius + 0.01, "{p:?}");
        assert!(p.z > 2.5);
        w.set_group_enabled(1, false);
        for _ in 0..30 {
            p = w.move_character(p, Vec3::new(0.0, -0.01, 0.1), SHAPE, true).pos;
        }
        assert!(p.z > 4.0);
    }

    #[test]
    fn walks_up_ramp() {
        let mut w = CollisionWorld::new(2.0);
        // Ramp rising 2m over 4m along +z.
        w.add_quad(
            [Vec3::new(-2.0, 0.0, 0.0), Vec3::new(-2.0, 2.0, 4.0), Vec3::new(2.0, 2.0, 4.0), Vec3::new(2.0, 0.0, 0.0)],
            0,
            0,
        );
        w.add_quad(
            [Vec3::new(-2.0, 0.0, -4.0), Vec3::new(-2.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), Vec3::new(2.0, 0.0, -4.0)],
            0,
            0,
        );
        let mut p = Vec3::new(0.0, 0.0, -1.0);
        let mut g = true;
        for _ in 0..50 {
            let r = w.move_character(p, Vec3::new(0.0, -0.05, 0.08), SHAPE, g);
            p = r.pos;
            g = r.grounded;
        }
        assert!(g);
        assert!((p.y - p.z * 0.5).abs() < 0.05, "{p:?}");
    }

    #[test]
    fn falls_off_ledge() {
        let w = floor_and_wall();
        let r = w.move_character(Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.0, -0.1, 0.0), SHAPE, false);
        assert!(!r.grounded);
        let r = w.move_character(Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.0, -0.1, 0.0), SHAPE, false);
        assert!(r.grounded);
    }

    #[test]
    fn raycast_hits() {
        let w = floor_and_wall();
        let h = w.raycast(Vec3::new(0.0, 1.0, 0.0), Vec3::Z, 10.0).unwrap();
        assert!((h.t - 3.0).abs() < 1e-4);
    }
}
