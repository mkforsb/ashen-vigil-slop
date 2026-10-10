//! The shared humanoid rig (player knight, husks, NPC knights).
//!
//! Model space: feet on y = 0, facing +Z, left hand side is +X.

use ashen_engine::anim::two_bone_ik;
use ashen_engine::math::{Mat4, Quat, Vec3, euler_deg, look_rotation};
use ashen_engine::skeleton::{Pose, Skeleton};

pub const PELVIS: usize = 0;
pub const SPINE: usize = 1;
pub const CHEST: usize = 2;
pub const HEAD: usize = 3;
pub const L_UPPER: usize = 4;
pub const L_FORE: usize = 5;
pub const L_HAND: usize = 6;
pub const R_UPPER: usize = 7;
pub const R_FORE: usize = 8;
pub const R_HAND: usize = 9;
pub const L_THIGH: usize = 10;
pub const L_SHIN: usize = 11;
pub const L_FOOT: usize = 12;
pub const R_THIGH: usize = 13;
pub const R_SHIN: usize = 14;
pub const R_FOOT: usize = 15;
pub const BONES: usize = 16;

pub const PELVIS_Y: f32 = 0.95;
pub const UPPER_LEN: f32 = 0.29;
pub const FORE_LEN: f32 = 0.26;
pub const THIGH_LEN: f32 = 0.43;
pub const SHIN_LEN: f32 = 0.42;
pub const SHOULDER_X: f32 = 0.21;
pub const HIP_X: f32 = 0.11;
/// Hand-bone-space offset of the fist centre (where weapon grips sit).
pub const FIST: Vec3 = Vec3::new(0.0, -0.07, 0.0);

pub fn skeleton() -> Skeleton {
    let mut s = Skeleton::new();
    let pelvis = s.add("pelvis", None, Vec3::new(0.0, PELVIS_Y, 0.0));
    let spine = s.add("spine", Some(pelvis), Vec3::new(0.0, 0.10, 0.0));
    let chest = s.add("chest", Some(spine), Vec3::new(0.0, 0.25, 0.0));
    s.add("head", Some(chest), Vec3::new(0.0, 0.25, 0.0));
    let lu = s.add("l_upper", Some(chest), Vec3::new(SHOULDER_X, 0.20, 0.0));
    let lf = s.add("l_fore", Some(lu), Vec3::new(0.0, -UPPER_LEN, 0.0));
    s.add("l_hand", Some(lf), Vec3::new(0.0, -FORE_LEN, 0.0));
    let ru = s.add("r_upper", Some(chest), Vec3::new(-SHOULDER_X, 0.20, 0.0));
    let rf = s.add("r_fore", Some(ru), Vec3::new(0.0, -UPPER_LEN, 0.0));
    s.add("r_hand", Some(rf), Vec3::new(0.0, -FORE_LEN, 0.0));
    let lt = s.add("l_thigh", Some(pelvis), Vec3::new(HIP_X, -0.05, 0.0));
    let ls = s.add("l_shin", Some(lt), Vec3::new(0.0, -THIGH_LEN, 0.0));
    s.add("l_foot", Some(ls), Vec3::new(0.0, -SHIN_LEN, 0.0));
    let rt = s.add("r_thigh", Some(pelvis), Vec3::new(-HIP_X, -0.05, 0.0));
    let rs = s.add("r_shin", Some(rt), Vec3::new(0.0, -THIGH_LEN, 0.0));
    s.add("r_foot", Some(rs), Vec3::new(0.0, -SHIN_LEN, 0.0));
    assert_eq!(s.len(), BONES);
    s
}

/// Bones affected by an upper-body overlay (e.g. drinking while walking).
pub fn upper_body_mask() -> Vec<bool> {
    let mut m = vec![false; BONES];
    for b in [SPINE, CHEST, HEAD, L_UPPER, L_FORE, L_HAND, R_UPPER, R_FORE, R_HAND] {
        m[b] = true;
    }
    m
}

/// Bones affected by the shield-raise overlay.
pub fn shield_arm_mask() -> Vec<bool> {
    let mut m = vec![false; BONES];
    for b in [L_UPPER, L_FORE, L_HAND] {
        m[b] = true;
    }
    m
}

/// IK target for a hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hand {
    /// Wrist position in the body frame.
    pub pos: Vec3,
    /// Direction the elbow points to.
    pub elbow: Vec3,
    /// Direction of the held item's +Z axis (sword blade / shield face).
    pub grip: Vec3,
    /// Preferred +Y of the held item; zero means "continue the forearm".
    pub up: Vec3,
}

