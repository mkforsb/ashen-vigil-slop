//! CPU-side mesh data and a builder for low-poly procedural geometry.

use crate::math::{Aabb, Mat3, Mat4, Vec2, Vec3};

/// Vertex flag: ignore lighting (emissive / full-bright).
pub const VF_UNLIT: u8 = 1;
/// Vertex flag: don't apply distance fog.
pub const VF_NOFOG: u8 = 2;

/// GPU vertex layout (40 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// Vertex colour, multiplied with texture and lighting. 255 = 1.0.
    pub color: [u8; 4],
    /// `[texture layer, VF_* flags, unused, unused]`.
    pub tex: [u8; 4],
}

impl Vertex {
    pub fn position(&self) -> Vec3 {
        Vec3::from(self.pos)
    }
}

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn tri_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn bounds(&self) -> Aabb {
        let mut b = Aabb::EMPTY;
        for v in &self.vertices {
            b.grow(v.position());
        }
        b
    }

    /// Iterates triangles as world positions.
    pub fn triangles(&self) -> impl Iterator<Item = [Vec3; 3]> + '_ {
        self.indices.chunks_exact(3).map(|t| {
            [
                self.vertices[t[0] as usize].position(),
                self.vertices[t[1] as usize].position(),
                self.vertices[t[2] as usize].position(),
            ]
        })
    }

    pub fn append(&mut self, other: &MeshData) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    /// Returns raw bytes of the vertex buffer.
    pub fn vertex_bytes(&self) -> &[u8] {
        // SAFETY: Vertex is repr(C), made only of plain-old-data fields with no padding
        // (3*4 + 3*4 + 2*4 + 4 + 4 = 40 bytes, 4-byte aligned).
        unsafe {
            std::slice::from_raw_parts(
                self.vertices.as_ptr() as *const u8,
                self.vertices.len() * std::mem::size_of::<Vertex>(),
            )
        }
    }

    pub fn index_bytes(&self) -> &[u8] {
        // SAFETY: u32 slice reinterpreted as bytes.
        unsafe { std::slice::from_raw_parts(self.indices.as_ptr() as *const u8, self.indices.len() * 4) }
    }
}

/// Colour helper: `rgb(0x30, 0x20, 0x10)`.
pub const fn rgb(r: u8, g: u8, b: u8) -> [u8; 4] {
    [r, g, b, 255]
}

pub fn rgb_f(c: Vec3) -> [u8; 4] {
    [
        (c.x.clamp(0.0, 1.0) * 255.0) as u8,
        (c.y.clamp(0.0, 1.0) * 255.0) as u8,
        (c.z.clamp(0.0, 1.0) * 255.0) as u8,
        255,
    ]
}

/// Multiplies an RGBA8 colour by a scalar.
pub fn shade(c: [u8; 4], k: f32) -> [u8; 4] {
    [
        (c[0] as f32 * k).clamp(0.0, 255.0) as u8,
        (c[1] as f32 * k).clamp(0.0, 255.0) as u8,
        (c[2] as f32 * k).clamp(0.0, 255.0) as u8,
        c[3],
    ]
}

/// Immediate-style builder with a transform stack and "current" vertex
/// attributes (colour, texture layer, flags).
pub struct MeshBuilder {
    pub mesh: MeshData,
    xf: Mat4,
    nxf: Mat3,
    stack: Vec<Mat4>,
    pub color: [u8; 4],
    pub layer: u8,
    pub flags: u8,
}

impl Default for MeshBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshBuilder {
    pub fn new() -> Self {
        MeshBuilder {
            mesh: MeshData::default(),
            xf: Mat4::IDENTITY,
            nxf: Mat3::IDENTITY,
            stack: Vec::new(),
            color: [255; 4],
            layer: 0,
            flags: 0,
        }
    }

    pub fn finish(self) -> MeshData {
        self.mesh
    }

    pub fn set_color(&mut self, c: [u8; 4]) -> &mut Self {
        self.color = c;
        self
    }

    pub fn set_layer(&mut self, l: u8) -> &mut Self {
        self.layer = l;
        self
    }

