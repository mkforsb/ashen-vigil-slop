//! Warden Demon clips, authored as forward-kinematics key poses.

use super::{Ease, KeyClip};
use crate::rig::demon::*;
use ashen_engine::anim::ClipLibrary;
use ashen_engine::math::{Vec3, euler_deg};
use ashen_engine::skeleton::{Pose, Skeleton};

use Ease::*;

pub mod clip {
    pub const IDLE: &str = "d.idle";
    pub const WALK: &str = "d.walk";
    pub const SMASH: &str = "d.smash";
    pub const SWEEP: &str = "d.sweep";
    pub const LEAP: &str = "d.leap";
    pub const POUND: &str = "d.pound";
    pub const ROAR: &str = "d.roar";
    pub const FALL: &str = "d.fall";
    pub const LAND: &str = "d.land";
    pub const STAGGER: &str = "d.stagger";
    pub const DEATH: &str = "d.death";

    pub const WALK_DUR: f32 = 1.8;
    pub const SMASH_DUR: f32 = 2.4;
    pub const SWEEP_DUR: f32 = 2.3;
    pub const LEAP_DUR: f32 = 3.2;
    pub const POUND_DUR: f32 = 2.5;
    pub const ROAR_DUR: f32 = 2.4;
    pub const LAND_DUR: f32 = 1.6;
    pub const STAGGER_DUR: f32 = 2.8;
    pub const DEATH_DUR: f32 = 4.0;
}

/// Demon key pose: Euler degrees per bone plus a pelvis offset.
#[derive(Clone, Copy, Debug)]
pub struct DPose {
    pub hips: Vec3,
    pub b: [Vec3; BONES],
}

const fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

impl DPose {
    pub fn base() -> Self {
        let mut b = [Vec3::ZERO; BONES];
        b[BELLY] = v(8.0, 0.0, 0.0);
        b[CHEST] = v(25.0, 0.0, 0.0);
        b[HEAD] = v(-30.0, 0.0, 0.0);
        b[JAW] = v(6.0, 0.0, 0.0);
        b[L_UPPER] = v(-15.0, 0.0, 30.0);
        b[L_FORE] = v(-25.0, 0.0, 0.0);
        b[R_UPPER] = v(-20.0, 0.0, -28.0);
        b[R_FORE] = v(-30.0, 0.0, 0.0);
        b[R_HAND] = v(60.0, 0.0, 0.0);
        b[L_THIGH] = v(-10.0, 0.0, 8.0);
        b[L_SHIN] = v(15.0, 0.0, 0.0);
        b[L_FOOT] = v(-5.0, 0.0, -8.0);
        b[R_THIGH] = v(-10.0, 0.0, -8.0);
        b[R_SHIN] = v(15.0, 0.0, 0.0);
        b[R_FOOT] = v(-5.0, 0.0, 8.0);
        b[TAIL1] = v(-25.0, 0.0, 0.0);
        b[TAIL2] = v(-10.0, 0.0, 0.0);
        b[TAIL3] = v(12.0, 0.0, 0.0);
        b[L_WING] = v(0.0, 60.0, 15.0);
        b[R_WING] = v(0.0, -60.0, -15.0);
        DPose { hips: v(0.0, -0.1, 0.0), b }
    }

    fn with(mut self, bone: usize, e: Vec3) -> Self {
        self.b[bone] = e;
        self
    }

    fn hips(mut self, h: Vec3) -> Self {
        self.hips = h;
        self
    }

    fn wings(self, l: Vec3) -> Self {
        self.with(L_WING, l).with(R_WING, v(l.x, -l.y, -l.z))
    }

    fn legs(self, thigh: f32, shin: f32) -> Self {
        let lt = self.b[L_THIGH];
        let rt = self.b[R_THIGH];
        self.with(L_THIGH, v(thigh, lt.y, lt.z)).with(R_THIGH, v(thigh, rt.y, rt.z)).with(L_SHIN, v(shin, 0.0, 0.0)).with(R_SHIN, v(shin, 0.0, 0.0))
    }