impl Hand {
    pub const fn new(pos: Vec3, elbow: Vec3, grip: Vec3) -> Self {
        Hand { pos, elbow, grip, up: Vec3::ZERO }
    }
    pub const fn up(mut self, up: Vec3) -> Self {
        self.up = up;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Foot {
    /// Ankle position in the body frame.
    pub pos: Vec3,
    /// Direction the knee points to.
    pub knee: Vec3,
    /// Foot yaw (degrees, + turns toes to the left) and pitch (+ toes down).
    pub yaw: f32,
    pub pitch: f32,
}

impl Foot {
    pub const fn new(pos: Vec3, knee: Vec3) -> Self {
        Foot { pos, knee, yaw: 0.0, pitch: 0.0 }
    }
}

/// High-level humanoid key pose, resolved to bone rotations via IK.
///
/// The *body frame* is model space rotated by `body` around the pelvis;
/// hand and foot targets live in that frame so whole-body rotations (rolls,
/// falling over) carry the limbs along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HPose {
    /// Pelvis translation from its rest position (model space).
    pub hips: Vec3,
    /// Whole-body rotation around the pelvis (Euler degrees).
    pub body: Vec3,
    pub pelvis: Vec3,
    pub spine: Vec3,
    pub chest: Vec3,
    pub head: Vec3,
    pub r_hand: Hand,
    pub l_hand: Hand,
    pub r_foot: Foot,
    pub l_foot: Foot,
}

pub const fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

impl HPose {
    /// Relaxed standing pose with arms down.
    pub const fn stand() -> Self {
        HPose {
            hips: Vec3::ZERO,
            body: Vec3::ZERO,
            pelvis: Vec3::ZERO,
            spine: Vec3::ZERO,
            chest: Vec3::ZERO,
            head: Vec3::ZERO,
            r_hand: Hand::new(v3(-0.26, 0.98, 0.04), v3(-0.3, 0.0, -1.0), v3(0.0, -0.3, 1.0)),
            l_hand: Hand::new(v3(0.26, 0.98, 0.04), v3(0.3, 0.0, -1.0), v3(0.0, -0.3, 1.0)),
            r_foot: Foot::new(v3(-0.12, 0.05, 0.0), v3(-0.1, 0.0, 1.0)),
            l_foot: Foot::new(v3(0.12, 0.05, 0.0), v3(0.1, 0.0, 1.0)),
        }
    }

    /// Mirrors the pose left/right (used for strafing variations etc).
    pub fn mirrored(&self) -> Self {
        let m = |v: Vec3| Vec3::new(-v.x, v.y, v.z);
        let me = |e: Vec3| Vec3::new(e.x, -e.y, -e.z);
        let mh = |h: Hand| Hand { pos: m(h.pos), elbow: m(h.elbow), grip: m(h.grip), up: m(h.up) };
        let mf = |f: Foot| Foot { pos: m(f.pos), knee: m(f.knee), yaw: -f.yaw, pitch: f.pitch };
        HPose {
            hips: m(self.hips),
            body: me(self.body),
            pelvis: me(self.pelvis),
            spine: me(self.spine),
            chest: me(self.chest),
            head: me(self.head),
            r_hand: mh(self.l_hand),
            l_hand: mh(self.r_hand),
            r_foot: mf(self.l_foot),
            l_foot: mf(self.r_foot),
        }
    }

    /// Linear blend of two key poses (used by procedural clips).
    pub fn lerp(&self, o: &HPose, t: f32) -> HPose {
        let l = |a: Vec3, b: Vec3| a.lerp(b, t);
        let lh = |a: Hand, b: Hand| Hand { pos: l(a.pos, b.pos), elbow: l(a.elbow, b.elbow), grip: l(a.grip, b.grip), up: l(a.up, b.up) };
        let lf = |a: Foot, b: Foot| Foot {
            pos: l(a.pos, b.pos),
            knee: l(a.knee, b.knee),
            yaw: a.yaw + (b.yaw - a.yaw) * t,
            pitch: a.pitch + (b.pitch - a.pitch) * t,
        };
        HPose {
            hips: l(self.hips, o.hips),
            body: l(self.body, o.body),
            pelvis: l(self.pelvis, o.pelvis),
            spine: l(self.spine, o.spine),
            chest: l(self.chest, o.chest),
            head: l(self.head, o.head),
            r_hand: lh(self.r_hand, o.r_hand),
            l_hand: lh(self.l_hand, o.l_hand),
            r_foot: lf(self.r_foot, o.r_foot),
            l_foot: lf(self.l_foot, o.l_foot),
        }
    }

