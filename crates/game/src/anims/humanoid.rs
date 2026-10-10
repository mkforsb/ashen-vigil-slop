//! Humanoid animation clips, authored as IK key poses.
//!
//! Body frame: +X is the character's left, +Y up, +Z forward; the right
//! shoulder sits around (-0.21, 1.50, 0). Arms reach about 0.55 m.

use super::{Ease, KeyClip, ProcClip};
use crate::rig::humanoid::{Foot, HPose, Hand, v3};
use ashen_engine::anim::{AnimParams, ClipLibrary};
use ashen_engine::math::{Vec3, smoothstep};
use ashen_engine::skeleton::Skeleton;
use std::f32::consts::TAU;

use Ease::*;

/// Clip names and durations shared with combat data.
pub mod clip {
    pub const IDLE: &str = "h.idle";
    pub const LOCO: &str = "h.loco";
    pub const BLOCK: &str = "h.block";
    pub const LIGHT1: &str = "h.light1";
    pub const LIGHT2: &str = "h.light2";
    pub const HEAVY1: &str = "h.heavy1";
    pub const HEAVY2: &str = "h.heavy2";
    pub const RUN_ATTACK: &str = "h.run_attack";
    pub const ROLL_ATTACK: &str = "h.roll_attack";
    pub const PLUNGE_FALL: &str = "h.plunge_fall";
    pub const PLUNGE_LAND: &str = "h.plunge_land";
    pub const ROLL: &str = "h.roll";
    pub const BACKSTEP: &str = "h.backstep";
    pub const HIT_LIGHT: &str = "h.hit_light";
    pub const HIT_HEAVY: &str = "h.hit_heavy";
    pub const KNOCKDOWN: &str = "h.knockdown";
    pub const DEATH: &str = "h.death";
    pub const GUARD_HIT: &str = "h.guard_hit";
    pub const GUARD_BREAK: &str = "h.guard_break";
    pub const DRINK: &str = "h.drink";
    pub const PICKUP: &str = "h.pickup";
    pub const PUSH: &str = "h.push";
    pub const SIT: &str = "h.sit";
    pub const SIT_LOOP: &str = "h.sit_loop";
    pub const STAND_UP: &str = "h.stand_up";
    pub const FALL: &str = "h.fall";
    pub const LAND: &str = "h.land";
    pub const LAND_HARD: &str = "h.land_hard";
    pub const JUMP: &str = "h.jump";
    pub const HUSK_IDLE: &str = "husk.idle";
    pub const HUSK_LOCO: &str = "husk.loco";
    pub const HUSK_ATTACK1: &str = "husk.attack1";
    pub const HUSK_ATTACK2: &str = "husk.attack2";
    pub const HUSK_CROUCH: &str = "husk.crouch";
    pub const HUSK_PUSH: &str = "husk.push";
    pub const ARCHER_AIM: &str = "archer.aim";
    pub const ARCHER_SHOOT: &str = "archer.shoot";
    pub const NPC_SLUMP: &str = "npc.slump";
    pub const NPC_TALK: &str = "npc.talk";

    pub const LIGHT_DUR: f32 = 0.9;
    pub const HEAVY1_DUR: f32 = 1.3;
    pub const HEAVY2_DUR: f32 = 1.2;
    pub const RUN_ATTACK_DUR: f32 = 1.15;
    pub const ROLL_ATTACK_DUR: f32 = 0.9;
    pub const PLUNGE_LAND_DUR: f32 = 1.3;
    pub const ROLL_DUR: f32 = 0.8;
    pub const BACKSTEP_DUR: f32 = 0.6;
    pub const HIT_LIGHT_DUR: f32 = 0.45;
    pub const HIT_HEAVY_DUR: f32 = 0.9;
    pub const KNOCKDOWN_DUR: f32 = 2.6;
    pub const DEATH_DUR: f32 = 2.0;
    pub const GUARD_HIT_DUR: f32 = 0.4;
    pub const GUARD_BREAK_DUR: f32 = 1.2;
    pub const DRINK_DUR: f32 = 1.6;
    pub const PICKUP_DUR: f32 = 1.0;
    pub const PUSH_DUR: f32 = 1.4;
    pub const SIT_DUR: f32 = 1.4;
    pub const STAND_UP_DUR: f32 = 1.1;
    pub const LAND_DUR: f32 = 0.35;
    pub const LAND_HARD_DUR: f32 = 1.1;
    pub const JUMP_DUR: f32 = 0.5;
    pub const HUSK_ATTACK1_DUR: f32 = 1.5;
    pub const HUSK_ATTACK2_DUR: f32 = 1.3;
    pub const HUSK_PUSH_DUR: f32 = 1.6;
    pub const ARCHER_SHOOT_DUR: f32 = 0.6;
}

const Y: Vec3 = Vec3::Y;

/// Sword-and-shield combat stance.
pub fn ready() -> HPose {
    HPose {
        hips: v3(0.0, -0.04, 0.0),
        spine: v3(6.0, 0.0, 0.0),
        head: v3(-4.0, 0.0, 0.0),
        r_hand: Hand::new(v3(-0.26, 1.04, 0.24), v3(-0.4, -0.3, -1.0), v3(0.08, -0.15, 1.0)),
        l_hand: Hand::new(v3(0.22, 1.12, 0.26), v3(0.6, -0.4, -1.0), v3(0.25, 0.0, 1.0)).up(Y),
        r_foot: Foot { yaw: -12.0, ..Foot::new(v3(-0.14, 0.05, -0.08), v3(-0.15, 0.0, 1.0)) },
        l_foot: Foot { yaw: 10.0, ..Foot::new(v3(0.13, 0.05, 0.12), v3(0.15, 0.0, 1.0)) },
        ..HPose::stand()
    }
}

/// Shield raised in front of the chest.
pub fn block_pose() -> HPose {
    HPose { l_hand: Hand::new(v3(0.12, 1.32, 0.34), v3(0.8, -0.5, -0.3), v3(0.0, 0.0, 1.0)).up(Y), ..ready() }
}

