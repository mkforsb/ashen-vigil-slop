//! Enemy and NPC brains. Each brain reads a snapshot of the world and
//! returns an [`Intent`] for the shared action system.

use crate::actor::{Action, ActorId, Intent, Request, Team};
use crate::combat::Atk;
use ashen_engine::collision::CollisionWorld;
use ashen_engine::math::*;
use ashen_engine::rng::Rng;

/// Read-only snapshot of an actor for AI decisions.
#[derive(Clone, Copy, Debug)]
pub struct ActorView {
    pub id: ActorId,
    pub team: Team,
    pub pos: Vec3,
    pub center: Vec3,
    pub yaw: f32,
    pub alive: bool,
    pub busy: bool,
    pub attacking: bool,
    pub grounded: bool,
    pub hp_frac: f32,
}

pub struct ThinkCtx<'a> {
    pub dt: f32,
    pub col: &'a CollisionWorld,
    pub rng: &'a mut Rng,
    pub player: Option<ActorView>,
    /// Sounds/alerts requested by brains: (actor id, kind).
    pub barks: &'a mut Vec<(ActorId, Bark)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bark {
    Alert,
    Shoot,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HuskState {
    Idle,
    Chase,
    Return,
}

#[derive(Clone, Debug)]
pub struct HuskBrain {
    pub passive: bool,
    pub state: HuskState,
    pub aggro: f32,
    pub cd: f32,
    pub home: Vec3,
    pub shuffle: f32,
}

impl HuskBrain {
    pub fn new(home: Vec3, passive: bool) -> Self {
        HuskBrain { passive, state: HuskState::Idle, aggro: 9.0, cd: 0.8, home, shuffle: 0.0 }
    }
}