    /// Resolves the key pose into local bone rotations.
    pub fn resolve(&self, skel: &Skeleton, out: &mut Pose) {
        let rb = euler_deg(self.body.x, self.body.y, self.body.z);
        out.root_offset = self.hips;
        out.rot[PELVIS] = rb * euler_deg(self.pelvis.x, self.pelvis.y, self.pelvis.z);
        out.rot[SPINE] = euler_deg(self.spine.x, self.spine.y, self.spine.z);
        out.rot[CHEST] = euler_deg(self.chest.x, self.chest.y, self.chest.z);
        out.rot[HEAD] = euler_deg(self.head.x, self.head.y, self.head.z);

        let pelvis_pos = skel.bones[PELVIS].offset + self.hips;
        let to_model = |p: Vec3| pelvis_pos + rb * (p - pelvis_pos);
        let m_pelvis = Mat4::from_rotation_translation(out.rot[PELVIS], pelvis_pos);
        let m_spine = m_pelvis * Mat4::from_rotation_translation(out.rot[SPINE], skel.bones[SPINE].offset);
        let m_chest = m_spine * Mat4::from_rotation_translation(out.rot[CHEST], skel.bones[CHEST].offset);
        let q_chest = out.rot[PELVIS] * out.rot[SPINE] * out.rot[CHEST];

        for (upper, fore, hand, h) in [(L_UPPER, L_FORE, L_HAND, &self.l_hand), (R_UPPER, R_FORE, R_HAND, &self.r_hand)] {
            let shoulder = m_chest.transform_point3(skel.bones[upper].offset);
            let sol = two_bone_ik(shoulder, UPPER_LEN, FORE_LEN, to_model(h.pos), rb * h.elbow, true);
            out.rot[upper] = q_chest.inverse() * sol.upper;
            out.rot[fore] = sol.upper.inverse() * sol.lower;
            let up = if h.up.length_squared() < 1e-4 { sol.lower * Vec3::Y } else { rb * h.up };
            let hand_world = look_rotation(rb * h.grip, up);
            out.rot[hand] = sol.lower.inverse() * hand_world;
        }
        let q_pelvis = out.rot[PELVIS];
        for (thigh, shin, foot, f) in [(L_THIGH, L_SHIN, L_FOOT, &self.l_foot), (R_THIGH, R_SHIN, R_FOOT, &self.r_foot)] {
            let hip = m_pelvis.transform_point3(skel.bones[thigh].offset);
            let sol = two_bone_ik(hip, THIGH_LEN, SHIN_LEN, to_model(f.pos), rb * f.knee, false);
            out.rot[thigh] = q_pelvis.inverse() * sol.upper;
            out.rot[shin] = sol.upper.inverse() * sol.lower;
            let foot_world = rb * euler_deg(f.pitch, f.yaw, 0.0);
            out.rot[foot] = sol.lower.inverse() * foot_world;
        }
    }

    pub fn to_pose(&self, skel: &Skeleton) -> Pose {
        let mut p = Pose::identity(skel.len());
        self.resolve(skel, &mut p);
        p
    }
}

/// Model-space rotation of a bone in a pose (for tests / debugging).
pub fn bone_world_rot(skel: &Skeleton, pose: &Pose, bone: usize) -> Quat {
    let mut v = Vec::new();
    skel.model_rotations(pose, &mut v);
    v[bone]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ik_hands_reach_targets() {
        let s = skeleton();
        let mut p = HPose::stand();
        p.r_hand.pos = v3(-0.2, 1.4, 0.4);
        p.l_hand.pos = v3(0.3, 1.1, 0.2);
        p.chest = v3(10.0, -30.0, 0.0);
        let pose = p.to_pose(&s);
        let pos = s.model_positions(&pose);
        assert!((pos[R_HAND] - p.r_hand.pos).length() < 0.01, "{:?}", pos[R_HAND]);
        assert!((pos[L_HAND] - p.l_hand.pos).length() < 0.01, "{:?}", pos[L_HAND]);
        assert!((pos[L_FOOT] - p.l_foot.pos).length() < 0.01, "{:?}", pos[L_FOOT]);
    }

    #[test]
    fn grip_points_blade() {
        let s = skeleton();
        let mut p = HPose::stand();
        p.r_hand.grip = v3(0.0, 1.0, 0.0);
        let pose = p.to_pose(&s);
        let q = bone_world_rot(&s, &pose, R_HAND);
        assert!((q * Vec3::Z - Vec3::Y).length() < 1e-3);
    }

    #[test]
    fn body_rotation_carries_limbs() {
        let s = skeleton();
        let mut p = HPose::stand();
        p.body = v3(-90.0, 0.0, 0.0); // lying on the back
        let pose = p.to_pose(&s);
        let pos = s.model_positions(&pose);
        assert!(pos[HEAD].z < -0.3, "head goes backwards: {:?}", pos[HEAD]);
        assert!(pos[L_FOOT].z > 0.5, "feet go forwards: {:?}", pos[L_FOOT]);
    }
}
