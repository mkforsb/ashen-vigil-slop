//! Generic animation playback: clips, crossfades, masked overlay layers and
//! two-bone IK.
//!
//! Clip *content* lives in the game crate (it knows the rigs); the engine only
//! defines how clips are sampled and blended.

use crate::math::{Mat3, Quat, Vec2, Vec3};
use crate::skeleton::{Pose, Skeleton};
use std::collections::HashMap;

/// Extra inputs that procedural clips may use (locomotion speed etc).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnimParams {
    /// Horizontal speed in m/s.
    pub speed: f32,
    /// Movement direction in the character's local frame (x: left, y: forward).
    pub move_dir: Vec2,
    /// Generic 0..1 blend parameter (e.g. block raise amount).
    pub blend: f32,
}

pub trait Clip {
    fn name(&self) -> &str;
    fn duration(&self) -> f32;
    fn looping(&self) -> bool;
    /// Writes a full pose for time `t` (seconds, already wrapped/clamped).
    fn sample(&self, t: f32, params: &AnimParams, skel: &Skeleton, out: &mut Pose);
}

#[derive(Default)]
pub struct ClipLibrary {
    clips: Vec<Box<dyn Clip>>,
    by_name: HashMap<String, usize>,
}

pub type ClipId = usize;

impl ClipLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, clip: Box<dyn Clip>) -> ClipId {
        let id = self.clips.len();
        let prev = self.by_name.insert(clip.name().to_string(), id);
        assert!(prev.is_none(), "duplicate clip name {}", clip.name());
        self.clips.push(clip);
        id
    }

    pub fn get(&self, id: ClipId) -> &dyn Clip {
        self.clips[id].as_ref()
    }

    pub fn id(&self, name: &str) -> ClipId {
        *self.by_name.get(name).unwrap_or_else(|| panic!("unknown clip {name}"))
    }

    pub fn try_id(&self, name: &str) -> Option<ClipId> {
        self.by_name.get(name).copied()
    }

    pub fn len(&self) -> usize {
        self.clips.len()
    }

    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playback {
    pub clip: ClipId,
    pub time: f32,
    pub speed: f32,
}

impl Playback {
    fn new(clip: ClipId) -> Self {
        Playback { clip, time: 0.0, speed: 1.0 }
    }

    fn advance(&mut self, dt: f32, lib: &ClipLibrary) {
        let c = lib.get(self.clip);
        self.time += dt * self.speed;
        let d = c.duration().max(1e-4);
        if c.looping() {
            self.time = self.time.rem_euclid(d);
        } else {
            self.time = self.time.min(d);
        }
    }
}

/// An overlay layer applied on top of the base animation through a bone mask.
#[derive(Clone, Debug)]
pub struct Overlay {
    pub play: Playback,
    pub weight: f32,
    pub mask: Vec<bool>,
}

/// Plays one base clip with crossfading, plus an optional masked overlay.
#[derive(Clone, Debug)]
pub struct Animator {
    pub current: Playback,
    prev: Option<Playback>,
    fade: f32,
    fade_dur: f32,
    pub overlay: Option<Overlay>,
    scratch: Option<Pose>,
}

impl Animator {
    pub fn new(clip: ClipId) -> Self {
        Animator { current: Playback::new(clip), prev: None, fade: 0.0, fade_dur: 0.0, overlay: None, scratch: None }
    }

    /// Switches to `clip` with a crossfade. Re-playing the current clip keeps
    /// playing it unless `restart` is set.
    pub fn play(&mut self, clip: ClipId, fade: f32, restart: bool) {
        if clip == self.current.clip && !restart {
            return;
        }
        if fade > 0.0 {
            self.prev = Some(self.current);
            self.fade = 0.0;
            self.fade_dur = fade;
        } else {
            self.prev = None;
        }
        self.current = Playback::new(clip);
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.current.speed = speed;
    }

    pub fn time(&self) -> f32 {
        self.current.time
    }

    pub fn set_time(&mut self, t: f32) {
        self.current.time = t;
    }

    pub fn update(&mut self, dt: f32, lib: &ClipLibrary) {
        self.current.advance(dt, lib);
        if let Some(p) = self.prev.as_mut() {
            p.advance(dt, lib);
            self.fade += dt;
            if self.fade >= self.fade_dur {
                self.prev = None;
            }
        }
        if let Some(o) = self.overlay.as_mut() {
            o.play.advance(dt, lib);
        }
    }

    pub fn evaluate(&mut self, lib: &ClipLibrary, skel: &Skeleton, params: &AnimParams, out: &mut Pose) {
        let mut tmp = self.scratch.take().unwrap_or_else(|| Pose::identity(skel.len()));
        lib.get(self.current.clip).sample(self.current.time, params, skel, out);
        if let Some(p) = self.prev {
            lib.get(p.clip).sample(p.time, params, skel, &mut tmp);
            let t = crate::math::smoothstep(0.0, 1.0, self.fade / self.fade_dur.max(1e-4));
            // out = mix(prev, current, t)
            tmp.blend_from(out, t, None);
            out.clone_from(&tmp);
        }
        if let Some(o) = &self.overlay {
            if o.weight > 0.0 {
                lib.get(o.play.clip).sample(o.play.time, params, skel, &mut tmp);
                out.blend_from(&tmp, o.weight, Some(&o.mask));
            }
        }
        self.scratch = Some(tmp);
    }
}