    pub fn set_flags(&mut self, f: u8) -> &mut Self {
        self.flags = f;
        self
    }

    /// Sets texture layer and colour at once.
    pub fn material(&mut self, layer: u8, color: [u8; 4]) -> &mut Self {
        self.layer = layer;
        self.color = color;
        self
    }

    pub fn push(&mut self, m: Mat4) {
        self.stack.push(self.xf);
        self.set_transform(self.xf * m);
    }

    pub fn pop(&mut self) {
        let m = self.stack.pop().expect("transform stack underflow");
        self.set_transform(m);
    }

    pub fn set_transform(&mut self, m: Mat4) {
        self.xf = m;
        self.nxf = Mat3::from_mat4(m).inverse().transpose();
    }

    pub fn vertex(&mut self, p: Vec3, n: Vec3, uv: Vec2) -> u32 {
        let c = self.color;
        self.vertex_c(p, n, uv, c)
    }

    pub fn vertex_c(&mut self, p: Vec3, n: Vec3, uv: Vec2, color: [u8; 4]) -> u32 {
        let wp = self.xf.transform_point3(p);
        let wn = (self.nxf * n).normalize_or_zero();
        let idx = self.mesh.vertices.len() as u32;
        self.mesh.vertices.push(Vertex {
            pos: wp.to_array(),
            normal: wn.to_array(),
            uv: uv.to_array(),
            color,
            tex: [self.layer, self.flags, 0, 0],
        });
        idx
    }

    pub fn tri_idx(&mut self, a: u32, b: u32, c: u32) {
        self.mesh.indices.extend_from_slice(&[a, b, c]);
    }

    /// A flat-shaded triangle, counter-clockwise when seen from the front.
    pub fn tri(&mut self, p: [Vec3; 3], uv: [Vec2; 3]) {
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
        let a = self.vertex(p[0], n, uv[0]);
        let b = self.vertex(p[1], n, uv[1]);
        let c = self.vertex(p[2], n, uv[2]);
        self.tri_idx(a, b, c);
    }

    /// A flat-shaded quad `p0 p1 p2 p3`, counter-clockwise when seen from the front.
    pub fn quad(&mut self, p: [Vec3; 4], uv: [Vec2; 4]) {
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
        self.quad_n(p, uv, n);
    }

    pub fn quad_n(&mut self, p: [Vec3; 4], uv: [Vec2; 4], n: Vec3) {
        let a = self.vertex(p[0], n, uv[0]);
        let b = self.vertex(p[1], n, uv[1]);
        let c = self.vertex(p[2], n, uv[2]);
        let d = self.vertex(p[3], n, uv[3]);
        self.tri_idx(a, b, c);
        self.tri_idx(a, c, d);
    }

    /// Quad whose winding is fixed up so its normal points away from `inside`.
    pub fn quad_out(&mut self, mut p: [Vec3; 4], mut uv: [Vec2; 4], inside: Vec3) {
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        let c = (p[0] + p[1] + p[2] + p[3]) * 0.25;
        if n.dot(c - inside) < 0.0 {
            p.swap(1, 3);
            uv.swap(1, 3);
        }
        self.quad(p, uv);
    }

    /// Generic hexahedron from a bottom and a top quad (corresponding corners).
    /// Faces are oriented outwards automatically. UVs span 0..1 per face,
    /// scaled by `uv_scale`.
    pub fn hexa(&mut self, bottom: [Vec3; 4], top: [Vec3; 4], uv_scale: Vec2) {
        let center = (bottom.iter().chain(top.iter()).copied().sum::<Vec3>()) / 8.0;
        let u = |x: f32, y: f32| Vec2::new(x * uv_scale.x, y * uv_scale.y);
        let uvs = [u(0.0, 1.0), u(1.0, 1.0), u(1.0, 0.0), u(0.0, 0.0)];
        for i in 0..4 {
            let j = (i + 1) % 4;
            self.quad_out([bottom[i], bottom[j], top[j], top[i]], uvs, center);
        }
        self.quad_out(top, [u(0.0, 0.0), u(1.0, 0.0), u(1.0, 1.0), u(0.0, 1.0)], center);
        self.quad_out(bottom, [u(0.0, 0.0), u(1.0, 0.0), u(1.0, 1.0), u(0.0, 1.0)], center);
    }

