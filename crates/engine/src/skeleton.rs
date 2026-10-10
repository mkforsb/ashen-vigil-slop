//! Rigid skeletons in the PSX tradition: every bone carries its own rigid
//! mesh piece, so there is no vertex skinning.

use crate::math::{Mat4, Quat, Vec3};

#[derive(Clone, Debug)]
pub struct Bone {
    pub name: &'static str,
    pub parent: Option<usize>,
    /// Translation from the parent joint in the parent's rest frame.
    pub offset: Vec3,
}

/// Bones are stored parents-first so a single forward pass computes world
/// transforms.
#[derive(Clone, Debug, Default)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

impl Skeleton {
    pub fn new() -> Self {
        Skeleton { bones: Vec::new() }
    }

    pub fn add(&mut self, name: &'static str, parent: Option<usize>, offset: Vec3) -> usize {
        if let Some(p) = parent {
            assert!(p < self.bones.len(), "parent must be added before child");
        }
        self.bones.push(Bone { name, parent, offset });
        self.bones.len() - 1
    }

    pub fn len(&self) -> usize {
        self.bones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }

    /// Computes model-space transforms for a pose.
    pub fn model_transforms(&self, pose: &Pose, out: &mut Vec<Mat4>) {
        out.clear();
        for (i, b) in self.bones.iter().enumerate() {
            let mut t = b.offset;
            if i == 0 {
                t += pose.root_offset;
            }
            let local = Mat4::from_rotation_translation(pose.rot[i], t);
            let m = match b.parent {
                Some(p) => out[p] * local,
                None => local,
            };
            out.push(m);
        }
    }

    /// Model-space rotation of every bone (cheaper than full matrices, used by IK).
    pub fn model_rotations(&self, pose: &Pose, out: &mut Vec<Quat>) {
        out.clear();
        for (i, b) in self.bones.iter().enumerate() {
            let r = match b.parent {
                Some(p) => out[p] * pose.rot[i],
                None => pose.rot[i],
            };
            out.push(r);
        }
    }

    /// Model-space position of every joint.
    pub fn model_positions(&self, pose: &Pose) -> Vec<Vec3> {
        let mut m = Vec::new();
        self.model_transforms(pose, &mut m);
        m.iter().map(|m| m.w_axis.truncate()).collect()
    }
}

/// Local bone rotations plus a translation for the root bone.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub rot: Vec<Quat>,
    pub root_offset: Vec3,
}

impl Pose {
    pub fn identity(n: usize) -> Self {
        Pose { rot: vec![Quat::IDENTITY; n], root_offset: Vec3::ZERO }
    }

    /// Blends towards `other` by `t`, optionally only for bones where
    /// `mask[i]` is true. The root offset blends unless `mask` excludes bone 0.
    pub fn blend_from(&mut self, other: &Pose, t: f32, mask: Option<&[bool]>) {
        if t <= 0.0 {
            return;
        }
        for i in 0..self.rot.len() {
            let w = match mask {
                Some(m) if !m[i] => continue,
                _ => t,
            };
            self.rot[i] = slerp_short(self.rot[i], other.rot[i], w);
        }
        if mask.is_none_or(|m| m[0]) {
            self.root_offset = self.root_offset.lerp(other.root_offset, t);
        }
    }
}

/// Spherical interpolation along the shortest arc.
pub fn slerp_short(a: Quat, b: Quat, t: f32) -> Quat {
    let b = if a.dot(b) < 0.0 { -b } else { b };
    a.slerp(b, t).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::euler_deg;

    #[test]
    fn chain_transforms() {
        let mut s = Skeleton::new();
        let root = s.add("root", None, Vec3::new(0.0, 1.0, 0.0));
        let arm = s.add("arm", Some(root), Vec3::new(0.5, 0.0, 0.0));
        let hand = s.add("hand", Some(arm), Vec3::new(0.0, -0.5, 0.0));
        let mut pose = Pose::identity(s.len());
        // Swing the arm forwards so the hand ends in front.
        pose.rot[arm] = euler_deg(-90.0, 0.0, 0.0);
        let p = s.model_positions(&pose);
        assert!((p[hand] - Vec3::new(0.5, 1.0, 0.5)).length() < 1e-5, "{:?}", p[hand]);
    }
}
