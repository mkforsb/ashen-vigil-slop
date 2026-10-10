//! Animation clip types and the clip libraries for each rig.

pub mod demon;
pub mod humanoid;

use crate::rig::humanoid::HPose;
use ashen_engine::anim::{AnimParams, Clip};
use ashen_engine::math::{ease_in, ease_in_out, ease_out};
use ashen_engine::skeleton::{Pose, Skeleton, slerp_short};

/// Easing used when arriving at a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ease {
    Linear,
    Smooth,
    /// Accelerate into the key (fast strikes).
    In,
    /// Decelerate into the key (wind-ups, settling).
    Out,
    /// Jump to the key at the end of the segment.
    Step,
}

impl Ease {
    fn apply(self, t: f32) -> f32 {
        match self {
            Ease::Linear => t,
            Ease::Smooth => ease_in_out(t),
            Ease::In => ease_in(t),
            Ease::Out => ease_out(t),
            Ease::Step => {
                if t >= 1.0 { 1.0 } else { 0.0 }
            }
        }
    }
}

/// Keyframed clip storing fully resolved poses per key.
pub struct KeyClip {
    name: String,
    duration: f32,
    looping: bool,
    /// (time in seconds, easing into this key, pose)
    keys: Vec<(f32, Ease, Pose)>,
}

impl KeyClip {
    pub fn from_poses(name: &str, duration: f32, looping: bool, keys: Vec<(f32, Ease, Pose)>) -> Self {
        assert!(!keys.is_empty());
        KeyClip { name: name.to_string(), duration, looping, keys }
    }

    /// Builds a humanoid clip from key poses; key times are normalised (0..1).
    pub fn humanoid(name: &str, duration: f32, looping: bool, keys: &[(f32, Ease, HPose)], skel: &Skeleton) -> Self {
        let keys = keys.iter().map(|(t, e, p)| (t * duration, *e, p.to_pose(skel))).collect();
        Self::from_poses(name, duration, looping, keys)
    }
}

impl Clip for KeyClip {
    fn name(&self) -> &str {
        &self.name
    }

    fn duration(&self) -> f32 {
        self.duration
    }

    fn looping(&self) -> bool {
        self.looping
    }

    fn sample(&self, t: f32, _params: &AnimParams, _skel: &Skeleton, out: &mut Pose) {
        let keys = &self.keys;
        if keys.len() == 1 || t <= keys[0].0 && !self.looping {
            out.clone_from(&keys[0].2);
            return;
        }
        // Find the segment [i, i+1] containing t.
        let mut i = keys.len() - 1;
        for k in 0..keys.len() - 1 {
            if t < keys[k + 1].0 {
                i = k;
                break;
            }
        }
        let (a, b, span, local) = if i == keys.len() - 1 {
            if self.looping {
                // Wrap from the last key to the first.
                let span = self.duration - keys[i].0 + keys[0].0;
                (i, 0, span, if t >= keys[i].0 { t - keys[i].0 } else { t + self.duration - keys[i].0 })
            } else {
                out.clone_from(&keys[i].2);
                return;
            }
        } else if t < keys[0].0 {
            let span = self.duration - keys[keys.len() - 1].0 + keys[0].0;
            (keys.len() - 1, 0, span, t + self.duration - keys[keys.len() - 1].0)
        } else {
            (i, i + 1, keys[i + 1].0 - keys[i].0, t - keys[i].0)
        };
        let u = keys[b].1.apply((local / span.max(1e-5)).clamp(0.0, 1.0));
        let pa = &keys[a].2;
        let pb = &keys[b].2;
        for (k, r) in out.rot.iter_mut().enumerate() {
            *r = slerp_short(pa.rot[k], pb.rot[k], u);
        }
        out.root_offset = pa.root_offset.lerp(pb.root_offset, u);
    }
}

type HPoseFn = Box<dyn Fn(f32, &AnimParams) -> HPose>;

/// Procedural humanoid clip: a function of normalised phase and parameters.
pub struct ProcClip {
    name: String,
    duration: f32,
    f: HPoseFn,
}

impl ProcClip {
    pub fn new(name: &str, duration: f32, f: impl Fn(f32, &AnimParams) -> HPose + 'static) -> Self {
        ProcClip { name: name.to_string(), duration, f: Box::new(f) }
    }
}

impl Clip for ProcClip {
    fn name(&self) -> &str {
        &self.name
    }

    fn duration(&self) -> f32 {
        self.duration
    }

    fn looping(&self) -> bool {
        true
    }

    fn sample(&self, t: f32, params: &AnimParams, skel: &Skeleton, out: &mut Pose) {
        (self.f)(t / self.duration, params).resolve(skel, out);
    }
}