    pub fn to_pose(&self) -> Pose {
        let mut p = Pose::identity(BONES);
        for (i, e) in self.b.iter().enumerate() {
            p.rot[i] = euler_deg(e.x, e.y, e.z);
        }
        p.root_offset = self.hips;
        p
    }
}

fn clip(name: &str, dur: f32, looping: bool, keys: Vec<(f32, Ease, DPose)>) -> Box<KeyClip> {
    Box::new(KeyClip::from_poses(name, dur, looping, keys.into_iter().map(|(t, e, p)| (t * dur, e, p.to_pose())).collect()))
}

fn idle() -> Vec<(f32, Ease, DPose)> {
    let b = DPose::base();
    let breathe = b
        .with(BELLY, v(10.0, 0.0, 0.0))
        .with(CHEST, v(27.0, 0.0, 0.0))
        .with(HEAD, v(-33.0, 4.0, 0.0))
        .with(JAW, v(10.0, 0.0, 0.0))
        .with(TAIL3, v(18.0, 10.0, 0.0))
        .wings(v(0.0, 52.0, 22.0))
        .hips(v(0.0, -0.16, 0.0));
    vec![(0.0, Smooth, b), (0.5, Smooth, breathe)]
}

fn walk() -> Vec<(f32, Ease, DPose)> {
    let b = DPose::base();
    let step_l = b
        .with(L_THIGH, v(-35.0, 0.0, 8.0))
        .with(L_SHIN, v(20.0, 0.0, 0.0))
        .with(R_THIGH, v(15.0, 0.0, -8.0))
        .with(R_SHIN, v(30.0, 0.0, 0.0))
        .with(PELVIS, v(0.0, 6.0, 4.0))
        .with(CHEST, v(25.0, -5.0, -3.0))
        .with(L_UPPER, v(5.0, 0.0, 30.0))
        .with(R_UPPER, v(-30.0, 0.0, -28.0))
        .with(TAIL1, v(-25.0, -8.0, 0.0))
        .hips(v(0.0, -0.2, 0.0));
    let pass_l = b
        .with(L_THIGH, v(-8.0, 0.0, 8.0))
        .with(L_SHIN, v(10.0, 0.0, 0.0))
        .with(R_THIGH, v(-25.0, 0.0, -8.0))
        .with(R_SHIN, v(60.0, 0.0, 0.0))
        .with(R_FOOT, v(15.0, 0.0, 8.0))
        .hips(v(0.0, 0.0, 0.0));
    let mirror = |p: DPose| {
        let mut m = p;
        for (l, r) in [(L_THIGH, R_THIGH), (L_SHIN, R_SHIN), (L_FOOT, R_FOOT), (L_UPPER, R_UPPER), (L_FORE, R_FORE)] {
            m.b[l] = v(p.b[r].x, -p.b[r].y, -p.b[r].z);
            m.b[r] = v(p.b[l].x, -p.b[l].y, -p.b[l].z);
        }
        for bone in [PELVIS, CHEST, TAIL1] {
            m.b[bone] = v(p.b[bone].x, -p.b[bone].y, -p.b[bone].z);
        }
        // Keep the club hand as is: only swing the arms' forward angle.
        m.b[R_HAND] = p.b[R_HAND];
        m
    };
    vec![(0.0, Smooth, step_l), (0.25, Smooth, pass_l), (0.5, Smooth, mirror(step_l)), (0.75, Smooth, mirror(pass_l))]
}

fn club_overhead(p: DPose) -> DPose {
    p.with(R_UPPER, v(-165.0, 0.0, -15.0)).with(R_FORE, v(-30.0, 0.0, 0.0)).with(R_HAND, v(-30.0, 0.0, 0.0))
}

fn club_impact(p: DPose) -> DPose {
    p.with(R_UPPER, v(-35.0, 0.0, -10.0)).with(R_FORE, v(-10.0, 0.0, 0.0)).with(R_HAND, v(55.0, 0.0, 0.0))
}

