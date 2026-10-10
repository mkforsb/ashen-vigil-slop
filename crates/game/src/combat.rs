//! Data-driven combat definitions: attacks, weapons, shields, hurtboxes.
//!
//! New weapons are added by defining their attacks in [`attack_table`] and a
//! [`WeaponDef`] with a moveset referencing them; the action system needs no
//! changes.

use crate::anims::demon::clip as dc;
use crate::anims::humanoid::clip as hc;
use crate::assets::sounds::Sfx;
use crate::rig::{demon as db, humanoid as hb};
use ashen_engine::math::Vec3;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Knock {
    /// Brief flinch (only if poise breaks).
    Flinch,
    /// Long stagger (only if poise breaks).
    Stagger,
    /// Always knocks the target down.
    Knockdown,
}

#[derive(Clone, Copy, Debug)]
pub enum HitShape {
    /// The equipped right-hand weapon's blade capsule.
    Weapon,
    /// Capsule in a bone's local space.
    Bone { bone: usize, a: Vec3, b: Vec3, r: f32 },
    /// Sphere relative to the actor's root (model space).
    Sphere { center: Vec3, r: f32 },
}

#[derive(Clone, Copy, Debug)]
pub struct HitWindow {
    /// Normalised start/end time within the attack.
    pub start: f32,
    pub end: f32,
    pub shape: HitShape,
    /// Motion value: damage multiplier on the attacker's attack rating.
    pub mv: f32,
    pub poise: f32,
    /// Stamina damage dealt to a blocking target.
    pub stamina: f32,
    pub knock: Knock,
    pub unblockable: bool,
}

/// Something that happens at a point in an attack (sounds, shakes, dust).
#[derive(Clone, Copy, Debug)]
pub enum CueKind {
    Sound(Sfx),
    /// Ground impact: sound, camera shake strength and dust at a bone (or root).
    Impact { sfx: Sfx, shake: f32, bone: Option<usize> },
}

#[derive(Clone, Copy, Debug)]
pub struct Cue {
    pub t: f32,
    pub kind: CueKind,
}

#[derive(Clone, Debug)]
pub struct AttackDef {
    pub name: &'static str,
    pub clip: &'static str,
    pub dur: f32,
    pub stamina: f32,
    pub windows: Vec<HitWindow>,
    /// Rotation towards the target is allowed until this normalised time.
    pub track_until: f32,
    /// Tracking speed in degrees per second.
    pub track_speed: f32,
    /// Forward root motion: (normalised time, speed m/s), linearly interpolated.
    pub motion: Vec<(f32, f32)>,
    /// Earliest normalised time a queued follow-up attack starts.
    pub chain_at: f32,
    /// Earliest normalised time a roll/block/move cancels the recovery.
    pub cancel_at: f32,
    pub hyperarmor: Option<(f32, f32)>,
    pub cues: Vec<Cue>,
    /// Normalised window during which the root travels towards the target
    /// captured at the start (leaping attacks).
    pub leap: Option<(f32, f32, f32)>,
    /// Extra vertical lift curve (normalised time, metres) applied to the body.
    pub lift: Vec<(f32, f32)>,
}

impl AttackDef {
    fn new(name: &'static str, clip: &'static str, dur: f32) -> Self {
        AttackDef {
            name,
            clip,
            dur,
            stamina: 0.0,
            windows: Vec::new(),
            track_until: 0.3,
            track_speed: 360.0,
            motion: Vec::new(),
            chain_at: 0.6,
            cancel_at: 0.75,
            hyperarmor: None,
            cues: Vec::new(),
            leap: None,
            lift: Vec::new(),
        }
    }

    pub fn motion_at(&self, t: f32) -> f32 {
        sample_curve(&self.motion, t)
    }

    pub fn lift_at(&self, t: f32) -> f32 {
        sample_curve(&self.lift, t)
    }

    pub fn has_hyperarmor(&self, t: f32) -> bool {
        self.hyperarmor.is_some_and(|(a, b)| t >= a && t <= b)
    }
}