    /// Axis-aligned box. UVs are in metres times `texels_per_m / 64`, so
    /// textures tile with consistent density.
    pub fn cube(&mut self, min: Vec3, max: Vec3, uv_per_m: f32) {
        let s = max - min;
        let c = (min + max) * 0.5;
        let f = |p: Vec3| p;
        let corners = |x: f32, y: f32, z: f32| f(Vec3::new(x, y, z));
        let (x0, y0, z0, x1, y1, z1) = (min.x, min.y, min.z, max.x, max.y, max.z);
        let k = uv_per_m;
        // +X, -X
        self.quad_out(
            [corners(x1, y0, z0), corners(x1, y0, z1), corners(x1, y1, z1), corners(x1, y1, z0)],
            [Vec2::new(0.0, s.y * k), Vec2::new(s.z * k, s.y * k), Vec2::new(s.z * k, 0.0), Vec2::ZERO],
            c,
        );
        self.quad_out(
            [corners(x0, y0, z1), corners(x0, y0, z0), corners(x0, y1, z0), corners(x0, y1, z1)],
            [Vec2::new(0.0, s.y * k), Vec2::new(s.z * k, s.y * k), Vec2::new(s.z * k, 0.0), Vec2::ZERO],
            c,
        );
        // +Z, -Z
        self.quad_out(
            [corners(x1, y0, z1), corners(x0, y0, z1), corners(x0, y1, z1), corners(x1, y1, z1)],
            [Vec2::new(0.0, s.y * k), Vec2::new(s.x * k, s.y * k), Vec2::new(s.x * k, 0.0), Vec2::ZERO],
            c,
        );
        self.quad_out(
            [corners(x0, y0, z0), corners(x1, y0, z0), corners(x1, y1, z0), corners(x0, y1, z0)],
            [Vec2::new(0.0, s.y * k), Vec2::new(s.x * k, s.y * k), Vec2::new(s.x * k, 0.0), Vec2::ZERO],
            c,
        );
        // +Y, -Y
        self.quad_out(
            [corners(x0, y1, z0), corners(x1, y1, z0), corners(x1, y1, z1), corners(x0, y1, z1)],
            [Vec2::ZERO, Vec2::new(s.x * k, 0.0), Vec2::new(s.x * k, s.z * k), Vec2::new(0.0, s.z * k)],
            c,
        );
        self.quad_out(
            [corners(x0, y0, z0), corners(x1, y0, z0), corners(x1, y0, z1), corners(x0, y0, z1)],
            [Vec2::ZERO, Vec2::new(s.x * k, 0.0), Vec2::new(s.x * k, s.z * k), Vec2::new(0.0, s.z * k)],
            c,
        );
    }

    /// Box centred at `c` with half extents `h`; UV 0..1 per face.
    pub fn box_c(&mut self, c: Vec3, h: Vec3) {
        let b = [
            c + Vec3::new(-h.x, -h.y, -h.z),
            c + Vec3::new(h.x, -h.y, -h.z),
            c + Vec3::new(h.x, -h.y, h.z),
            c + Vec3::new(-h.x, -h.y, h.z),
        ];
        let t = b.map(|p| p + Vec3::new(0.0, 2.0 * h.y, 0.0));
        self.hexa(b, t, Vec2::ONE);
    }

    /// A limb-like tapered box hanging from `top` down to `bottom` (any axis).
    /// `top_half` / `bottom_half` are half widths (x: side to side, y: front to back)
    /// in a frame whose "up" is the segment direction and "forward" is close to +Z.
    pub fn taper(&mut self, top: Vec3, bottom: Vec3, top_half: Vec2, bottom_half: Vec2) {
        let axis = (top - bottom).normalize_or(Vec3::Y);
        let fwd_ref = if axis.dot(Vec3::Z).abs() > 0.9 { Vec3::Y } else { Vec3::Z };
        let side = fwd_ref.cross(axis).normalize();
        let fwd = axis.cross(side).normalize();
        let ring = |c: Vec3, h: Vec2| {
            [
                c - side * h.x - fwd * h.y,
                c + side * h.x - fwd * h.y,
                c + side * h.x + fwd * h.y,
                c - side * h.x + fwd * h.y,
            ]
        };
        self.hexa(ring(bottom, bottom_half), ring(top, top_half), Vec2::ONE);
    }