fn with_r(p: HPose, pos: Vec3, grip: Vec3, elbow: Vec3) -> HPose {
    HPose { r_hand: Hand::new(pos, elbow, grip), ..p }
}

fn feet(p: HPose, l: Vec3, r: Vec3) -> HPose {
    HPose { l_foot: Foot { pos: l, ..p.l_foot }, r_foot: Foot { pos: r, ..p.r_foot }, ..p }
}

fn light1() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let wind = HPose {
        chest: v3(0.0, -35.0, 0.0),
        spine: v3(4.0, -12.0, 0.0),
        hips: v3(0.0, -0.06, 0.0),
        l_hand: Hand::new(v3(0.18, 1.18, 0.30), v3(0.6, -0.4, -1.0), v3(0.2, 0.0, 1.0)).up(Y),
        ..feet(
            with_r(base, v3(-0.42, 1.40, -0.02), v3(-0.5, 0.35, -0.8), v3(-0.3, -1.0, -0.4)),
            v3(0.13, 0.05, 0.22),
            v3(-0.15, 0.05, -0.12),
        )
    };
    let s1 = HPose { chest: v3(0.0, -10.0, 0.0), ..with_r(wind, v3(-0.40, 1.32, 0.38), v3(-0.6, 0.05, 0.8), v3(-0.5, -1.0, -0.2)) };
    let s2 = HPose {
        chest: v3(0.0, 15.0, 0.0),
        spine: v3(4.0, 0.0, 0.0),
        ..with_r(s1, v3(0.0, 1.28, 0.52), v3(0.55, -0.05, 0.84), v3(-0.5, -1.0, -0.2))
    };
    let s3 = HPose {
        chest: v3(0.0, 35.0, 0.0),
        spine: v3(4.0, 10.0, 0.0),
        ..with_r(s2, v3(0.28, 1.24, 0.28), v3(0.9, -0.1, -0.2), v3(-0.2, -1.0, -0.2))
    };
    let rec = HPose { chest: v3(0.0, 18.0, 0.0), spine: v3(5.0, 4.0, 0.0), ..with_r(s3, v3(0.05, 1.12, 0.32), v3(0.6, -0.2, 0.8), v3(-0.4, -1.0, -0.4)) };
    vec![(0.0, Linear, base), (0.28, Out, wind), (0.36, In, s1), (0.42, Linear, s2), (0.50, Out, s3), (0.75, Smooth, rec), (1.0, Smooth, base)]
}

fn light2() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let start = light1()[4].2;
    let wind = HPose {
        chest: v3(0.0, 40.0, 0.0),
        spine: v3(4.0, 12.0, 0.0),
        ..feet(
            with_r(start, v3(0.18, 1.42, 0.05), v3(0.5, 0.4, -0.75), v3(-0.6, -1.0, 0.0)),
            v3(0.13, 0.05, 0.18),
            v3(-0.15, 0.05, -0.14),
        )
    };
    let s1 = HPose { chest: v3(0.0, 15.0, 0.0), ..with_r(wind, v3(0.15, 1.32, 0.45), v3(0.6, 0.05, 0.8), v3(-0.6, -1.0, -0.2)) };
    let s2 = HPose { chest: v3(0.0, -15.0, 0.0), spine: v3(4.0, 0.0, 0.0), ..with_r(s1, v3(-0.2, 1.28, 0.52), v3(-0.55, -0.05, 0.84), v3(-0.5, -1.0, -0.2)) };
    let s3 = HPose {
        chest: v3(0.0, -35.0, 0.0),
        spine: v3(4.0, -10.0, 0.0),
        ..with_r(s2, v3(-0.48, 1.20, 0.18), v3(-0.9, -0.1, -0.2), v3(-0.3, -1.0, -0.3))
    };
    let rec = HPose { chest: v3(0.0, -15.0, 0.0), ..with_r(s3, v3(-0.35, 1.10, 0.25), v3(-0.4, -0.2, 0.9), v3(-0.4, -1.0, -0.4)) };
    vec![(0.0, Linear, start), (0.26, Out, wind), (0.34, In, s1), (0.40, Linear, s2), (0.48, Out, s3), (0.75, Smooth, rec), (1.0, Smooth, base)]
}

fn heavy1() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let wind = HPose {
        spine: v3(-6.0, 0.0, 0.0),
        chest: v3(-12.0, -15.0, 0.0),
        hips: v3(0.0, -0.02, -0.05),
        l_hand: Hand::new(v3(0.25, 1.20, 0.25), v3(0.6, -0.4, -1.0), v3(0.3, 0.0, 1.0)).up(Y),
        ..feet(with_r(base, v3(-0.18, 1.88, -0.10), v3(0.1, 0.55, -0.83), v3(-1.0, 0.3, 0.0)), v3(0.13, 0.05, 0.15), v3(-0.14, 0.05, -0.12))
    };
    let s1 = HPose { chest: v3(10.0, -5.0, 0.0), spine: v3(5.0, 0.0, 0.0), ..with_r(wind, v3(-0.12, 1.75, 0.42), v3(0.0, 0.7, 0.7), v3(-1.0, 0.0, -0.3)) };
    let s2 = HPose {
        chest: v3(25.0, 0.0, 0.0),
        spine: v3(12.0, 0.0, 0.0),
        hips: v3(0.0, -0.16, 0.12),
        ..feet(with_r(s1, v3(-0.08, 1.05, 0.60), v3(0.0, -0.55, 0.83), v3(-1.0, 0.0, -0.3)), v3(0.14, 0.05, 0.42), v3(-0.15, 0.05, -0.2))
    };
    let s3 = HPose { chest: v3(28.0, 0.0, 0.0), ..with_r(s2, v3(-0.08, 0.80, 0.62), v3(0.0, -0.8, 0.6), v3(-1.0, 0.0, -0.3)) };
    let rec = HPose {
        chest: v3(12.0, 0.0, 0.0),
        spine: v3(8.0, 0.0, 0.0),
        hips: v3(0.0, -0.10, 0.05),
        ..with_r(s3, v3(-0.15, 1.00, 0.45), v3(0.0, -0.4, 0.9), v3(-0.6, -0.5, -0.6))
    };
    vec![(0.0, Linear, base), (0.34, Out, wind), (0.43, In, s1), (0.49, Linear, s2), (0.56, Out, s3), (0.8, Smooth, rec), (1.0, Smooth, base)]
}

