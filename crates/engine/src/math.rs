//! Small math helpers on top of `glam`.

pub use glam::{EulerRot, Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
use std::f32::consts::{PI, TAU};

pub const DEG: f32 = PI / 180.0;

/// Degrees to radians.
#[inline]
pub fn deg(d: f32) -> f32 {
    d * DEG
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
pub fn inv_lerp(a: f32, b: f32, v: f32) -> f32 {
    if (b - a).abs() < 1e-9 { 0.0 } else { (v - a) / (b - a) }
}

/// Maps `v` from `[a0, a1]` to `[b0, b1]`, clamped.
#[inline]
pub fn remap(v: f32, a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    lerp(b0, b1, inv_lerp(a0, a1, v).clamp(0.0, 1.0))
}

#[inline]
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

#[inline]
pub fn ease_in(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

/// Wraps an angle into `[-PI, PI)`.
#[inline]
pub fn wrap_angle(a: f32) -> f32 {
    let mut a = (a + PI) % TAU;
    if a < 0.0 {
        a += TAU;
    }
    a - PI
}

/// Shortest signed difference `to - from` in radians.
#[inline]
pub fn angle_diff(from: f32, to: f32) -> f32 {
    wrap_angle(to - from)
}

/// Moves `cur` towards `target` by at most `step`.
#[inline]
pub fn approach(cur: f32, target: f32, step: f32) -> f32 {
    if cur < target { (cur + step).min(target) } else { (cur - step).max(target) }
}

/// Rotates angle `cur` towards `target` by at most `step` radians.
#[inline]
pub fn approach_angle(cur: f32, target: f32, step: f32) -> f32 {
    let d = angle_diff(cur, target);
    if d.abs() <= step { target } else { wrap_angle(cur + step * d.signum()) }
}

/// Frame-rate independent exponential smoothing.
#[inline]
pub fn damp(cur: f32, target: f32, rate: f32, dt: f32) -> f32 {
    lerp(cur, target, 1.0 - (-rate * dt).exp())
}

#[inline]
pub fn damp_vec3(cur: Vec3, target: Vec3, rate: f32, dt: f32) -> Vec3 {
    cur.lerp(target, 1.0 - (-rate * dt).exp())
}

/// Forward vector for a yaw angle (see crate docs for the convention).
#[inline]
pub fn yaw_dir(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

/// Yaw angle of a (horizontal) direction.
#[inline]
pub fn dir_yaw(d: Vec3) -> f32 {
    d.x.atan2(d.z)
}

#[inline]
pub fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// Builds a quaternion from Euler angles in degrees.
///
/// Rotation is applied as yaw (Y), then pitch (X), then roll (Z), i.e.
/// `Ry * Rx * Rz`. With limbs hanging along -Y in rest pose:
/// - positive X swings the lower end of a limb backwards (and leans a spine
///   that points up forwards),
/// - positive Y turns towards the character's left (+X),
/// - positive Z swings the lower end of a hanging limb to the left (+X).
#[inline]
pub fn euler_deg(x: f32, y: f32, z: f32) -> Quat {
    Quat::from_euler(EulerRot::YXZ, deg(y), deg(x), deg(z))
}

/// Rotation that takes +Z to `fwd` while keeping `up` as close to +Y as possible.
pub fn look_rotation(fwd: Vec3, up: Vec3) -> Quat {
    let f = fwd.normalize_or(Vec3::Z);
    let mut r = up.cross(f);
    if r.length_squared() < 1e-8 {
        r = Vec3::Y.cross(f);
        if r.length_squared() < 1e-8 {
            r = Vec3::X;
        }
    }
    let r = r.normalize();
    let u = f.cross(r);
    Quat::from_mat3(&Mat3::from_cols(r, u, f)).normalize()
}

/// Right-handed view matrix looking from `eye` at `center`.
pub fn look_at(eye: Vec3, center: Vec3, up: Vec3) -> Mat4 {
    glam::camera::rh::view::look_at_mat4(eye, center, up)
}

/// Right-handed OpenGL-style perspective projection (`fov_y` in radians).
pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    glam::camera::rh::proj::opengl::perspective(fov_y, aspect, near, far)
}

/// Axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb { min: Vec3::splat(f32::MAX), max: Vec3::splat(f32::MIN) };

    pub fn new(a: Vec3, b: Vec3) -> Self {
        Aabb { min: a.min(b), max: a.max(b) }
    }

    pub fn from_center(c: Vec3, half: Vec3) -> Self {
        Aabb { min: c - half, max: c + half }
    }

    pub fn grow(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn union(&self, o: &Aabb) -> Aabb {
        Aabb { min: self.min.min(o.min), max: self.max.max(o.max) }
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn half(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    pub fn contains(&self, p: Vec3) -> bool {
        p.cmpge(self.min).all() && p.cmple(self.max).all()
    }

    pub fn intersects(&self, o: &Aabb) -> bool {
        self.min.cmple(o.max).all() && self.max.cmpge(o.min).all()
    }

    pub fn expand(&self, by: f32) -> Aabb {
        Aabb { min: self.min - Vec3::splat(by), max: self.max + Vec3::splat(by) }
    }

    pub fn distance_sq(&self, p: Vec3) -> f32 {
        let c = p.clamp(self.min, self.max);
        (p - c).length_squared()
    }
}

/// Closest point on segment `ab` to `p`, returned as the segment parameter.
pub fn closest_t_on_segment(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    let ab = b - a;
    let l = ab.length_squared();
    if l < 1e-12 { 0.0 } else { ((p - a).dot(ab) / l).clamp(0.0, 1.0) }
}

pub fn closest_point_on_segment(a: Vec3, b: Vec3, p: Vec3) -> Vec3 {
    a + (b - a) * closest_t_on_segment(a, b, p)
}

/// Closest points between segments `p1q1` and `p2q2` (Ericson, RTCD 5.1.9).
pub fn closest_points_segments(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (Vec3, Vec3) {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let eps = 1e-9;
    let (s, t);
    if a <= eps && e <= eps {
        return (p1, p2);
    }
    if a <= eps {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e <= eps {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s0 = if denom != 0.0 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = (-c / a).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = s0;
            t = t0;
        }
    }
    (p1 + d1 * s, p2 + d2 * t)
}

/// Capsule (segment with radius).
#[derive(Clone, Copy, Debug)]
pub struct Capsule {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

impl Capsule {
    pub fn new(a: Vec3, b: Vec3, radius: f32) -> Self {
        Capsule { a, b, radius }
    }

    /// Returns the contact point (midpoint between the closest axis points) if overlapping.
    pub fn intersects(&self, o: &Capsule) -> Option<Vec3> {
        let (p, q) = closest_points_segments(self.a, self.b, o.a, o.b);
        let r = self.radius + o.radius;
        if (p - q).length_squared() <= r * r { Some((p + q) * 0.5) } else { None }
    }
}

/// View frustum for culling, extracted from a view-projection matrix.
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    pub fn from_matrix(m: &Mat4) -> Self {
        let r0 = m.row(0);
        let r1 = m.row(1);
        let r2 = m.row(2);
        let r3 = m.row(3);
        let mut planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 + r2, r3 - r2];
        for p in planes.iter_mut() {
            let l = p.truncate().length();
            *p /= l;
        }
        Frustum { planes }
    }

    pub fn sphere_visible(&self, c: Vec3, r: f32) -> bool {
        self.planes.iter().all(|p| p.truncate().dot(c) + p.w >= -r)
    }

    pub fn aabb_visible(&self, b: &Aabb) -> bool {
        self.planes.iter().all(|p| {
            let n = p.truncate();
            let v = Vec3::new(
                if n.x >= 0.0 { b.max.x } else { b.min.x },
                if n.y >= 0.0 { b.max.y } else { b.min.y },
                if n.z >= 0.0 { b.max.z } else { b.min.z },
            );
            n.dot(v) + p.w >= 0.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_conventions_agree() {
        for a in [0.0f32, 0.7, -2.0, 3.0] {
            let q = Quat::from_rotation_y(a) * Vec3::Z;
            assert!((q - yaw_dir(a)).length() < 1e-5);
            assert!((dir_yaw(yaw_dir(a)) - wrap_angle(a)).abs() < 1e-5);
        }
    }

    #[test]
    fn euler_convention_matches_docs() {
        // Hanging arm swung forwards needs negative X.
        let fwd = euler_deg(-90.0, 0.0, 0.0) * Vec3::NEG_Y;
        assert!((fwd - Vec3::Z).length() < 1e-5);
        // Positive Z swings a hanging limb towards +X.
        let side = euler_deg(0.0, 0.0, 90.0) * Vec3::NEG_Y;
        assert!((side - Vec3::X).length() < 1e-5);
        // Positive Y turns forward towards +X.
        let turn = euler_deg(0.0, 90.0, 0.0) * Vec3::Z;
        assert!((turn - Vec3::X).length() < 1e-5);
    }

    #[test]
    fn look_rotation_maps_z() {
        let d = Vec3::new(0.3, -0.5, 0.8).normalize();
        let q = look_rotation(d, Vec3::Y);
        assert!((q * Vec3::Z - d).length() < 1e-4);
    }

    #[test]
    fn angles_wrap() {
        assert!((angle_diff(3.0, -3.0) - (TAU - 6.0)).abs() < 1e-5);
        assert!((approach_angle(3.0, -3.0, 10.0) + 3.0).abs() < 1e-5);
    }

    #[test]
    fn segment_closest() {
        let (p, q) = closest_points_segments(
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, -1.0),
            Vec3::new(0.0, 1.0, 1.0),
        );
        assert!(p.length() < 1e-5);
        assert!((q - Vec3::Y).length() < 1e-5);
    }
}