    /// Cylinder / cone frustum from `a` (radius `ra`) to `b` (radius `rb`).
    pub fn cylinder(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32, segs: u32, caps: bool, smooth: bool) {
        let axis = (b - a).normalize_or(Vec3::Y);
        let refv = if axis.dot(Vec3::Y).abs() > 0.9 { Vec3::Z } else { Vec3::Y };
        let s = refv.cross(axis).normalize();
        let t = axis.cross(s).normalize();
        let len = (b - a).length();
        let slope = (ra - rb) / len.max(1e-6);
        let dir = |i: u32| {
            let ang = i as f32 / segs as f32 * std::f32::consts::TAU;
            s * ang.cos() + t * ang.sin()
        };
        if smooth {
            let base = self.mesh.vertices.len() as u32;
            for i in 0..=segs {
                let d = dir(i);
                let n = (d + axis * slope).normalize();
                let u = i as f32 / segs as f32;
                self.vertex(a + d * ra, n, Vec2::new(u, 1.0));
                self.vertex(b + d * rb, n, Vec2::new(u, 0.0));
            }
            for i in 0..segs {
                let i0 = base + i * 2;
                // CCW seen from outside: a_i, a_{i+1}, b_{i+1}, b_i
                self.tri_idx(i0, i0 + 2, i0 + 3);
                self.tri_idx(i0, i0 + 3, i0 + 1);
            }
        } else {
            let center = (a + b) * 0.5;
            for i in 0..segs {
                let d0 = dir(i);
                let d1 = dir(i + 1);
                let u0 = i as f32 / segs as f32;
                let u1 = (i + 1) as f32 / segs as f32;
                self.quad_out(
                    [a + d0 * ra, a + d1 * ra, b + d1 * rb, b + d0 * rb],
                    [Vec2::new(u0, 1.0), Vec2::new(u1, 1.0), Vec2::new(u1, 0.0), Vec2::new(u0, 0.0)],
                    center,
                );
            }
        }
        if caps {
            for (c, r, n) in [(a, ra, -axis), (b, rb, axis)] {
                if r <= 1e-5 {
                    continue;
                }
                let ci = self.vertex(c, n, Vec2::splat(0.5));
                let first = self.mesh.vertices.len() as u32;
                for i in 0..segs {
                    let d = dir(i);
                    let ang = i as f32 / segs as f32 * std::f32::consts::TAU;
                    self.vertex(c + d * r, n, Vec2::new(0.5 + ang.cos() * 0.5, 0.5 + ang.sin() * 0.5));
                }
                for i in 0..segs {
                    let i0 = first + i;
                    let i1 = first + (i + 1) % segs;
                    if n.dot(axis) > 0.0 {
                        self.tri_idx(ci, i0, i1);
                    } else {
                        self.tri_idx(ci, i1, i0);
                    }
                }
            }
        }
    }

    /// UV-sphere scaled to an ellipsoid, smooth normals.
    pub fn ellipsoid(&mut self, c: Vec3, r: Vec3, segs_u: u32, segs_v: u32) {
        self.ellipsoid_part(c, r, segs_u, segs_v, 0.0, 1.0);
    }