fn smash() -> Vec<(f32, Ease, DPose)> {
    let b = DPose::base();
    let wind = club_overhead(b)
        .with(CHEST, v(5.0, -10.0, 0.0))
        .with(BELLY, v(0.0, 0.0, 0.0))
        .with(HEAD, v(-15.0, 0.0, 0.0))
        .with(JAW, v(25.0, 0.0, 0.0))
        .with(L_UPPER, v(-40.0, 0.0, 40.0))
        .with(L_FORE, v(-40.0, 0.0, 0.0))
        .with(L_THIGH, v(-25.0, 0.0, 10.0))
        .with(L_SHIN, v(25.0, 0.0, 0.0))
        .with(R_THIGH, v(10.0, 0.0, -8.0))
        .with(R_SHIN, v(25.0, 0.0, 0.0))
        .wings(v(0.0, 25.0, 35.0))
        .hips(v(0.0, -0.2, -0.1));
    let mid = wind
        .with(R_UPPER, v(-95.0, 0.0, -10.0))
        .with(R_FORE, v(-10.0, 0.0, 0.0))
        .with(R_HAND, v(20.0, 0.0, 0.0))
        .with(CHEST, v(35.0, 0.0, 0.0));
    let hit = club_impact(mid)
        .with(CHEST, v(50.0, 0.0, 0.0))
        .with(BELLY, v(15.0, 0.0, 0.0))
        .with(HEAD, v(-45.0, 0.0, 0.0))
        .with(JAW, v(10.0, 0.0, 0.0))
        .wings(v(0.0, 45.0, 20.0))
        .hips(v(0.0, -0.45, 0.35));
    vec![(0.0, Linear, b), (0.35, Out, wind), (0.45, In, mid), (0.52, Linear, hit), (0.75, Linear, hit), (1.0, Smooth, b)]
}

fn sweep() -> Vec<(f32, Ease, DPose)> {
    let b = DPose::base();
    let wind = b
        .with(CHEST, v(15.0, -60.0, 0.0))
        .with(BELLY, v(5.0, -20.0, 0.0))
        .with(HEAD, v(-25.0, 40.0, 0.0))
        .with(R_UPPER, v(-10.0, 0.0, -85.0))
        .with(R_FORE, v(-15.0, 0.0, 0.0))
        .with(R_HAND, v(0.0, -25.0, 0.0))
        .with(L_UPPER, v(-20.0, 0.0, 50.0))
        .with(L_THIGH, v(-20.0, 0.0, 15.0))
        .with(R_THIGH, v(5.0, 0.0, -15.0))
        .hips(v(0.0, -0.3, 0.0));
    let mid = wind
        .with(CHEST, v(15.0, 0.0, 0.0))
        .with(BELLY, v(5.0, 0.0, 0.0))
        .with(HEAD, v(-25.0, 0.0, 0.0))
        .with(R_UPPER, v(-45.0, 0.0, -80.0))
        .with(R_HAND, v(0.0, 10.0, 0.0));
    let end = mid
        .with(CHEST, v(15.0, 60.0, 0.0))
        .with(BELLY, v(5.0, 20.0, 0.0))
        .with(HEAD, v(-25.0, -40.0, 0.0))
        .with(R_UPPER, v(-65.0, 0.0, -45.0))
        .with(R_HAND, v(0.0, 40.0, 0.0));
    vec![(0.0, Linear, b), (0.38, Out, wind), (0.48, In, mid), (0.56, Linear, end), (0.8, Out, end), (1.0, Smooth, b)]
}