fn heavy2() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let wind = HPose {
        chest: v3(0.0, -30.0, 0.0),
        hips: v3(0.0, -0.06, -0.08),
        r_hand: Hand::new(v3(-0.30, 1.20, -0.12), v3(-0.5, -0.5, -1.0), v3(0.15, 0.05, 1.0)).up(Y),
        ..base
    };
    let s1 = HPose {
        chest: v3(8.0, 10.0, 0.0),
        hips: v3(0.0, -0.12, 0.15),
        r_hand: Hand::new(v3(-0.08, 1.32, 0.68), v3(-0.6, -0.6, -0.4), v3(0.05, 0.0, 1.0)).up(Y),
        ..feet(wind, v3(0.14, 0.05, 0.45), v3(-0.15, 0.05, -0.3))
    };
    let s2 = HPose { r_hand: Hand { pos: v3(-0.08, 1.30, 0.66), ..s1.r_hand }, ..s1 };
    let rec = base.lerp(&s2, 0.4);
    vec![(0.0, Linear, base), (0.32, Out, wind), (0.42, In, s1), (0.52, Linear, s2), (0.8, Smooth, rec), (1.0, Smooth, base)]
}

fn run_attack() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let start = HPose { spine: v3(15.0, 0.0, 0.0), ..with_r(base, v3(-0.32, 1.10, -0.05), v3(-0.2, -0.4, -0.9), v3(-0.4, -0.3, -1.0)) };
    let leap = HPose {
        body: v3(-5.0, 0.0, 0.0),
        hips: v3(0.0, 0.05, 0.0),
        spine: v3(-4.0, 0.0, 0.0),
        chest: v3(-10.0, -10.0, 0.0),
        ..feet(with_r(start, v3(-0.15, 1.90, -0.05), v3(0.1, 0.6, -0.8), v3(-1.0, 0.3, 0.0)), v3(0.13, 0.30, 0.15), v3(-0.13, 0.25, -0.2))
    };
    let strike = HPose {
        body: Vec3::ZERO,
        hips: v3(0.0, -0.15, 0.10),
        chest: v3(25.0, 0.0, 0.0),
        spine: v3(12.0, 0.0, 0.0),
        ..feet(with_r(leap, v3(-0.08, 0.95, 0.62), v3(0.0, -0.65, 0.75), v3(-1.0, 0.0, -0.3)), v3(0.14, 0.05, 0.40), v3(-0.15, 0.05, -0.3))
    };
    vec![(0.0, Linear, start), (0.2, Out, leap), (0.36, In, strike), (0.6, Out, strike), (1.0, Smooth, base)]
}

fn roll_attack() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let crouch = HPose { hips: v3(0.0, -0.30, 0.0), spine: v3(25.0, 0.0, 0.0), ..with_r(base, v3(-0.25, 0.80, 0.30), v3(0.0, -0.2, 1.0), v3(-0.5, 0.0, -1.0)) };
    let strike = HPose {
        hips: v3(0.0, -0.10, 0.10),
        spine: v3(5.0, 0.0, 0.0),
        chest: v3(5.0, 15.0, 0.0),
        r_hand: Hand::new(v3(-0.05, 1.35, 0.62), v3(-0.6, -0.6, -0.4), v3(0.1, 0.25, 1.0)).up(Y),
        ..feet(crouch, v3(0.14, 0.05, 0.35), v3(-0.15, 0.05, -0.25))
    };
    vec![(0.0, Linear, crouch), (0.3, In, strike), (0.55, Out, strike), (1.0, Smooth, base)]
}

fn plunge_fall() -> HPose {
    HPose {
        r_hand: Hand::new(v3(-0.08, 1.45, 0.30), v3(-1.0, 0.0, -0.3), v3(0.0, -1.0, 0.15)),
        l_hand: Hand::new(v3(0.30, 1.25, 0.15), v3(0.6, 0.0, -1.0), v3(0.6, 0.0, 0.8)).up(Y),
        ..feet(ready(), v3(0.13, 0.25, 0.10), v3(-0.13, 0.15, -0.10))
    }
}

fn plunge_land() -> Vec<(f32, Ease, HPose)> {
    let fall = plunge_fall();
    let land = HPose {
        hips: v3(0.0, -0.45, 0.05),
        spine: v3(35.0, 0.0, 0.0),
        chest: v3(15.0, 0.0, 0.0),
        r_hand: Hand::new(v3(-0.05, 0.45, 0.45), v3(-1.0, 0.0, -0.3), v3(0.0, -1.0, 0.1)),
        ..feet(fall, v3(0.14, 0.05, 0.25), v3(-0.14, 0.05, -0.15))
    };
    vec![(0.0, Linear, fall), (0.1, In, land), (0.55, Linear, land), (1.0, Smooth, ready())]
}

fn tuck(body_x: f32, hips_y: f32) -> HPose {
    HPose {
        body: v3(body_x, 0.0, 0.0),
        hips: v3(0.0, hips_y, 0.0),
        spine: v3(40.0, 0.0, 0.0),
        chest: v3(30.0, 0.0, 0.0),
        head: v3(20.0, 0.0, 0.0),
        r_hand: Hand::new(v3(-0.18, 0.95, 0.40), v3(-0.6, 0.3, -1.0), v3(0.0, 0.2, 1.0)),
        l_hand: Hand::new(v3(0.18, 1.00, 0.40), v3(0.6, 0.3, -1.0), v3(0.0, 0.0, 1.0)).up(Y),
        r_foot: Foot::new(v3(-0.12, 0.45 + hips_y + 0.4, 0.12), v3(0.0, 1.0, 1.0)),
        l_foot: Foot::new(v3(0.12, 0.45 + hips_y + 0.4, 0.18), v3(0.0, 1.0, 1.0)),
        ..ready()
    }
}