/// Piecewise-linear curve sampling; zero outside the keys.
pub fn sample_curve(keys: &[(f32, f32)], t: f32) -> f32 {
    if keys.is_empty() || t < keys[0].0 || t > keys[keys.len() - 1].0 {
        return 0.0;
    }
    for w in keys.windows(2) {
        let (t0, v0) = w[0];
        let (t1, v1) = w[1];
        if t >= t0 && t <= t1 {
            let u = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
            return v0 + (v1 - v0) * u;
        }
    }
    keys[keys.len() - 1].1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Atk {
    LsLight1,
    LsLight2,
    LsHeavy1,
    LsHeavy2,
    LsRun,
    LsRoll,
    LsBackstep,
    LsPlunge,
    HuskOverhead,
    HuskSwipe,
    DemonSmash,
    DemonSweep,
    DemonLeap,
    DemonPound,
    DemonLedge,
}

const fn w(start: f32, end: f32, shape: HitShape, mv: f32, poise: f32, stamina: f32, knock: Knock) -> HitWindow {
    HitWindow { start, end, shape, mv, poise, stamina, knock, unblockable: false }
}

fn cue(t: f32, s: Sfx) -> Cue {
    Cue { t, kind: CueKind::Sound(s) }
}

fn impact(t: f32, sfx: Sfx, shake: f32, bone: Option<usize>) -> Cue {
    Cue { t, kind: CueKind::Impact { sfx, shake, bone } }
}

/// Club capsule in the demon's right-hand space.
const CLUB: HitShape = HitShape::Bone { bone: db::R_HAND, a: Vec3::new(0.0, -0.25, 1.4), b: Vec3::new(0.0, -0.25, 3.3), r: 0.7 };

fn build_attacks() -> Vec<AttackDef> {
    use Knock::*;
    let mut v = Vec::new();
    // Order must match `Atk`.
    let light = |name, clip| {
        let mut a = AttackDef::new(name, clip, hc::LIGHT_DUR);
        a.stamina = 18.0;
        a.windows = vec![w(0.33, 0.50, HitShape::Weapon, 1.0, 22.0, 30.0, Flinch)];
        a.track_until = 0.3;
        a.motion = vec![(0.25, 0.0), (0.32, 2.2), (0.45, 0.6), (0.6, 0.0)];
        a.chain_at = 0.55;
        a.cancel_at = 0.62;
        a.cues = vec![cue(0.32, Sfx::SwingLight)];
        a
    };
    v.push(light("Longsword light", hc::LIGHT1));
    v.push(light("Longsword light (backhand)", hc::LIGHT2));
    let mut a = AttackDef::new("Longsword heavy", hc::HEAVY1, hc::HEAVY1_DUR);
    a.stamina = 28.0;
    a.windows = vec![w(0.42, 0.57, HitShape::Weapon, 1.45, 40.0, 55.0, Stagger)];
    a.track_until = 0.38;
    a.motion = vec![(0.38, 0.0), (0.45, 3.0), (0.55, 0.8), (0.65, 0.0)];
    a.chain_at = 0.62;
    a.cancel_at = 0.7;
    a.cues = vec![cue(0.42, Sfx::SwingHeavy)];
    v.push(a);
    let mut a = AttackDef::new("Longsword thrust", hc::HEAVY2, hc::HEAVY2_DUR);
    a.stamina = 28.0;
    a.windows = vec![w(0.38, 0.52, HitShape::Weapon, 1.4, 40.0, 50.0, Stagger)];
    a.track_until = 0.36;
    a.motion = vec![(0.34, 0.0), (0.41, 4.2), (0.5, 1.0), (0.6, 0.0)];
    a.chain_at = 0.62;
    a.cancel_at = 0.7;
    a.cues = vec![cue(0.38, Sfx::SwingHeavy)];
    v.push(a);
    let mut a = AttackDef::new("Running attack", hc::RUN_ATTACK, hc::RUN_ATTACK_DUR);
    a.stamina = 22.0;
    a.windows = vec![w(0.26, 0.42, HitShape::Weapon, 1.3, 35.0, 45.0, Stagger)];
    a.track_until = 0.15;
    a.motion = vec![(0.0, 5.5), (0.34, 4.0), (0.45, 0.5), (0.6, 0.0)];
    a.chain_at = 0.65;
    a.cancel_at = 0.72;
    a.cues = vec![cue(0.26, Sfx::SwingHeavy)];
    v.push(a);
    let mut a = AttackDef::new("Rolling attack", hc::ROLL_ATTACK, hc::ROLL_ATTACK_DUR);
    a.stamina = 18.0;
    a.windows = vec![w(0.22, 0.4, HitShape::Weapon, 0.95, 22.0, 30.0, Flinch)];
    a.track_until = 0.15;
    a.motion = vec![(0.0, 1.5), (0.25, 2.8), (0.42, 0.0)];
    a.chain_at = 0.6;
    a.cancel_at = 0.65;
    a.cues = vec![cue(0.22, Sfx::SwingLight)];
    v.push(a);
    let mut a = light("Backstep attack", hc::LIGHT1);
    a.motion = vec![(0.0, 1.5), (0.32, 2.5), (0.45, 0.6), (0.6, 0.0)];
    v.push(a);
    let mut a = AttackDef::new("Plunging attack", hc::PLUNGE_LAND, hc::PLUNGE_LAND_DUR);
    a.windows = vec![w(0.0, 0.12, HitShape::Weapon, 5.0, 200.0, 200.0, Knockdown)];
    a.track_until = 0.0;
    a.chain_at = 0.8;
    a.cancel_at = 0.8;
    v.push(a);

    // Husks.
    let mut a = AttackDef::new("Husk overhead", hc::HUSK_ATTACK1, hc::HUSK_ATTACK1_DUR);
    a.windows = vec![w(0.45, 0.58, HitShape::Weapon, 1.0, 22.0, 28.0, Flinch)];
    a.track_until = 0.38;
    a.track_speed = 200.0;
    a.motion = vec![(0.42, 0.0), (0.5, 2.0), (0.58, 0.0)];
    a.cancel_at = 1.0;
    a.cues = vec![cue(0.45, Sfx::SwingLight), cue(0.1, Sfx::HuskGroan)];
    v.push(a);
    let mut a = AttackDef::new("Husk swipe", hc::HUSK_ATTACK2, hc::HUSK_ATTACK2_DUR);
    a.windows = vec![w(0.42, 0.55, HitShape::Weapon, 0.9, 20.0, 25.0, Flinch)];
    a.track_until = 0.36;
    a.track_speed = 200.0;
    a.motion = vec![(0.38, 0.0), (0.46, 1.6), (0.55, 0.0)];
    a.cancel_at = 1.0;
    a.cues = vec![cue(0.42, Sfx::SwingLight)];
    v.push(a);

    // Warden Demon.
    let mut a = AttackDef::new("Hammer drive", dc::SMASH, dc::SMASH_DUR);
    a.windows = vec![
        w(0.45, 0.55, CLUB, 380.0, 100.0, 150.0, Knockdown),
        w(0.52, 0.56, HitShape::Sphere { center: Vec3::new(0.0, 0.0, 4.6), r: 2.4 }, 200.0, 60.0, 90.0, Stagger),
    ];
    a.track_until = 0.38;
    a.track_speed = 70.0;
    a.motion = vec![(0.42, 0.0), (0.5, 1.5), (0.55, 0.0)];
    a.hyperarmor = Some((0.0, 1.0));
    a.cues = vec![cue(0.05, Sfx::DemonGrunt), cue(0.44, Sfx::SwingDemon), impact(0.52, Sfx::DemonSlam, 0.9, Some(db::R_HAND))];
    v.push(a);
    let mut a = AttackDef::new("Hammer sweep", dc::SWEEP, dc::SWEEP_DUR);
    a.windows = vec![w(0.43, 0.58, CLUB, 320.0, 80.0, 130.0, Knockdown)];
    a.track_until = 0.36;
    a.track_speed = 60.0;
    a.motion = vec![(0.4, 0.0), (0.48, 1.0), (0.58, 0.0)];
    a.hyperarmor = Some((0.0, 1.0));
    a.cues = vec![cue(0.05, Sfx::DemonGrunt), cue(0.43, Sfx::SwingDemon)];
    v.push(a);
    let mut a = AttackDef::new("Leaping smash", dc::LEAP, dc::LEAP_DUR);
    a.windows = vec![
        w(0.56, 0.66, CLUB, 400.0, 100.0, 160.0, Knockdown),
        w(0.61, 0.65, HitShape::Sphere { center: Vec3::new(0.0, 0.0, 1.0), r: 3.6 }, 220.0, 80.0, 100.0, Knockdown),
    ];
    a.track_until = 0.25;
    a.track_speed = 120.0;
    a.leap = Some((0.28, 0.62, 16.0));
    a.hyperarmor = Some((0.0, 1.0));
    a.cues = vec![cue(0.24, Sfx::DemonWing), cue(0.4, Sfx::DemonWing), cue(0.5, Sfx::DemonRoar), impact(0.62, Sfx::DemonSlam, 1.0, None)];
    v.push(a);
    let mut a = AttackDef::new("Ground pound", dc::POUND, dc::POUND_DUR);
    a.windows = vec![
        w(0.5, 0.56, HitShape::Sphere { center: Vec3::new(0.0, 1.0, 0.5), r: 3.0 }, 300.0, 100.0, 120.0, Knockdown),
        w(0.53, 0.6, HitShape::Sphere { center: Vec3::new(0.0, 0.0, 0.0), r: 5.0 }, 200.0, 70.0, 100.0, Knockdown),
    ];
    a.track_until = 0.3;
    a.track_speed = 50.0;
    a.hyperarmor = Some((0.0, 1.0));
    a.cues = vec![cue(0.1, Sfx::DemonGrunt), cue(0.3, Sfx::DemonWing), impact(0.52, Sfx::DemonSlam, 1.0, None)];
    v.push(a);
    let mut a = AttackDef::new("Ledge slam", dc::LEAP, dc::LEAP_DUR);
    a.windows = vec![w(0.5, 0.64, CLUB, 380.0, 100.0, 160.0, Knockdown)];
    a.track_until = 0.4;
    a.track_speed = 90.0;
    a.lift = vec![(0.25, 0.0), (0.45, 6.5), (0.6, 6.0), (0.75, 0.0)];
    a.hyperarmor = Some((0.0, 1.0));
    a.cues = vec![cue(0.24, Sfx::DemonWing), cue(0.36, Sfx::DemonWing), cue(0.46, Sfx::DemonWing), impact(0.75, Sfx::DemonSlam, 0.8, None)];
    v.push(a);
    v
}

pub fn attacks() -> &'static [AttackDef] {
    static T: OnceLock<Vec<AttackDef>> = OnceLock::new();
    T.get_or_init(build_attacks)
}