fn leap() -> Vec<(f32, Ease, DPose)> {
    let b = DPose::base();
    let crouch = b.legs(-45.0, 75.0).with(CHEST, v(40.0, 0.0, 0.0)).wings(v(0.0, 0.0, 55.0)).hips(v(0.0, -0.6, 0.0));
    let takeoff = club_overhead(b)
        .legs(5.0, 10.0)
        .with(L_FOOT, v(30.0, 0.0, 0.0))
        .with(R_FOOT, v(30.0, 0.0, 0.0))
        .with(CHEST, v(10.0, 0.0, 0.0))
        .wings(v(0.0, 0.0, -35.0))
        .hips(v(0.0, 0.6, 0.0));
    let apex = takeoff.legs(-40.0, 60.0).wings(v(0.0, 0.0, 60.0)).with(JAW, v(30.0, 0.0, 0.0)).hips(v(0.0, 2.8, 0.0));
    let fall = apex.with(R_UPPER, v(-110.0, 0.0, -10.0)).wings(v(0.0, 0.0, -30.0)).hips(v(0.0, 1.0, 0.0));
    let land = club_impact(b)
        .legs(-40.0, 70.0)
        .with(CHEST, v(50.0, 0.0, 0.0))
        .with(HEAD, v(-45.0, 0.0, 0.0))
        .wings(v(0.0, 30.0, 30.0))
        .hips(v(0.0, -0.55, 0.3));
    vec![(0.0, Linear, b), (0.18, Out, crouch), (0.28, In, takeoff), (0.45, Out, apex), (0.56, In, fall), (0.62, In, land), (0.8, Linear, land), (1.0, Smooth, b)]
}

fn pound() -> Vec<(f32, Ease, DPose)> {
    let b = DPose::base();
    let crouch = b
        .legs(-45.0, 80.0)
        .with(CHEST, v(45.0, 0.0, 0.0))
        .with(L_UPPER, v(-10.0, 0.0, 60.0))
        .with(R_UPPER, v(-10.0, 0.0, -60.0))
        .hips(v(0.0, -0.65, 0.0));
    let up = b
        .legs(0.0, 5.0)
        .with(L_UPPER, v(-120.0, 0.0, 40.0))
        .with(R_UPPER, v(-120.0, 0.0, -40.0))
        .with(R_HAND, v(-20.0, 0.0, 0.0))
        .with(CHEST, v(0.0, 0.0, 0.0))
        .with(HEAD, v(-50.0, 0.0, 0.0))
        .with(JAW, v(35.0, 0.0, 0.0))
        .wings(v(0.0, 0.0, 60.0))
        .hips(v(0.0, 1.6, 0.0));
    let slam = crouch
        .with(L_UPPER, v(-30.0, 0.0, 70.0))
        .with(R_UPPER, v(-30.0, 0.0, -70.0))
        .with(CHEST, v(40.0, 0.0, 0.0))
        .with(JAW, v(30.0, 0.0, 0.0))
        .wings(v(0.0, 20.0, 40.0))
        .hips(v(0.0, -0.8, 0.0));
    vec![(0.0, Linear, b), (0.25, Out, crouch), (0.40, Out, up), (0.52, In, slam), (0.75, Linear, slam), (1.0, Smooth, b)]
}

fn roar() -> Vec<(f32, Ease, DPose)> {
    let b = DPose::base();
    let r = b
        .with(HEAD, v(-60.0, 0.0, 0.0))
        .with(JAW, v(40.0, 0.0, 0.0))
        .with(CHEST, v(-5.0, 0.0, 0.0))
        .with(L_UPPER, v(-30.0, 0.0, 70.0))
        .with(R_UPPER, v(-30.0, 0.0, -70.0))
        .with(R_HAND, v(20.0, 0.0, 0.0))
        .wings(v(0.0, -10.0, 45.0))
        .hips(v(0.0, 0.05, -0.1));
    let r2 = r.with(HEAD, v(-55.0, 8.0, 5.0)).with(JAW, v(35.0, 0.0, 0.0));
    vec![(0.0, Linear, b), (0.2, Out, r), (0.8, Linear, r2), (1.0, Smooth, b)]
}

fn fall_pose() -> DPose {
    club_overhead(DPose::base()).legs(-20.0, 30.0).wings(v(0.0, -10.0, 60.0)).with(L_UPPER, v(-60.0, 0.0, 60.0))
}