    /// Partial ellipsoid between latitudes `v0..v1` (0 = top pole, 1 = bottom pole).
    pub fn ellipsoid_part(&mut self, c: Vec3, r: Vec3, segs_u: u32, segs_v: u32, v0: f32, v1: f32) {
        let base = self.mesh.vertices.len() as u32;
        for j in 0..=segs_v {
            let v = v0 + (v1 - v0) * j as f32 / segs_v as f32;
            let phi = v * std::f32::consts::PI;
            for i in 0..=segs_u {
                let u = i as f32 / segs_u as f32;
                let th = u * std::f32::consts::TAU;
                let unit = Vec3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
                let p = c + unit * r;
                let n = (unit / r).normalize_or(Vec3::Y);
                self.vertex(p, n, Vec2::new(u, v));
            }
        }
        let row = segs_u + 1;
        for j in 0..segs_v {
            for i in 0..segs_u {
                let a = base + j * row + i;
                let b = a + 1;
                let cc = a + row;
                let d = cc + 1;
                // CCW from outside
                self.tri_idx(a, b, d);
                self.tri_idx(a, d, cc);
            }
        }
    }

    /// A subdivided planar quad, used for level surfaces so per-vertex lighting
    /// and affine texture warping behave like PSX-era geometry.
    /// `origin + u*su + v*sv` spans the quad; `uv_fn` maps world position to UV.
    #[allow(clippy::too_many_arguments)]
    pub fn grid_quad(
        &mut self,
        origin: Vec3,
        su: Vec3,
        sv: Vec3,
        nu: u32,
        nv: u32,
        uv_fn: &dyn Fn(Vec3) -> Vec2,
        color_fn: &dyn Fn(Vec3) -> [u8; 4],
    ) {
        let n = su.cross(sv).normalize_or_zero();
        let base = self.mesh.vertices.len() as u32;
        for j in 0..=nv {
            for i in 0..=nu {
                let p = origin + su * (i as f32 / nu as f32) + sv * (j as f32 / nv as f32);
                let col = color_fn(self.xf.transform_point3(p));
                let uv = uv_fn(self.xf.transform_point3(p));
                self.vertex_c(p, n, uv, col);
            }
        }
        let row = nu + 1;
        for j in 0..nv {
            for i in 0..nu {
                let a = base + j * row + i;
                self.tri_idx(a, a + 1, a + row + 1);
                self.tri_idx(a, a + row + 1, a + row);
            }
        }
    }

    pub fn append(&mut self, other: &MeshData) {
        let base = self.mesh.vertices.len() as u32;
        for v in &other.vertices {
            let p = self.xf.transform_point3(Vec3::from(v.pos));
            let n = (self.nxf * Vec3::from(v.normal)).normalize_or_zero();
            self.mesh.vertices.push(Vertex { pos: p.to_array(), normal: n.to_array(), ..*v });
        }
        self.mesh.indices.extend(other.indices.iter().map(|i| i + base));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outward(mesh: &MeshData, center: Vec3) -> bool {
        mesh.triangles().all(|[a, b, c]| {
            let n = (b - a).cross(c - a);
            n.length_squared() < 1e-12 || n.dot((a + b + c) / 3.0 - center) > 0.0
        })
    }

    #[test]
    fn vertex_is_40_bytes() {
        assert_eq!(std::mem::size_of::<Vertex>(), 40);
    }

    #[test]
    fn cube_faces_point_out() {
        let mut b = MeshBuilder::new();
        b.cube(Vec3::ZERO, Vec3::new(2.0, 1.0, 3.0), 1.0);
        let m = b.finish();
        assert_eq!(m.tri_count(), 12);
        assert!(outward(&m, Vec3::new(1.0, 0.5, 1.5)));
    }

    #[test]
    fn primitives_point_out() {
        let mut b = MeshBuilder::new();
        b.ellipsoid(Vec3::ZERO, Vec3::new(1.0, 2.0, 1.0), 8, 6);
        assert!(outward(&b.finish(), Vec3::ZERO));
        let mut b = MeshBuilder::new();
        b.cylinder(Vec3::ZERO, Vec3::Y * 2.0, 0.5, 0.3, 6, true, true);
        assert!(outward(&b.finish(), Vec3::Y));
        let mut b = MeshBuilder::new();
        b.taper(Vec3::Y, Vec3::ZERO, Vec2::new(0.2, 0.1), Vec2::new(0.1, 0.1));
        assert!(outward(&b.finish(), Vec3::Y * 0.5));
    }
}