pub fn attack(a: Atk) -> &'static AttackDef {
    &attacks()[a as usize]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WeaponKind {
    Longsword,
    BrokenSword,
    Bow,
    DemonClub,
}

#[derive(Clone, Debug)]
pub struct WeaponDef {
    pub name: &'static str,
    /// Physical attack rating.
    pub damage: f32,
    /// Blade capsule along hand +Z: (base z, tip z, radius), from the fist.
    pub blade: (f32, f32, f32),
    pub light: &'static [Atk],
    pub heavy: &'static [Atk],
    pub running: Option<Atk>,
    pub rolling: Option<Atk>,
    pub backstep: Option<Atk>,
    pub plunge: Option<Atk>,
}

pub fn weapon(k: WeaponKind) -> WeaponDef {
    use Atk::*;
    match k {
        WeaponKind::Longsword => WeaponDef {
            name: "Longsword",
            damage: 110.0,
            blade: (0.1, 1.0, 0.09),
            light: &[LsLight1, LsLight2],
            heavy: &[LsHeavy1, LsHeavy2],
            running: Some(LsRun),
            rolling: Some(LsRoll),
            backstep: Some(LsBackstep),
            plunge: Some(LsPlunge),
        },
        WeaponKind::BrokenSword => WeaponDef {
            name: "Broken Sword",
            damage: 70.0,
            blade: (0.05, 0.45, 0.08),
            light: &[HuskOverhead, HuskSwipe],
            heavy: &[HuskOverhead],
            running: None,
            rolling: None,
            backstep: None,
            plunge: None,
        },
        WeaponKind::Bow => WeaponDef {
            name: "Short Bow",
            damage: 55.0,
            blade: (0.0, 0.0, 0.0),
            light: &[],
            heavy: &[],
            running: None,
            rolling: None,
            backstep: None,
            plunge: None,
        },
        WeaponKind::DemonClub => WeaponDef {
            name: "Warden's Club",
            damage: 1.0,
            blade: (1.4, 3.3, 0.7),
            light: &[DemonSmash],
            heavy: &[DemonSweep],
            running: None,
            rolling: None,
            backstep: None,
            plunge: None,
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ShieldKind {
    KiteShield,
}

#[derive(Clone, Copy, Debug)]
pub struct ShieldDef {
    pub name: &'static str,
    /// Fraction of physical damage blocked.
    pub absorb: f32,
    /// Reduces stamina lost when blocking (0..100).
    pub stability: f32,
}

pub fn shield(k: ShieldKind) -> ShieldDef {
    match k {
        ShieldKind::KiteShield => ShieldDef { name: "Kite Shield", absorb: 1.0, stability: 60.0 },
    }
}

/// Vulnerable capsule in a bone's space.
#[derive(Clone, Copy, Debug)]
pub struct Hurtbox {
    pub bone: usize,
    pub a: Vec3,
    pub b: Vec3,
    pub r: f32,
}

const fn hb(bone: usize, a: Vec3, b: Vec3, r: f32) -> Hurtbox {
    Hurtbox { bone, a, b, r }
}

pub fn human_hurtboxes() -> &'static [Hurtbox] {
    const Z: Vec3 = Vec3::ZERO;
    static H: [Hurtbox; 6] = [
        hb(hb::PELVIS, Vec3::new(0.0, -0.1, 0.0), Vec3::new(0.0, 0.1, 0.0), 0.2),
        hb(hb::CHEST, Vec3::new(0.0, -0.2, 0.0), Vec3::new(0.0, 0.2, 0.0), 0.24),
        hb(hb::HEAD, Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.0, 0.25, 0.0), 0.14),
        hb(hb::L_THIGH, Z, Vec3::new(0.0, -0.43, 0.0), 0.1),
        hb(hb::R_THIGH, Z, Vec3::new(0.0, -0.43, 0.0), 0.1),
        hb(hb::L_SHIN, Z, Vec3::new(0.0, -0.42, 0.0), 0.09),
    ];
    &H
}

pub fn demon_hurtboxes() -> &'static [Hurtbox] {
    const Z: Vec3 = Vec3::ZERO;
    static H: [Hurtbox; 12] = [
        hb(db::PELVIS, Vec3::new(0.0, 0.0, -0.1), Vec3::new(0.0, 0.3, -0.1), 1.05),
        hb(db::BELLY, Vec3::new(0.0, 0.3, 0.3), Vec3::new(0.0, 1.1, 0.3), 1.35),
        hb(db::CHEST, Vec3::new(-0.8, 0.35, 0.0), Vec3::new(0.8, 0.35, 0.0), 0.9),
        hb(db::HEAD, Vec3::new(0.0, 0.1, 0.2), Vec3::new(0.0, 0.3, 0.4), 0.55),
        hb(db::L_UPPER, Z, Vec3::new(0.0, -1.3, 0.0), 0.42),
        hb(db::R_UPPER, Z, Vec3::new(0.0, -1.3, 0.0), 0.42),
        hb(db::L_FORE, Z, Vec3::new(0.0, -1.2, 0.0), 0.35),
        hb(db::L_THIGH, Z, Vec3::new(0.0, -1.0, 0.0), 0.5),
        hb(db::R_THIGH, Z, Vec3::new(0.0, -1.0, 0.0), 0.5),
        hb(db::L_SHIN, Z, Vec3::new(0.0, -1.0, 0.0), 0.42),
        hb(db::R_SHIN, Z, Vec3::new(0.0, -1.0, 0.0), 0.42),
        hb(db::TAIL1, Z, Vec3::new(0.0, 0.0, -1.2), 0.38),
    ];
    &H
}

/// Damage after defensive absorption (never below 1).
pub fn mitigate(raw: f32, absorb: f32) -> f32 {
    (raw * (1.0 - absorb)).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_enum() {
        let t = attacks();
        assert_eq!(t.len(), Atk::DemonLedge as usize + 1);
        assert_eq!(attack(Atk::LsHeavy1).clip, hc::HEAVY1);
        assert_eq!(attack(Atk::DemonPound).clip, dc::POUND);
    }

    #[test]
    fn curves() {
        let a = attack(Atk::LsLight1);
        assert_eq!(a.motion_at(0.1), 0.0);
        assert!(a.motion_at(0.32) > 2.0);
        assert!(sample_curve(&[(0.0, 1.0), (1.0, 3.0)], 0.5) == 2.0);
    }

    #[test]
    fn windows_are_ordered() {
        for a in attacks() {
            for w in &a.windows {
                assert!(w.start < w.end && w.end <= 1.0, "{}", a.name);
            }
            assert!(a.chain_at <= 1.0 && a.cancel_at <= 1.0);
        }
    }
}