fn roll() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let crouch = HPose { hips: v3(0.0, -0.15, 0.0), spine: v3(20.0, 0.0, 0.0), ..base };
    let land = HPose {
        hips: v3(0.0, -0.30, 0.0),
        spine: v3(25.0, 0.0, 0.0),
        ..feet(base, v3(0.13, 0.05, 0.20), v3(-0.13, 0.05, -0.15))
    };
    vec![
        (0.0, Linear, crouch),
        (0.10, In, tuck(45.0, -0.40)),
        (0.25, Linear, tuck(135.0, -0.20)),
        (0.40, Linear, tuck(225.0, -0.20)),
        (0.55, Linear, tuck(315.0, -0.36)),
        (0.68, Out, land),
        (1.0, Smooth, base),
    ]
}

fn backstep() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let hop = HPose { hips: v3(0.0, 0.02, 0.0), body: v3(-8.0, 0.0, 0.0), ..feet(base, v3(0.13, 0.12, 0.05), v3(-0.13, 0.20, -0.15)) };
    let plant = HPose { hips: v3(0.0, -0.12, 0.0), body: v3(-4.0, 0.0, 0.0), ..feet(base, v3(0.13, 0.05, 0.05), v3(-0.14, 0.05, -0.2)) };
    vec![(0.0, Linear, base), (0.15, Out, hop), (0.35, In, plant), (1.0, Smooth, base)]
}

fn hit_light() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let hit = HPose {
        chest: v3(-14.0, 12.0, 0.0),
        head: v3(-20.0, 0.0, 8.0),
        spine: v3(-6.0, 0.0, 0.0),
        hips: v3(0.0, -0.05, -0.05),
        r_hand: Hand { pos: v3(-0.35, 1.15, 0.10), ..base.r_hand },
        l_hand: Hand { pos: v3(0.30, 1.25, 0.15), ..base.l_hand },
        ..base
    };
    vec![(0.0, Linear, base), (0.15, Out, hit), (1.0, Smooth, base)]
}

fn hit_heavy() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let hit = HPose {
        body: v3(-12.0, 0.0, 0.0),
        chest: v3(-15.0, 0.0, 0.0),
        head: v3(-25.0, 0.0, 0.0),
        hips: v3(0.0, -0.10, -0.10),
        r_hand: Hand { pos: v3(-0.45, 1.25, 0.0), ..base.r_hand },
        l_hand: Hand { pos: v3(0.45, 1.30, 0.0), ..base.l_hand },
        ..feet(base, v3(0.13, 0.05, 0.05), v3(-0.15, 0.05, -0.35))
    };
    let mid = base.lerp(&hit, 0.5);
    vec![(0.0, Linear, base), (0.2, Out, hit), (0.6, Smooth, mid), (1.0, Smooth, base)]
}

fn knockdown() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let flung = HPose {
        body: v3(-35.0, 0.0, 0.0),
        hips: v3(0.0, -0.10, -0.15),
        head: v3(-20.0, 0.0, 0.0),
        r_hand: Hand { pos: v3(-0.45, 1.35, 0.05), ..base.r_hand },
        l_hand: Hand { pos: v3(0.45, 1.40, 0.05), ..base.l_hand },
        ..base
    };
    let lying = HPose {
        body: v3(-85.0, 0.0, 0.0),
        hips: v3(0.0, -0.78, -0.40),
        head: v3(-10.0, 0.0, 0.0),
        spine: Vec3::ZERO,
        r_hand: Hand { pos: v3(-0.50, 1.25, 0.10), ..base.r_hand },
        l_hand: Hand { pos: v3(0.50, 1.25, 0.10), ..base.l_hand },
        ..feet(base, v3(0.16, 0.05, 0.05), v3(-0.15, 0.05, 0.0))
    };
    let sit_up = HPose {
        body: v3(-40.0, 0.0, 0.0),
        hips: v3(0.0, -0.72, -0.25),
        spine: v3(30.0, 0.0, 0.0),
        r_hand: Hand { pos: v3(-0.35, 0.95, -0.05), ..base.r_hand },
        ..feet(base, v3(0.16, 0.25, 0.30), v3(-0.15, 0.25, 0.25))
    };
    let crouch = HPose {
        hips: v3(0.0, -0.40, 0.0),
        spine: v3(35.0, 0.0, 0.0),
        r_hand: Hand { pos: v3(-0.30, 0.55, 0.30), ..base.r_hand },
        ..feet(base, v3(0.13, 0.05, 0.25), v3(-0.13, 0.05, -0.10))
    };
    vec![
        (0.0, Linear, base),
        (0.10, Out, flung),
        (0.20, In, lying),
        (0.55, Linear, lying),
        (0.70, Smooth, sit_up),
        (0.85, Smooth, crouch),
        (1.0, Smooth, base),
    ]
}