fn land() -> Vec<(f32, Ease, DPose)> {
    let b = DPose::base();
    let c = b.legs(-50.0, 85.0).with(CHEST, v(50.0, 0.0, 0.0)).with(HEAD, v(-45.0, 0.0, 0.0)).wings(v(0.0, 20.0, 40.0)).hips(v(0.0, -0.8, 0.0));
    vec![(0.0, Linear, fall_pose()), (0.12, In, c), (0.45, Linear, c), (1.0, Smooth, b)]
}

fn stagger() -> Vec<(f32, Ease, DPose)> {
    let b = DPose::base();
    let reel = b
        .with(CHEST, v(-15.0, 10.0, 0.0))
        .with(HEAD, v(-50.0, -10.0, 10.0))
        .with(JAW, v(35.0, 0.0, 0.0))
        .with(L_UPPER, v(-50.0, 0.0, 70.0))
        .with(R_UPPER, v(-30.0, 0.0, -60.0))
        .wings(v(0.0, 0.0, 50.0))
        .hips(v(0.0, -0.1, -0.4));
    let knee = reel
        .with(L_THIGH, v(-80.0, 0.0, 10.0))
        .with(L_SHIN, v(110.0, 0.0, 0.0))
        .with(R_THIGH, v(15.0, 0.0, -10.0))
        .with(R_SHIN, v(95.0, 0.0, 0.0))
        .with(CHEST, v(40.0, 0.0, 0.0))
        .with(HEAD, v(-20.0, 0.0, 0.0))
        .with(JAW, v(20.0, 0.0, 0.0))
        .with(L_UPPER, v(-30.0, 0.0, 20.0))
        .wings(v(0.0, 50.0, 10.0))
        .hips(v(0.0, -0.95, 0.0));
    vec![(0.0, Linear, b), (0.12, Out, reel), (0.35, In, knee), (0.8, Linear, knee), (1.0, Smooth, b)]
}

fn death() -> Vec<(f32, Ease, DPose)> {
    let b = DPose::base();
    let howl = roar()[1].2.hips(v(0.0, -0.1, 0.0));
    let kneel = stagger()[2].2;
    let flat = kneel
        .with(PELVIS, v(35.0, 0.0, 0.0))
        .with(CHEST, v(55.0, 0.0, 0.0))
        .with(HEAD, v(10.0, 0.0, 30.0))
        .with(JAW, v(25.0, 0.0, 0.0))
        .with(L_UPPER, v(-60.0, 0.0, 80.0))
        .with(R_UPPER, v(-60.0, 0.0, -80.0))
        .with(L_THIGH, v(-50.0, 0.0, 10.0))
        .wings(v(0.0, 20.0, 5.0))
        .hips(v(0.0, -1.5, 0.3));
    vec![(0.0, Linear, b), (0.2, Out, howl), (0.45, Smooth, kneel), (0.7, In, flat), (1.0, Linear, flat)]
}

pub fn register(lib: &mut ClipLibrary, _skel: &Skeleton) {
    use clip::*;
    lib.add(clip(IDLE, 3.0, true, idle()));
    lib.add(clip(WALK, WALK_DUR, true, walk()));
    lib.add(clip(SMASH, SMASH_DUR, false, smash()));
    lib.add(clip(SWEEP, SWEEP_DUR, false, sweep()));
    lib.add(clip(LEAP, LEAP_DUR, false, leap()));
    lib.add(clip(POUND, POUND_DUR, false, pound()));
    lib.add(clip(ROAR, ROAR_DUR, false, roar()));
    lib.add(clip(FALL, 1.0, true, vec![(0.0, Linear, fall_pose())]));
    lib.add(clip(LAND, LAND_DUR, false, land()));
    lib.add(clip(STAGGER, STAGGER_DUR, false, stagger()));
    lib.add(clip(DEATH, DEATH_DUR, false, death()));
}