#[derive(Clone, Debug)]
pub struct ArcherBrain {
    pub cd: f32,
    pub aim: f32,
    pub range: f32,
    pub aware: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemonPhase {
    /// Sitting on the high ledge, waiting for the player to enter.
    Perched,
    /// Dropping into the arena (scripted).
    Dropping,
    /// Standing in the arena, waiting for the player to come through the fog.
    Waiting,
    Combat,
}

#[derive(Clone, Debug)]
pub struct DemonBrain {
    pub phase: DemonPhase,
    pub cd: f32,
    pub ledge_timer: f32,
    pub last: Option<Atk>,
}

impl DemonBrain {
    pub fn new(phase: DemonPhase) -> Self {
        DemonBrain { phase, cd: 1.5, ledge_timer: 0.0, last: None }
    }
}

#[derive(Clone, Debug)]
pub enum Brain {
    None,
    Player,
    Husk(HuskBrain),
    Archer(ArcherBrain),
    Demon(DemonBrain),
    /// Scripted husk that shoves the boulder when triggered.
    Pusher { triggered: bool },
    Npc,
}

/// Extra intent fields for brains that act outside the action system.
#[derive(Clone, Copy, Debug, Default)]
pub struct Extra {
    /// Fire a projectile at this point.
    pub shoot_at: Option<Vec3>,
}

fn sees(ctx: &ThinkCtx, from: Vec3, to: Vec3) -> bool {
    ctx.col.line_of_sight(from, to)
}

impl Brain {
    pub fn think(&mut self, me: &ActorView, action: &Action, ctx: &mut ThinkCtx) -> (Intent, Extra) {
        let mut it = Intent { speed_scale: 1.0, ..Default::default() };
        let mut ex = Extra::default();
        let Some(p) = ctx.player.filter(|p| p.alive) else {
            if let Brain::Husk(h) = self {
                h.state = HuskState::Return;
                let to = flat(h.home - me.pos);
                if to.length() > 0.6 {
                    it.mv = to.normalize() * 0.6;
                }
            }
            return (it, ex);
        };
        let to = flat(p.pos - me.pos);
        let d = to.length();
        let ang = angle_diff(me.yaw, dir_yaw(to)).abs();
        match self {
            Brain::None | Brain::Player | Brain::Npc | Brain::Pusher { .. } => {}
            Brain::Husk(h) => {
                h.cd -= ctx.dt;
                if h.passive {
                    return (it, ex);
                }
                match h.state {
                    HuskState::Idle => {
                        let cone = ang < deg(70.0) || d < 3.0;
                        if d < h.aggro && cone && (p.pos.y - me.pos.y).abs() < 3.0 && sees(ctx, me.center, p.center) {
                            h.state = HuskState::Chase;
                            h.cd = 0.6;
                            ctx.barks.push((me.id, Bark::Alert));
                        }
                    }
                    HuskState::Chase => {
                        if flat(me.pos - h.home).length() > 22.0 || (p.pos.y - me.pos.y).abs() > 4.0 {
                            h.state = HuskState::Return;
                        } else if !me.busy {
                            it.face = Some(p.center);
                            if d > 1.8 {
                                it.mv = to / d;
                                it.speed_scale = 0.72;
                            } else {
                                h.shuffle += ctx.dt;
                                it.mv = Vec3::new(-to.z, 0.0, to.x).normalize_or_zero() * (h.shuffle * 0.8).sin() * 0.3;
                                it.walk = true;
                                if h.cd <= 0.0 && ang < deg(40.0) {
                                    let atk = if ctx.rng.chance(0.6) { Atk::HuskOverhead } else { Atk::HuskSwipe };
                                    it.request = Some(Request::Attack(atk));
                                    h.cd = ctx.rng.range(1.3, 2.8);
                                }
                            }
                        }
                    }
                    HuskState::Return => {
                        let home = flat(h.home - me.pos);
                        if home.length() < 0.6 {
                            h.state = HuskState::Idle;
                        } else {
                            it.mv = home.normalize() * 0.6;
                            it.walk = true;
                        }
                        if d < h.aggro * 0.6 && (p.pos.y - me.pos.y).abs() < 3.0 && sees(ctx, me.center, p.center) {
                            h.state = HuskState::Chase;
                        }
                    }
                }
            }
            Brain::Archer(a) => {
                a.cd -= ctx.dt;
                let visible = d < a.range && sees(ctx, me.center + Vec3::Y * 0.3, p.center);
                if visible {
                    if !a.aware {
                        ctx.barks.push((me.id, Bark::Alert));
                    }
                    a.aware = true;
                    it.face = Some(p.center);
                    if !me.busy && a.cd <= 0.0 && ang < deg(20.0) {
                        a.aim += ctx.dt;
                        if a.aim > 0.9 {
                            a.aim = 0.0;
                            a.cd = ctx.rng.range(2.2, 3.4);
                            ex.shoot_at = Some(p.center);
                            ctx.barks.push((me.id, Bark::Shoot));
                        }
                    }
                } else {
                    a.aim = 0.0;
                }
            }
            Brain::Demon(b) => {
                if b.phase != DemonPhase::Combat {
                    return (it, ex);
                }
                b.cd -= ctx.dt;
                it.face = Some(p.center);
                let on_ledge = p.pos.y > me.pos.y + 5.0;
                if on_ledge {
                    b.ledge_timer += ctx.dt;
                } else {
                    b.ledge_timer = 0.0;
                }
                if me.busy || matches!(action, Action::Land { .. }) {
                    return (it, ex);
                }
                // Gets more aggressive as it weakens.
                let rage = 1.0 - me.hp_frac;
                if on_ledge {
                    if d > 4.0 {
                        it.mv = to / d;
                        it.speed_scale = 0.56;
                    }
                    if b.ledge_timer > 7.0 && b.cd <= 0.0 && d < 9.0 {
                        it.request = Some(Request::Attack(Atk::DemonLedge));
                        b.ledge_timer = 0.0;
                        b.cd = 3.0;
                    }
                    return (it, ex);
                }
                if b.cd > 0.0 {
                    if d > 5.5 {
                        it.mv = to / d;
                        it.speed_scale = 0.56;
                    }
                    return (it, ex);
                }
                let r = ctx.rng.f32();
                let pick = if d > 10.0 {
                    if r < 0.55 { Some(Atk::DemonLeap) } else { None }
                } else if d > 6.0 {
                    if r < 0.3 { Some(Atk::DemonLeap) } else { None }
                } else if ang < deg(35.0) {
                    Some(if r < 0.45 {
                        Atk::DemonSmash
                    } else if r < 0.75 {
                        Atk::DemonSweep
                    } else {
                        Atk::DemonPound
                    })
                } else if ang < deg(110.0) {
                    Some(if r < 0.6 { Atk::DemonSweep } else { Atk::DemonPound })
                } else if r < 0.65 {
                    Some(Atk::DemonPound)
                } else {
                    None
                };
                // Avoid repeating the same attack three times in a row.
                let pick = match (pick, b.last) {
                    (Some(a), Some(l)) if a == l && ctx.rng.chance(0.5) => None,
                    _ => pick,
                };
                match pick {
                    Some(a) => {
                        it.request = Some(Request::Attack(a));
                        b.last = Some(a);
                        b.cd = ctx.rng.range(0.9, 2.0) * (1.0 - rage * 0.4);
                    }
                    None => {
                        if d > 5.0 {
                            it.mv = to / d.max(0.01);
                            it.speed_scale = 0.56;
                        }
                        b.cd = 0.4;
                    }
                }
            }
        }
        (it, ex)
    }
}