fn death() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let kneel = HPose {
        hips: v3(0.0, -0.45, 0.0),
        spine: v3(20.0, 0.0, 0.0),
        head: v3(25.0, 0.0, 0.0),
        r_hand: Hand::new(v3(-0.26, 0.65, 0.25), v3(-0.5, 0.0, -1.0), v3(0.0, -1.0, 0.3)),
        l_hand: Hand::new(v3(0.25, 0.70, 0.20), v3(0.5, 0.0, -1.0), v3(0.3, -0.3, 1.0)).up(Y),
        r_foot: Foot { pitch: 60.0, ..Foot::new(v3(-0.13, 0.08, -0.38), v3(0.0, -0.3, 1.0)) },
        l_foot: Foot { pitch: 60.0, ..Foot::new(v3(0.13, 0.08, -0.38), v3(0.0, -0.3, 1.0)) },
        ..base
    };
    let face_down = HPose {
        body: v3(80.0, 0.0, 0.0),
        hips: v3(0.0, -0.75, 0.35),
        spine: v3(0.0, 0.0, 0.0),
        head: v3(-10.0, 30.0, 0.0),
        r_hand: Hand::new(v3(-0.40, 1.10, 0.25), v3(-0.5, 1.0, 0.0), v3(-0.3, 0.0, 1.0)),
        l_hand: Hand::new(v3(0.40, 1.20, 0.20), v3(0.5, 1.0, 0.0), v3(0.3, 0.0, 1.0)).up(Y),
        r_foot: Foot::new(v3(-0.15, 0.05, 0.0), v3(0.0, 0.0, 1.0)),
        l_foot: Foot::new(v3(0.15, 0.05, 0.05), v3(0.0, 0.0, 1.0)),
        ..base
    };
    vec![(0.0, Linear, base), (0.25, Out, kneel), (0.55, In, face_down), (1.0, Linear, face_down)]
}

fn guard_hit() -> Vec<(f32, Ease, HPose)> {
    let b = block_pose();
    let hit = HPose {
        chest: v3(-8.0, 8.0, 0.0),
        hips: v3(0.0, -0.06, -0.06),
        l_hand: Hand { pos: v3(0.12, 1.30, 0.22), ..b.l_hand },
        ..b
    };
    vec![(0.0, Linear, b), (0.12, Out, hit), (1.0, Smooth, b)]
}

fn guard_break() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let broken = HPose {
        body: v3(-12.0, 0.0, 0.0),
        chest: v3(-10.0, 20.0, 0.0),
        head: v3(-15.0, 0.0, 0.0),
        l_hand: Hand::new(v3(0.50, 1.35, -0.05), v3(0.0, -1.0, -0.5), v3(1.0, 0.3, 0.0)).up(Y),
        r_hand: Hand { pos: v3(-0.40, 1.10, 0.0), ..base.r_hand },
        ..feet(base, v3(0.13, 0.05, 0.05), v3(-0.15, 0.05, -0.30))
    };
    vec![(0.0, Linear, block_pose()), (0.15, Out, broken), (0.7, Smooth, broken.lerp(&base, 0.3)), (1.0, Smooth, base)]
}

fn drink() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let sip = HPose {
        head: v3(-35.0, 0.0, 0.0),
        chest: v3(-6.0, 0.0, 0.0),
        r_hand: Hand::new(v3(-0.10, 1.58, 0.20), v3(-1.0, -0.3, 0.0), v3(0.3, 0.7, -0.6)),
        ..base
    };
    let sip2 = HPose { head: v3(-40.0, 0.0, 0.0), r_hand: Hand { pos: v3(-0.09, 1.60, 0.17), ..sip.r_hand }, ..sip };
    vec![(0.0, Linear, base), (0.25, Smooth, sip), (0.6, Linear, sip2), (0.78, Smooth, base.lerp(&sip, 0.2)), (1.0, Smooth, base)]
}

fn pickup() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let low = HPose {
        hips: v3(0.0, -0.45, 0.0),
        spine: v3(45.0, 0.0, 0.0),
        chest: v3(10.0, 0.0, 0.0),
        r_hand: Hand::new(v3(-0.20, 0.25, 0.45), v3(-0.5, 0.0, -1.0), v3(0.0, -0.3, 1.0)),
        ..feet(base, v3(0.14, 0.05, 0.25), v3(-0.14, 0.05, -0.15))
    };
    vec![(0.0, Linear, base), (0.35, Out, low), (0.55, Linear, low), (1.0, Smooth, base)]
}

fn push() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let p = HPose {
        spine: v3(20.0, 0.0, 0.0),
        hips: v3(0.0, -0.10, 0.05),
        r_hand: Hand::new(v3(-0.15, 1.35, 0.50), v3(-1.0, -0.5, -0.2), v3(0.0, 0.6, -0.8)),
        l_hand: Hand::new(v3(0.15, 1.35, 0.50), v3(1.0, -0.5, -0.2), v3(0.5, 0.0, 0.8)).up(Y),
        ..feet(base, v3(0.14, 0.05, 0.30), v3(-0.14, 0.05, -0.35))
    };
    let p2 = HPose { hips: v3(0.0, -0.12, 0.12), ..p };
    vec![(0.0, Linear, base), (0.25, Smooth, p), (0.75, Linear, p2), (1.0, Smooth, base)]
}

fn sitting() -> HPose {
    HPose {
        hips: v3(0.0, -0.70, -0.10),
        spine: v3(15.0, 0.0, 0.0),
        head: v3(15.0, 0.0, 0.0),
        r_hand: Hand::new(v3(-0.18, 0.65, 0.40), v3(-0.5, 0.0, -1.0), v3(0.2, -0.3, 1.0)),
        l_hand: Hand::new(v3(0.18, 0.65, 0.40), v3(0.5, 0.0, -1.0), v3(0.2, 0.0, 1.0)).up(Y),
        r_foot: Foot::new(v3(-0.10, 0.05, 0.40), v3(0.0, 1.0, 1.0)),
        l_foot: Foot::new(v3(0.15, 0.05, 0.45), v3(0.0, 1.0, 1.0)),
        ..ready()
    }
}

fn fall_pose() -> HPose {
    HPose {
        spine: v3(-5.0, 0.0, 0.0),
        r_hand: Hand { pos: v3(-0.40, 1.45, 0.10), ..ready().r_hand },
        l_hand: Hand { pos: v3(0.40, 1.50, 0.10), ..ready().l_hand },
        ..feet(ready(), v3(0.13, 0.20, 0.12), v3(-0.13, 0.12, -0.12))
    }
}

/// Stride half-length for the knight's locomotion at a given speed.
pub fn knight_stride(speed: f32) -> f32 {
    let run = smoothstep(1.8, 4.0, speed);
    let sprint = smoothstep(4.5, 6.0, speed);
    0.35 + run * 0.30 + sprint * 0.2
}