/// Builds a world rotation whose local -Y axis points along `dir` and whose
/// local +Z axis is as close as possible to `z_hint`.
pub fn bone_basis(dir: Vec3, z_hint: Vec3) -> Quat {
    let y = -dir.normalize_or(Vec3::NEG_Y);
    let mut z = z_hint - y * z_hint.dot(y);
    if z.length_squared() < 1e-8 {
        z = if y.dot(Vec3::Z).abs() < 0.9 { Vec3::Z - y * y.z } else { Vec3::Y - y * y.y };
    }
    let z = z.normalize();
    let x = y.cross(z);
    Quat::from_mat3(&Mat3::from_cols(x, y, z)).normalize()
}

/// Result of a two-bone IK solve, as model-space bone rotations.
#[derive(Clone, Copy, Debug)]
pub struct IkSolution {
    pub upper: Quat,
    pub lower: Quat,
    pub mid: Vec3,
    pub end: Vec3,
}

/// Two-bone IK for limbs that hang along -Y in their rest pose.
///
/// - `start`: model-space position of the upper joint (shoulder / hip).
/// - `pole`: direction the middle joint (elbow / knee) should point towards.
/// - `bend_plus_z`: true if the lower bone bends towards the upper bone's +Z
///   (elbows: forearm folds forwards), false for -Z (knees).
pub fn two_bone_ik(start: Vec3, l1: f32, l2: f32, target: Vec3, pole: Vec3, bend_plus_z: bool) -> IkSolution {
    let to_t = target - start;
    let dist = to_t.length().clamp(1e-4, (l1 + l2) * 0.9999).max((l1 - l2).abs() + 1e-4);
    let dir = to_t.normalize_or(Vec3::NEG_Y);
    let target = start + dir * dist;
    // Angle at the start joint from the law of cosines.
    let cos_a = ((l1 * l1 + dist * dist - l2 * l2) / (2.0 * l1 * dist)).clamp(-1.0, 1.0);
    let sin_a = (1.0 - cos_a * cos_a).sqrt();
    let mut pole_perp = pole - dir * pole.dot(dir);
    if pole_perp.length_squared() < 1e-8 {
        pole_perp = Vec3::Z - dir * dir.z;
        if pole_perp.length_squared() < 1e-8 {
            pole_perp = Vec3::X - dir * dir.x;
        }
    }
    let pole_perp = pole_perp.normalize();
    let mid = start + (dir * cos_a + pole_perp * sin_a) * l1;
    let upper_dir = (mid - start).normalize();
    let lower_dir = (target - mid).normalize_or(upper_dir);
    // Inside of the bend, perpendicular to the upper bone.
    let mut inside = to_t - upper_dir * to_t.dot(upper_dir);
    if inside.length_squared() < 1e-8 {
        inside = -pole_perp;
    }
    let z_hint = if bend_plus_z { inside } else { -inside };
    let upper = bone_basis(upper_dir, z_hint);
    // Lower bone shares the hinge (X) axis with the upper bone.
    let hinge = upper * Vec3::X;
    let y = -lower_dir;
    let z = hinge.cross(y).normalize_or(upper * Vec3::Z);
    let x = y.cross(z);
    let lower = Quat::from_mat3(&Mat3::from_cols(x, y, z)).normalize();
    IkSolution { upper, lower, mid, end: target }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ik_reaches_target() {
        let start = Vec3::new(0.2, 1.4, 0.0);
        let target = Vec3::new(0.0, 1.2, 0.5);
        let s = two_bone_ik(start, 0.3, 0.3, target, Vec3::new(1.0, -0.5, -1.0), true);
        let end = start + (s.upper * Vec3::NEG_Y) * 0.3 + (s.lower * Vec3::NEG_Y) * 0.3;
        assert!((end - target).length() < 1e-3, "{end:?}");
        // Elbow should be on the pole side.
        let mid_off = s.mid - (start + target) * 0.5;
        assert!(mid_off.dot(Vec3::new(1.0, -0.5, -1.0)) > 0.0);
        // Forearm folds towards upper bone's +Z.
        let fold = (s.lower * Vec3::NEG_Y).dot(s.upper * Vec3::Z);
        assert!(fold > 0.0);
    }

    #[test]
    fn ik_knee_bends_backwards() {
        let hip = Vec3::new(0.1, 0.9, 0.0);
        let foot = Vec3::new(0.1, 0.2, 0.1);
        let s = two_bone_ik(hip, 0.42, 0.42, foot, Vec3::Z, false);
        assert!(s.mid.z > 0.1, "knee forward");
        let fold = (s.lower * Vec3::NEG_Y).dot(s.upper * Vec3::Z);
        assert!(fold < 0.0, "shin folds backwards");
    }

    #[test]
    fn bone_basis_axes() {
        let q = bone_basis(Vec3::Z, Vec3::Y);
        assert!((q * Vec3::NEG_Y - Vec3::Z).length() < 1e-5);
        assert!((q * Vec3::Z - Vec3::Y).length() < 1e-5);
    }
}