/// Distance travelled per locomotion cycle (two steps).
pub fn cycle_len(speed: f32) -> f32 {
    4.0 * knight_stride(speed)
}

/// Locomotion style knobs shared by the knight and husks.
#[derive(Clone, Copy)]
struct LocoStyle {
    hunch: f32,
    stride_scale: f32,
    arm_swing: f32,
    limp: f32,
    sword_and_shield: bool,
}

fn locomotion(phase: f32, p: &AnimParams, st: LocoStyle) -> HPose {
    let speed = p.speed;
    let run = smoothstep(1.8, 4.0, speed);
    let sprint = smoothstep(4.5, 6.0, speed);
    let moving = smoothstep(0.05, 0.6, speed);
    let stride = knight_stride(speed) * st.stride_scale * moving;
    let lift = (0.08 + run * 0.08 + sprint * 0.06) * moving;
    let dir = if p.move_dir.length_squared() > 1e-4 { p.move_dir.normalize() } else { glam::Vec2::Y };
    let d3 = v3(dir.x, 0.0, dir.y);
    let a = phase * TAU;
    let foot = |side: f32, ph: f32| {
        let fwd = ph.sin() * stride;
        let up = ph.cos().max(0.0) * lift;
        let limp = if side < 0.0 { st.limp } else { 0.0 };
        Foot {
            pos: v3(side * 0.13, 0.05 + up * (1.0 - limp), 0.0) + d3 * fwd,
            knee: v3(side * 0.1, 0.0, 1.0),
            yaw: side * 6.0,
            pitch: -ph.cos() * 15.0 * moving,
        }
    };
    let base = if st.sword_and_shield { ready() } else { HPose::stand() };
    let fwd_amt = dir.y.max(0.0);
    let swing = a.sin() * st.arm_swing * (0.4 + run) * moving * fwd_amt;
    let r_pos = v3(-0.27, 1.02, 0.18 + swing * 0.15).lerp(v3(-0.33, 1.10, -0.10), sprint);
    let r_grip = v3(0.05, -0.35, 1.0).lerp(v3(-0.2, -0.4, -0.9), sprint);
    let l_pos = v3(0.24, 1.10, 0.22 - swing * 0.08).lerp(v3(0.28, 1.15, 0.10), sprint);
    HPose {
        hips: v3(0.0, -0.04 - run * 0.06 - st.hunch * 0.08 + 0.03 * (2.0 * a).cos() * moving, 0.0),
        pelvis: v3(0.0, a.sin() * (6.0 + 6.0 * run) * moving * fwd_amt, 0.0),
        spine: v3(4.0 + run * 10.0 + sprint * 12.0 + st.hunch * 25.0, 0.0, 0.0),
        chest: v3(st.hunch * 8.0, -a.sin() * 8.0 * moving * fwd_amt, 0.0),
        head: v3(-4.0 - run * 6.0 - st.hunch * 30.0, 0.0, 0.0),
        r_hand: Hand { pos: r_pos, grip: r_grip, ..base.r_hand },
        l_hand: Hand { pos: l_pos, ..base.l_hand },
        l_foot: foot(1.0, a),
        r_foot: foot(-1.0, a + std::f32::consts::PI),
        ..base
    }
}

fn knight_idle() -> Vec<(f32, Ease, HPose)> {
    let base = ready();
    let breathe = HPose { chest: v3(-3.0, 0.0, 0.0), hips: v3(0.0, -0.05, 0.0), ..base };
    vec![(0.0, Smooth, base), (0.5, Smooth, breathe)]
}

// ---- Husks -------------------------------------------------------------

fn husk_base() -> HPose {
    HPose {
        hips: v3(0.0, -0.08, 0.0),
        spine: v3(28.0, 0.0, 0.0),
        chest: v3(8.0, 0.0, 0.0),
        head: v3(-30.0, 0.0, 0.0),
        r_hand: Hand::new(v3(-0.25, 0.85, 0.25), v3(-0.4, 0.0, -1.0), v3(0.1, -0.6, 0.8)),
        l_hand: Hand::new(v3(0.25, 0.85, 0.20), v3(0.4, 0.0, -1.0), v3(0.0, -1.0, 0.2)),
        r_foot: Foot::new(v3(-0.15, 0.05, -0.05), v3(-0.2, 0.0, 1.0)),
        l_foot: Foot::new(v3(0.15, 0.05, 0.10), v3(0.2, 0.0, 1.0)),
        ..HPose::stand()
    }
}

fn husk_idle() -> Vec<(f32, Ease, HPose)> {
    let b = husk_base();
    let sway = HPose { chest: v3(14.0, 6.0, 0.0), head: v3(-25.0, -10.0, 5.0), hips: v3(0.02, -0.10, 0.0), ..b };
    vec![(0.0, Smooth, b), (0.5, Smooth, sway)]
}

fn husk_attack1() -> Vec<(f32, Ease, HPose)> {
    let b = husk_base();
    let wind = HPose {
        chest: v3(-10.0, -10.0, 0.0),
        spine: v3(10.0, 0.0, 0.0),
        head: v3(-10.0, 0.0, 0.0),
        r_hand: Hand::new(v3(-0.15, 1.75, -0.05), v3(-1.0, 0.2, 0.0), v3(0.0, 0.5, -0.85)),
        l_hand: Hand { pos: v3(0.30, 1.10, 0.25), ..b.l_hand },
        ..b
    };
    let strike = HPose {
        spine: v3(35.0, 0.0, 0.0),
        chest: v3(15.0, 0.0, 0.0),
        hips: v3(0.0, -0.15, 0.10),
        r_hand: Hand::new(v3(-0.05, 0.90, 0.55), v3(-1.0, 0.0, -0.3), v3(0.0, -0.6, 0.8)),
        ..feet(wind, v3(0.15, 0.05, 0.35), v3(-0.15, 0.05, -0.2))
    };
    vec![(0.0, Linear, b), (0.40, Out, wind), (0.52, In, strike), (0.75, Linear, strike), (1.0, Smooth, b)]
}

fn husk_attack2() -> Vec<(f32, Ease, HPose)> {
    let b = husk_base();
    let wind = HPose {
        chest: v3(5.0, -40.0, 0.0),
        r_hand: Hand::new(v3(-0.45, 1.30, -0.05), v3(-0.3, -1.0, -0.3), v3(-0.6, 0.3, -0.75)),
        ..b
    };
    let mid = HPose { chest: v3(10.0, 0.0, 0.0), r_hand: Hand::new(v3(-0.05, 1.20, 0.52), v3(-0.5, -1.0, -0.2), v3(0.4, -0.1, 0.9)), ..wind };
    let end = HPose { chest: v3(15.0, 35.0, 0.0), r_hand: Hand::new(v3(0.28, 1.05, 0.25), v3(-0.2, -1.0, -0.2), v3(0.9, -0.2, -0.2)), ..mid };
    vec![(0.0, Linear, b), (0.38, Out, wind), (0.47, In, mid), (0.53, Linear, end), (0.75, Linear, end), (1.0, Smooth, b)]
}

fn husk_crouch() -> Vec<(f32, Ease, HPose)> {
    let c = HPose {
        hips: v3(0.0, -0.62, 0.0),
        spine: v3(45.0, 0.0, 0.0),
        head: v3(30.0, 0.0, 0.0),
        r_hand: Hand::new(v3(-0.12, 0.55, 0.35), v3(-0.5, 0.0, -1.0), v3(0.5, -0.5, 0.5)),
        l_hand: Hand::new(v3(0.12, 0.55, 0.35), v3(0.5, 0.0, -1.0), v3(-0.5, -0.5, 0.5)),
        r_foot: Foot::new(v3(-0.15, 0.05, 0.25), v3(-0.2, 1.0, 1.0)),
        l_foot: Foot::new(v3(0.15, 0.05, 0.25), v3(0.2, 1.0, 1.0)),
        ..husk_base()
    };
    let rock = HPose { spine: v3(48.0, 0.0, 0.0), hips: v3(0.0, -0.63, 0.02), ..c };
    vec![(0.0, Smooth, c), (0.5, Smooth, rock)]
}

fn husk_push() -> Vec<(f32, Ease, HPose)> {
    let b = husk_base();
    let p = HPose {
        spine: v3(35.0, 0.0, 0.0),
        hips: v3(0.0, -0.15, 0.10),
        r_hand: Hand::new(v3(-0.15, 1.10, 0.55), v3(-1.0, -0.5, -0.2), v3(0.0, 0.5, 0.8)),
        l_hand: Hand::new(v3(0.15, 1.10, 0.55), v3(1.0, -0.5, -0.2), v3(0.0, 0.5, 0.8)),
        ..feet(b, v3(0.15, 0.05, 0.25), v3(-0.15, 0.05, -0.45))
    };
    let shove = HPose { hips: v3(0.0, -0.12, 0.25), r_hand: Hand { pos: v3(-0.15, 1.15, 0.75), ..p.r_hand }, l_hand: Hand { pos: v3(0.15, 1.15, 0.75), ..p.l_hand }, ..p };
    vec![(0.0, Linear, b), (0.35, Smooth, p), (0.55, In, shove), (0.8, Linear, shove), (1.0, Smooth, b)]
}

fn archer_aim() -> HPose {
    HPose {
        hips: v3(0.0, -0.06, 0.0),
        spine: v3(10.0, 0.0, 0.0),
        chest: v3(0.0, 40.0, 0.0),
        head: v3(-5.0, -38.0, 0.0),
        l_hand: Hand::new(v3(0.08, 1.42, 0.52), v3(0.3, -1.0, 0.0), v3(0.0, 0.0, 1.0)).up(Y),
        r_hand: Hand::new(v3(-0.05, 1.52, 0.10), v3(-1.0, 0.0, -0.3), v3(0.0, 0.0, 1.0)).up(Y),
        r_foot: Foot::new(v3(-0.14, 0.05, -0.15), v3(-0.3, 0.0, 1.0)),
        l_foot: Foot::new(v3(0.14, 0.05, 0.18), v3(0.3, 0.0, 1.0)),
        ..husk_base()
    }
}

fn archer_shoot() -> Vec<(f32, Ease, HPose)> {
    let a = archer_aim();
    let rel = HPose { r_hand: Hand { pos: v3(-0.22, 1.55, -0.08), ..a.r_hand }, ..a };
    vec![(0.0, Linear, a), (0.1, Out, rel), (0.6, Linear, rel), (1.0, Smooth, a)]
}

fn npc_slump() -> HPose {
    HPose {
        hips: v3(0.0, -0.72, 0.0),
        body: v3(-12.0, 0.0, 0.0),
        spine: v3(-6.0, 0.0, 0.0),
        head: v3(28.0, 12.0, 0.0),
        r_hand: Hand::new(v3(-0.32, 0.30, 0.18), v3(-0.6, 0.5, 0.0), v3(0.2, -0.3, 1.0)),
        l_hand: Hand::new(v3(0.30, 0.35, 0.22), v3(0.6, 0.5, 0.0), v3(0.5, 0.0, 0.8)).up(Y),
        r_foot: Foot::new(v3(-0.12, 0.06, 0.55), v3(0.0, 1.0, 0.3)),
        l_foot: Foot::new(v3(0.18, 0.06, 0.45), v3(0.3, 1.0, 0.3)),
        ..HPose::stand()
    }
}

/// Registers every humanoid clip.
pub fn register(lib: &mut ClipLibrary, skel: &Skeleton) {
    use clip::*;
    let k = |name: &str, dur: f32, looping: bool, keys: Vec<(f32, Ease, HPose)>| Box::new(KeyClip::humanoid(name, dur, looping, &keys, skel));
    let hold = |name: &str, p: HPose| Box::new(KeyClip::humanoid(name, 1.0, true, &[(0.0, Linear, p)], skel));

    lib.add(k(IDLE, 3.0, true, knight_idle()));
    let knight = LocoStyle { hunch: 0.0, stride_scale: 1.0, arm_swing: 1.0, limp: 0.0, sword_and_shield: true };
    lib.add(Box::new(ProcClip::new(LOCO, 1.0, move |ph, p| locomotion(ph, p, knight))));
    lib.add(hold(BLOCK, block_pose()));
    lib.add(k(LIGHT1, LIGHT_DUR, false, light1()));
    lib.add(k(LIGHT2, LIGHT_DUR, false, light2()));
    lib.add(k(HEAVY1, HEAVY1_DUR, false, heavy1()));
    lib.add(k(HEAVY2, HEAVY2_DUR, false, heavy2()));
    lib.add(k(RUN_ATTACK, RUN_ATTACK_DUR, false, run_attack()));
    lib.add(k(ROLL_ATTACK, ROLL_ATTACK_DUR, false, roll_attack()));
    lib.add(hold(PLUNGE_FALL, plunge_fall()));
    lib.add(k(PLUNGE_LAND, PLUNGE_LAND_DUR, false, plunge_land()));
    lib.add(k(ROLL, ROLL_DUR, false, roll()));
    lib.add(k(BACKSTEP, BACKSTEP_DUR, false, backstep()));
    lib.add(k(HIT_LIGHT, HIT_LIGHT_DUR, false, hit_light()));
    lib.add(k(HIT_HEAVY, HIT_HEAVY_DUR, false, hit_heavy()));
    lib.add(k(KNOCKDOWN, KNOCKDOWN_DUR, false, knockdown()));
    lib.add(k(DEATH, DEATH_DUR, false, death()));
    lib.add(k(GUARD_HIT, GUARD_HIT_DUR, false, guard_hit()));
    lib.add(k(GUARD_BREAK, GUARD_BREAK_DUR, false, guard_break()));
    lib.add(k(DRINK, DRINK_DUR, false, drink()));
    lib.add(k(PICKUP, PICKUP_DUR, false, pickup()));
    lib.add(k(PUSH, PUSH_DUR, false, push()));
    lib.add(k(SIT, SIT_DUR, false, vec![(0.0, Linear, ready()), (0.4, Smooth, ready().lerp(&sitting(), 0.5)), (1.0, Smooth, sitting())]));
    lib.add(k(SIT_LOOP, 3.0, true, vec![(0.0, Smooth, sitting()), (0.5, Smooth, HPose { spine: v3(18.0, 0.0, 0.0), ..sitting() })]));
    lib.add(k(STAND_UP, STAND_UP_DUR, false, vec![(0.0, Linear, sitting()), (0.5, Smooth, sitting().lerp(&ready(), 0.6)), (1.0, Smooth, ready())]));
    lib.add(hold(FALL, fall_pose()));
    let land = HPose { hips: v3(0.0, -0.22, 0.0), spine: v3(15.0, 0.0, 0.0), ..ready() };
    lib.add(k(LAND, LAND_DUR, false, vec![(0.0, Linear, fall_pose()), (0.3, Out, land), (1.0, Smooth, ready())]));
    let hard = HPose {
        hips: v3(0.0, -0.50, 0.0),
        spine: v3(40.0, 0.0, 0.0),
        r_hand: Hand { pos: v3(-0.25, 0.40, 0.35), ..ready().r_hand },
        ..feet(ready(), v3(0.14, 0.05, 0.22), v3(-0.14, 0.05, -0.12))
    };
    lib.add(k(LAND_HARD, LAND_HARD_DUR, false, vec![(0.0, Linear, fall_pose()), (0.15, Out, hard), (0.6, Linear, hard), (1.0, Smooth, ready())]));
    let tucked = HPose { ..feet(fall_pose(), v3(0.13, 0.35, 0.15), v3(-0.13, 0.30, -0.05)) };
    lib.add(k(JUMP, JUMP_DUR, false, vec![(0.0, Linear, ready()), (0.3, Out, tucked), (1.0, Smooth, fall_pose())]));

    lib.add(k(HUSK_IDLE, 2.5, true, husk_idle()));
    let husk = LocoStyle { hunch: 1.0, stride_scale: 0.75, arm_swing: 0.6, limp: 0.5, sword_and_shield: false };
    lib.add(Box::new(ProcClip::new(HUSK_LOCO, 1.0, move |ph, p| {
        let mut h = locomotion(ph, p, husk);
        let b = husk_base();
        h.r_hand = Hand { pos: b.r_hand.pos + v3(0.0, 0.0, (ph * TAU).sin() * 0.06), ..b.r_hand };
        h.l_hand = Hand { pos: b.l_hand.pos - v3(0.0, 0.0, (ph * TAU).sin() * 0.06), ..b.l_hand };
        h
    })));
    lib.add(k(HUSK_ATTACK1, HUSK_ATTACK1_DUR, false, husk_attack1()));
    lib.add(k(HUSK_ATTACK2, HUSK_ATTACK2_DUR, false, husk_attack2()));
    lib.add(k(HUSK_CROUCH, 3.0, true, husk_crouch()));
    lib.add(k(HUSK_PUSH, HUSK_PUSH_DUR, false, husk_push()));
    lib.add(hold(ARCHER_AIM, archer_aim()));
    lib.add(k(ARCHER_SHOOT, ARCHER_SHOOT_DUR, false, archer_shoot()));
    lib.add(k(NPC_SLUMP, 4.0, true, vec![(0.0, Smooth, npc_slump()), (0.5, Smooth, HPose { head: v3(32.0, 8.0, 0.0), ..npc_slump() })]));
    let talk = HPose { head: v3(5.0, 0.0, 0.0), ..npc_slump() };
    lib.add(k(NPC_TALK, 3.0, true, vec![(0.0, Smooth, talk), (0.5, Smooth, HPose { head: v3(10.0, -6.0, 0.0), ..talk })]));
}
