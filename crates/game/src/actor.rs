//! Actors (player, enemies, NPCs) and the shared action state machine.
//!
//! Brains (player input or AI) produce an [`Intent`]; the action system turns
//! intents into committed actions with animation, root motion, stamina,
//! i-frames, hyperarmor and cancel windows. Every character uses the same
//! machinery, so new enemies mostly need a moveset and a brain.

use crate::ai::Brain;
use crate::anims::{demon::clip as dc, humanoid::clip as hc};
use crate::assets::sounds::Sfx;
use crate::assets::{Assets, ModelDef};
use crate::combat::{self, Atk, AttackDef, CueKind, ShieldKind, WeaponKind};
use crate::rig::{demon as db, humanoid as hb};
use ashen_engine::anim::{AnimParams, Animator, Overlay};
use ashen_engine::collision::{CharacterShape, CollisionWorld};
use ashen_engine::math::*;
use ashen_engine::render::MeshHandle;
use ashen_engine::skeleton::Pose;

pub type ActorId = u32;

pub const GRAVITY: f32 = 24.0;
pub const RUN_SPEED: f32 = 4.0;
pub const WALK_SPEED: f32 = 1.6;
pub const SPRINT_SPEED: f32 = 6.0;
pub const LOCK_SPEED: f32 = 3.4;
pub const BLOCK_SPEED: f32 = 1.8;
pub const ROLL_COST: f32 = 20.0;
pub const BACKSTEP_COST: f32 = 14.0;
pub const JUMP_COST: f32 = 18.0;
pub const SPRINT_COST: f32 = 14.0;
/// Falls shorter than this never hurt.
pub const SAFE_FALL: f32 = 5.0;
/// Falls longer than this are lethal.
pub const LETHAL_FALL: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Team {
    Player,
    Hostile,
    Neutral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    Knight,
    Corvin,
    Husk,
    HuskPale,
    Demon,
}

impl BodyKind {
    pub fn is_demon(self) -> bool {
        self == BodyKind::Demon
    }

    pub fn model(self, assets: &Assets) -> &ModelDef {
        match self {
            BodyKind::Knight => &assets.knight,
            BodyKind::Corvin => &assets.corvin,
            BodyKind::Husk => &assets.husk,
            BodyKind::HuskPale => &assets.husk_pale,
            BodyKind::Demon => &assets.demon,
        }
    }
}

/// Which clips a body uses for its basic states.
#[derive(Clone, Copy, Debug)]
pub struct AnimSet {
    pub idle: &'static str,
    pub loco: &'static str,
    /// Metres per locomotion cycle at a given speed (0 = fixed rate).
    pub cycle: fn(f32) -> f32,
    pub hit_light: &'static str,
    pub hit_heavy: &'static str,
    pub knockdown: &'static str,
    pub death: &'static str,
    pub fall: &'static str,
    pub land: &'static str,
}

fn knight_cycle(s: f32) -> f32 {
    crate::anims::humanoid::cycle_len(s)
}

fn husk_cycle(s: f32) -> f32 {
    crate::anims::humanoid::cycle_len(s) * 0.75
}

fn demon_cycle(_s: f32) -> f32 {
    3.4
}

pub const KNIGHT_ANIMS: AnimSet = AnimSet {
    idle: hc::IDLE,
    loco: hc::LOCO,
    cycle: knight_cycle,
    hit_light: hc::HIT_LIGHT,
    hit_heavy: hc::HIT_HEAVY,
    knockdown: hc::KNOCKDOWN,
    death: hc::DEATH,
    fall: hc::FALL,
    land: hc::LAND,
};

pub const HUSK_ANIMS: AnimSet = AnimSet {
    idle: hc::HUSK_IDLE,
    loco: hc::HUSK_LOCO,
    cycle: husk_cycle,
    hit_light: hc::HIT_LIGHT,
    hit_heavy: hc::HIT_HEAVY,
    knockdown: hc::KNOCKDOWN,
    death: hc::DEATH,
    fall: hc::FALL,
    land: hc::LAND,
};

pub const DEMON_ANIMS: AnimSet = AnimSet {
    idle: dc::IDLE,
    loco: dc::WALK,
    cycle: demon_cycle,
    hit_light: dc::IDLE,
    hit_heavy: dc::STAGGER,
    knockdown: dc::STAGGER,
    death: dc::DEATH,
    fall: dc::FALL,
    land: dc::LAND,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Request {
    Light,
    Heavy,
    /// Roll towards a world direction; zero means backstep.
    Roll(Vec3),
    Jump,
    Drink,
    Interact,
    /// A specific attack (used by AI).
    Attack(Atk),
}

/// What a brain wants this tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct Intent {
    /// Desired movement in world space; length 1 = run.
    pub mv: Vec3,
    pub sprint: bool,
    pub walk: bool,
    pub block: bool,
    /// Point to keep facing (lock-on target or AI target).
    pub face: Option<Vec3>,
    pub request: Option<Request>,
    /// Speed multiplier for AI locomotion.
    pub speed_scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseTag {
    Pickup,
    PushDoor,
    SitDown,
    StandUp,
    FogWalk,
    Push,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Move,
    Attack { atk: Atk, chain: Option<(bool, u8)>, target: Option<Vec3>, origin: Vec3 },
    Roll { dir: Vec3 },
    Backstep,
    Jump { dir: Vec3 },
    Fall,
    Plunge { hit: bool },
    Land { hard: bool },
    Hit { heavy: bool, dir: Vec3 },
    Knockdown { dir: Vec3 },
    GuardHit { dir: Vec3 },
    GuardBreak,
    Drink { healed: bool },
    Use { tag: UseTag, clip: &'static str, dur: f32, fired: bool, dir: Vec3 },
    Sitting,
    Script { clip: &'static str, dur: f32 },
    Death,
}

#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub hp: f32,
    pub max_hp: f32,
    pub stamina: f32,
    pub max_stamina: f32,
    pub poise: f32,
    pub max_poise: f32,
    /// Physical damage absorption 0..1.
    pub absorb: f32,
    pub stamina_regen: f32,
}

impl Stats {
    pub fn knight() -> Self {
        Stats { hp: 520.0, max_hp: 520.0, stamina: 100.0, max_stamina: 100.0, poise: 30.0, max_poise: 30.0, absorb: 0.25, stamina_regen: 52.0 }
    }

    pub fn husk(hp: f32) -> Self {
        Stats { hp, max_hp: hp, stamina: 80.0, max_stamina: 80.0, poise: 8.0, max_poise: 8.0, absorb: 0.05, stamina_regen: 40.0 }
    }

    pub fn demon() -> Self {
        Stats { hp: 1000.0, max_hp: 1000.0, stamina: 999.0, max_stamina: 999.0, poise: 160.0, max_poise: 160.0, absorb: 0.35, stamina_regen: 999.0 }
    }
}

/// Events produced by actors during their update.
#[derive(Clone, Copy, Debug)]
pub enum ActorEvent {
    Sound { sfx: Sfx, pos: Vec3, vol: f32 },
    Shake(f32),
    Dust { pos: Vec3, size: f32 },
    Footstep { pos: Vec3, surface: u8, heavy: bool },
    Healed(ActorId),
    Died(ActorId),
    FellToDeath(ActorId),
    UseFired { id: ActorId, tag: UseTag },
    Plunged { id: ActorId },
    Landed { id: ActorId, height: f32 },
}

pub struct Actor {
    pub id: ActorId,
    pub name: &'static str,
    pub body: BodyKind,
    pub team: Team,
    pub anims: AnimSet,
    pub right: Option<WeaponKind>,
    pub left: Option<ShieldKind>,
    pub right_mesh: Option<MeshHandle>,
    pub left_mesh: Option<MeshHandle>,
    pub pos: Vec3,
    pub yaw: f32,
    pub vel: Vec3,
    pub grounded: bool,
    pub air_top: f32,
    pub shape: CharacterShape,
    pub mass: f32,
    pub stats: Stats,
    pub stamina_delay: f32,
    pub poise_timer: f32,
    pub action: Action,
    pub action_t: f32,
    pub queued: Option<(Request, f32)>,
    pub anim: Animator,
    pub params: AnimParams,
    pub pose: Pose,
    /// Model-space bone transforms of the current pose.
    pub mats: Vec<Mat4>,
    pub block_amount: f32,
    pub blocking: bool,
    pub sprinting: bool,
    pub lock: Option<ActorId>,
    pub hitstop: f32,
    pub flash: f32,
    pub dead: bool,
    pub dissolve: f32,
    pub remove: bool,
    pub souls: u32,
    pub brain: Brain,
    pub spawn_pos: Vec3,
    pub spawn_yaw: f32,
    /// Ids already hit by the current attack swing.
    pub hit_list: Vec<ActorId>,
    pub prev_blade: Option<(Vec3, Vec3)>,
    pub step_phase: f32,
    /// Extra vertical offset of the visual body (leaps).
    pub lift: f32,
    pub invulnerable: bool,
    /// Last attacker, used for kill credit.
    pub last_hit_by: Option<ActorId>,
    pub cue_cursor: f32,
    pub surface: u8,
}

impl Actor {
    pub fn new(id: ActorId, name: &'static str, body: BodyKind, team: Team, pos: Vec3, yaw: f32, assets: &Assets) -> Self {
        let (anims, shape, mass, stats) = match body {
            BodyKind::Demon => (DEMON_ANIMS, CharacterShape { radius: 2.35, height: 5.0, step: 0.6 }, 50.0, Stats::demon()),
            BodyKind::Husk | BodyKind::HuskPale => (HUSK_ANIMS, CharacterShape { radius: 0.35, height: 1.7, step: 0.45 }, 1.0, Stats::husk(110.0)),
            _ => (KNIGHT_ANIMS, CharacterShape { radius: 0.38, height: 1.8, step: 0.45 }, 1.2, Stats::knight()),
        };
        let skel_len = if body.is_demon() { db::BONES } else { hb::BONES };
        let idle = assets.clips.id(anims.idle);
        Actor {
            id,
            name,
            body,
            team,
            anims,
            right: None,
            left: None,
            right_mesh: None,
            left_mesh: None,
            pos,
            yaw,
            vel: Vec3::ZERO,
            grounded: true,
            air_top: pos.y,
            shape,
            mass,
            stats,
            stamina_delay: 0.0,
            poise_timer: 0.0,
            action: Action::Move,
            action_t: 0.0,
            queued: None,
            anim: Animator::new(idle),
            params: AnimParams::default(),
            pose: Pose::identity(skel_len),
            mats: vec![Mat4::IDENTITY; skel_len],
            block_amount: 0.0,
            blocking: false,
            sprinting: false,
            lock: None,
            hitstop: 0.0,
            flash: 0.0,
            dead: false,
            dissolve: 0.0,
            remove: false,
            souls: 0,
            brain: Brain::None,
            spawn_pos: pos,
            spawn_yaw: yaw,
            hit_list: Vec::new(),
            prev_blade: None,
            step_phase: 0.0,
            lift: 0.0,
            invulnerable: false,
            last_hit_by: None,
            cue_cursor: 0.0,
            surface: 0,
        }
    }

    pub fn equip(&mut self, right: Option<WeaponKind>, left: Option<ShieldKind>, assets: &Assets) {
        self.right = right;
        self.left = left;
        self.right_mesh = right.map(|w| match w {
            WeaponKind::Longsword => assets.items.longsword,
            WeaponKind::BrokenSword => assets.items.broken_sword,
            WeaponKind::Bow => assets.items.bow,
            WeaponKind::DemonClub => assets.items.club,
        });
        self.left_mesh = left.map(|_| assets.items.kite_shield);
        if right == Some(WeaponKind::Bow) {
            // Bows are held in the left hand.
            self.left_mesh = self.right_mesh.take();
        }
    }

    pub fn alive(&self) -> bool {
        !self.dead
    }

    pub fn forward(&self) -> Vec3 {
        yaw_dir(self.yaw)
    }

    /// Root transform (model -> world), including visual lift.
    pub fn world(&self) -> Mat4 {
        Mat4::from_rotation_translation(Quat::from_rotation_y(self.yaw), self.pos + Vec3::Y * self.lift)
    }

    pub fn bone_world(&self, bone: usize) -> Mat4 {
        self.world() * self.mats[bone]
    }

    /// Approximate centre of mass for targeting.
    pub fn center(&self) -> Vec3 {
        self.pos + Vec3::Y * (self.shape.height * 0.55 + self.lift)
    }

    pub fn action_dur(&self) -> f32 {
        match &self.action {
            Action::Attack { atk, .. } => combat::attack(*atk).dur,
            Action::Roll { .. } => hc::ROLL_DUR,
            Action::Backstep => hc::BACKSTEP_DUR,
            Action::Jump { .. } => hc::JUMP_DUR,
            Action::Land { hard } => {
                if self.body.is_demon() {
                    dc::LAND_DUR
                } else if *hard {
                    hc::LAND_HARD_DUR
                } else {
                    hc::LAND_DUR
                }
            }
            Action::Hit { heavy, .. } => {
                if self.body.is_demon() {
                    dc::STAGGER_DUR
                } else if *heavy {
                    hc::HIT_HEAVY_DUR
                } else {
                    hc::HIT_LIGHT_DUR
                }
            }
            Action::Knockdown { .. } => {
                if self.body.is_demon() {
                    dc::STAGGER_DUR
                } else {
                    hc::KNOCKDOWN_DUR
                }
            }
            Action::GuardHit { .. } => hc::GUARD_HIT_DUR,
            Action::GuardBreak => hc::GUARD_BREAK_DUR,
            Action::Drink { .. } => hc::DRINK_DUR,
            Action::Use { dur, .. } => *dur,
            Action::Script { dur, .. } => *dur,
            Action::Death => {
                if self.body.is_demon() {
                    dc::DEATH_DUR
                } else {
                    hc::DEATH_DUR
                }
            }
            Action::Move | Action::Fall | Action::Plunge { .. } | Action::Sitting => f32::INFINITY,
        }
    }

    /// Normalised action time.
    pub fn action_tn(&self) -> f32 {
        let d = self.action_dur();
        if d.is_finite() { (self.action_t / d).min(1.0) } else { 0.0 }
    }

    pub fn current_attack(&self) -> Option<&'static AttackDef> {
        match &self.action {
            Action::Attack { atk, .. } => Some(combat::attack(*atk)),
            _ => None,
        }
    }

    pub fn has_iframes(&self) -> bool {
        if self.invulnerable {
            return true;
        }
        let tn = self.action_tn();
        match &self.action {
            Action::Roll { .. } => (0.08..=0.55).contains(&tn),
            Action::Backstep => (0.05..=0.4).contains(&tn),
            Action::Knockdown { .. } => !self.body.is_demon() && (0.15..=0.8).contains(&tn),
            Action::Death => true,
            _ => false,
        }
    }

    pub fn has_hyperarmor(&self) -> bool {
        match &self.action {
            Action::Attack { atk, .. } => combat::attack(*atk).has_hyperarmor(self.action_tn()),
            Action::Script { .. } => true,
            _ => false,
        }
    }

    /// Starts an action, resetting timers.
    pub fn set_action(&mut self, a: Action) {
        self.action = a;
        self.action_t = 0.0;
        self.hit_list.clear();
        self.prev_blade = None;
        self.cue_cursor = -1.0;
        self.lift = 0.0;
    }

    fn spend(&mut self, cost: f32) {
        self.stats.stamina -= cost;
        self.stamina_delay = 0.55;
    }

    /// Whether a queued request may start right now.
    fn can_start(&self, req: &Request) -> bool {
        let tn = self.action_tn();
        let stam_ok = self.stats.stamina > 0.0;
        let needs_stamina = !matches!(req, Request::Drink | Request::Interact);
        if needs_stamina && !stam_ok {
            return false;
        }
        if !self.grounded && !matches!(self.action, Action::Fall | Action::Jump { .. }) {
            return false;
        }
        match &self.action {
            Action::Move => true,
            Action::Attack { atk, .. } => {
                let d = combat::attack(*atk);
                match req {
                    Request::Light | Request::Heavy | Request::Attack(_) => tn >= d.chain_at,
                    Request::Roll(_) | Request::Jump => tn >= d.cancel_at,
                    Request::Drink | Request::Interact => tn >= 0.9,
                }
            }
            Action::Roll { .. } => match req {
                Request::Light => tn >= 0.55,
                Request::Roll(_) => tn >= 0.72,
                Request::Heavy => tn >= 0.8,
                _ => tn >= 0.85,
            },
            Action::Backstep => match req {
                Request::Light => tn >= 0.4,
                _ => tn >= 0.7,
            },
            Action::Fall | Action::Jump { .. } => matches!(req, Request::Light | Request::Heavy) && self.vel.y < 0.0,
            Action::Land { hard } => tn >= if *hard { 0.8 } else { 0.4 },
            Action::GuardHit { .. } => matches!(req, Request::Roll(_)) && tn >= 0.5,
            Action::Hit { heavy: false, .. } => matches!(req, Request::Roll(_)) && tn >= 0.75,
            _ => false,
        }
    }

    /// Picks the attack a Light/Heavy request turns into given the current action.
    fn resolve_attack(&self, heavy: bool) -> Option<(Atk, Option<(bool, u8)>)> {
        let w = combat::weapon(self.right?);
        let list = if heavy { w.heavy } else { w.light };
        if list.is_empty() {
            return None;
        }
        match &self.action {
            Action::Move if self.sprinting && !heavy => w.running.map(|a| (a, None)),
            Action::Roll { .. } if !heavy => w.rolling.map(|a| (a, None)),
            Action::Backstep if !heavy => w.backstep.map(|a| (a, None)),
            Action::Attack { chain: Some((was_heavy, i)), .. } if *was_heavy == heavy => {
                let n = (*i as usize + 1) % list.len();
                Some((list[n], Some((heavy, n as u8))))
            }
            _ => Some((list[0], Some((heavy, 0)))),
        }
        .or(Some((list[0], Some((heavy, 0)))))
    }

    fn start_request(&mut self, req: Request, ctx: &mut Ctx) -> bool {
        match req {
            Request::Light | Request::Heavy => {
                if matches!(self.action, Action::Fall | Action::Jump { .. }) {
                    if self.right.is_some_and(|w| combat::weapon(w).plunge.is_some()) {
                        self.set_action(Action::Plunge { hit: false });
                        return true;
                    }
                    return false;
                }
                let Some((atk, chain)) = self.resolve_attack(req == Request::Heavy) else { return false };
                self.begin_attack(atk, chain, ctx.face_target);
                true
            }
            Request::Attack(atk) => {
                self.begin_attack(atk, None, ctx.face_target);
                true
            }
            Request::Roll(dir) => {
                if dir.length_squared() > 0.01 {
                    let d = flat(dir).normalize();
                    self.yaw = dir_yaw(d);
                    self.spend(ROLL_COST);
                    self.set_action(Action::Roll { dir: d });
                    ctx.emit(ActorEvent::Sound { sfx: Sfx::Roll, pos: self.pos, vol: 0.8 });
                } else {
                    self.spend(BACKSTEP_COST);
                    self.set_action(Action::Backstep);
                    ctx.emit(ActorEvent::Sound { sfx: Sfx::Roll, pos: self.pos, vol: 0.5 });
                }
                true
            }
            Request::Jump => {
                if !self.sprinting {
                    return false;
                }
                self.spend(JUMP_COST);
                let d = self.forward();
                self.vel.y = 6.0;
                self.grounded = false;
                self.air_top = self.pos.y;
                self.set_action(Action::Jump { dir: d });
                true
            }
            Request::Drink => {
                if ctx.flasks_available {
                    self.set_action(Action::Drink { healed: false });
                    ctx.used_flask = true;
                    true
                } else {
                    false
                }
            }
            Request::Interact => false,
        }
    }

    pub fn begin_attack(&mut self, atk: Atk, chain: Option<(bool, u8)>, target: Option<Vec3>) {
        let def = combat::attack(atk);
        self.spend(def.stamina);
        let origin = self.pos;
        self.set_action(Action::Attack { atk, chain, target, origin });
    }

    /// Sets the action to a timed interaction (pickups, doors, fog veils...).
    pub fn begin_use(&mut self, tag: UseTag, dir: Vec3) {
        let (clip, dur) = match tag {
            UseTag::Pickup => (hc::PICKUP, hc::PICKUP_DUR),
            UseTag::PushDoor => (hc::PUSH, hc::PUSH_DUR),
            UseTag::SitDown => (hc::SIT, hc::SIT_DUR),
            UseTag::StandUp => (hc::STAND_UP, hc::STAND_UP_DUR),
            UseTag::FogWalk => (hc::PUSH, 1.6),
            UseTag::Push => (hc::HUSK_PUSH, hc::HUSK_PUSH_DUR),
        };
        if dir.length_squared() > 0.01 {
            self.yaw = dir_yaw(dir);
        }
        self.set_action(Action::Use { tag, clip, dur, fired: false, dir });
    }

    /// Reaction to being hit (damage already applied).
    pub fn react(&mut self, knock: combat::Knock, poise_dmg: f32, from: Vec3) {
        if self.dead {
            return;
        }
        let dir = flat(self.pos - from).normalize_or(-self.forward());
        self.stats.poise -= poise_dmg;
        self.poise_timer = 3.0;
        let broken = self.stats.poise <= 0.0;
        if knock == combat::Knock::Knockdown && !self.body.is_demon() {
            self.stats.poise = self.stats.max_poise;
            self.set_action(Action::Knockdown { dir });
            return;
        }
        if self.has_hyperarmor() && !(self.body.is_demon() && knock == combat::Knock::Knockdown && broken) {
            return;
        }
        if broken {
            self.stats.poise = self.stats.max_poise;
            if self.body.is_demon() {
                self.set_action(Action::Knockdown { dir });
            } else {
                let heavy = knock == combat::Knock::Stagger;
                self.yaw = dir_yaw(-dir);
                self.set_action(Action::Hit { heavy, dir });
            }
        }
    }

    pub fn kill(&mut self, ctx: &mut Ctx) {
        if self.dead {
            return;
        }
        self.dead = true;
        self.stats.hp = 0.0;
        self.blocking = false;
        self.set_action(Action::Death);
        ctx.emit(ActorEvent::Died(self.id));
    }

    /// Per-tick update: intent -> action -> movement -> animation.
    pub fn update(&mut self, intent: &Intent, ctx: &mut Ctx) {
        let dt = ctx.dt;
        self.flash = (self.flash - dt * 4.0).max(0.0);
        if self.hitstop > 0.0 {
            self.hitstop -= dt;
            self.animate(ctx, 0.0);
            return;
        }
        if self.dead {
            self.action_t += dt;
            if self.action_t > self.action_dur() * 0.7 {
                self.dissolve = (self.dissolve + dt * 0.5).min(1.0);
            }
            self.physics(Vec3::ZERO, ctx);
            self.animate(ctx, dt);
            return;
        }

        // Stamina and poise regeneration.
        let busy = matches!(self.action, Action::Attack { .. } | Action::Roll { .. } | Action::Backstep | Action::Jump { .. });
        if self.sprinting {
            self.stats.stamina -= SPRINT_COST * dt;
            self.stamina_delay = 0.3;
        }
        if self.stamina_delay > 0.0 {
            self.stamina_delay -= dt;
        } else if !busy {
            let rate = if self.blocking { self.stats.stamina_regen * 0.3 } else { self.stats.stamina_regen };
            self.stats.stamina = (self.stats.stamina + rate * dt).min(self.stats.max_stamina);
        }
        if self.poise_timer > 0.0 {
            self.poise_timer -= dt;
        } else {
            self.stats.poise = self.stats.max_poise;
        }

        // Queue requests and try to start them.
        if let Some(r) = intent.request {
            self.queued = Some((r, 0.45));
        }
        if let Some((r, ttl)) = self.queued {
            if self.can_start(&r) {
                self.queued = None;
                self.start_request(r, ctx);
            } else if ttl - dt <= 0.0 {
                self.queued = None;
            } else {
                self.queued = Some((r, ttl - dt));
            }
        }

        self.action_t += dt;
        let tn = self.action_tn();
        let fwd = self.forward();
        let mut want_vel = Vec3::ZERO;

        // Blocking is a held state usable while moving or in late recovery.
        let can_block = self.left.is_some()
            && match &self.action {
                Action::Move => true,
                Action::GuardHit { .. } => true,
                Action::Attack { atk, .. } => tn >= combat::attack(*atk).cancel_at,
                _ => false,
            };
        self.blocking = intent.block && can_block;
        if self.blocking && !matches!(self.action, Action::Move | Action::GuardHit { .. }) {
            self.set_action(Action::Move);
        }
        let target_block = if self.blocking { 1.0 } else { 0.0 };
        self.block_amount = approach(self.block_amount, target_block, dt * 7.0);

        match self.action.clone() {
            Action::Move => {
                let mv = intent.mv;
                let mag = mv.length().min(1.0);
                let locked = intent.face.is_some();
                self.sprinting = intent.sprint && mag > 0.3 && self.stats.stamina > 0.0 && !self.blocking;
                let mut speed = if self.sprinting {
                    SPRINT_SPEED
                } else if intent.walk || mag < 0.5 {
                    WALK_SPEED * (mag / 0.5).min(1.0)
                } else if locked {
                    LOCK_SPEED
                } else {
                    RUN_SPEED
                };
                if self.blocking {
                    speed = speed.min(BLOCK_SPEED);
                }
                if intent.speed_scale > 0.0 {
                    speed *= intent.speed_scale;
                }
                want_vel = if mag > 0.05 { mv / mag.max(1e-3) * speed } else { Vec3::ZERO };
                let turn = deg(if self.body.is_demon() { 70.0 } else { 900.0 }) * dt;
                if let (Some(p), false) = (intent.face, self.sprinting) {
                    let d = flat(p - self.pos);
                    if d.length_squared() > 0.01 {
                        self.yaw = approach_angle(self.yaw, dir_yaw(d), turn);
                    }
                } else if mag > 0.05 {
                    self.yaw = approach_angle(self.yaw, dir_yaw(mv), turn);
                }
                if !self.grounded {
                    self.air_top = self.pos.y;
                    self.set_action(Action::Fall);
                }
            }
            Action::Attack { atk, target, origin, .. } => {
                self.sprinting = false;
                let def = combat::attack(atk);
                let face = intent.face.or(target);
                if tn <= def.track_until {
                    let turn = deg(def.track_speed) * dt;
                    if let Some(p) = face {
                        let d = flat(p - self.pos);
                        if d.length_squared() > 0.01 {
                            self.yaw = approach_angle(self.yaw, dir_yaw(d), turn);
                        }
                    } else if intent.mv.length_squared() > 0.04 && tn < 0.05 {
                        self.yaw = dir_yaw(intent.mv);
                    }
                }
                want_vel = self.forward() * def.motion_at(tn);
                if let (Some((t0, t1, max_d)), Some(tp)) = (def.leap, target) {
                    if tn >= t0 && tn <= t1 {
                        let to = flat(tp - origin);
                        let dist = (to.length() - 3.5).clamp(0.0, max_d);
                        let dir = to.normalize_or(fwd);
                        want_vel = dir * dist / ((t1 - t0) * def.dur);
                    }
                }
                self.lift = def.lift_at(tn);
                self.fire_cues(def, tn, ctx);
                if tn >= 1.0 {
                    self.set_action(Action::Move);
                } else if tn >= def.cancel_at + 0.1 && intent.mv.length_squared() > 0.1 && self.queued.is_none() {
                    self.set_action(Action::Move);
                }
            }
            Action::Roll { dir } => {
                self.sprinting = false;
                let speed = combat::sample_curve(&[(0.0, 2.5), (0.08, 6.8), (0.5, 5.6), (0.72, 0.6), (1.0, 0.0)], tn);
                want_vel = dir * speed;
                if tn >= 1.0 {
                    self.set_action(Action::Move);
                }
            }
            Action::Backstep => {
                let speed = combat::sample_curve(&[(0.0, -5.0), (0.3, -4.0), (0.5, -0.5), (1.0, 0.0)], tn);
                want_vel = fwd * speed;
                if tn >= 1.0 {
                    self.set_action(Action::Move);
                }
            }
            Action::Jump { dir } => {
                want_vel = dir * SPRINT_SPEED * 0.95;
                if self.grounded && self.action_t > 0.1 {
                    self.land(ctx);
                }
            }
            Action::Fall => {
                // Keep some air control.
                let carry = flat(self.vel);
                want_vel = carry * 0.98 + intent.mv * 0.5;
                if self.grounded {
                    self.land(ctx);
                }
            }
            Action::Plunge { hit } => {
                want_vel = flat(self.vel) * 0.9;
                if self.grounded {
                    let fell = self.air_top - self.pos.y;
                    self.begin_attack(Atk::LsPlunge, None, None);
                    if hit {
                        // Already struck a foe on the way down: no fall damage.
                        self.hit_list.push(u32::MAX);
                    } else {
                        ctx.emit(ActorEvent::Landed { id: self.id, height: fell });
                    }
                    ctx.emit(ActorEvent::Sound { sfx: Sfx::Land, pos: self.pos, vol: 1.0 });
                    ctx.emit(ActorEvent::Dust { pos: self.pos, size: 1.0 });
                }
            }
            Action::Land { .. } => {
                if tn >= 1.0 {
                    self.set_action(Action::Move);
                }
            }
            Action::Hit { heavy, dir } => {
                let k = if heavy { 2.2 } else { 1.2 };
                want_vel = dir * combat::sample_curve(&[(0.0, k), (0.3, 0.3), (0.5, 0.0)], tn);
                if tn >= 1.0 {
                    self.set_action(Action::Move);
                }
            }
            Action::Knockdown { dir } => {
                let k = if self.body.is_demon() { 0.0 } else { 5.0 };
                want_vel = dir * combat::sample_curve(&[(0.0, k), (0.18, 1.0), (0.25, 0.0)], tn);
                if tn >= 1.0 {
                    self.set_action(Action::Move);
                }
            }
            Action::GuardHit { dir } => {
                want_vel = dir * combat::sample_curve(&[(0.0, 2.8), (0.5, 0.0)], tn);
                if tn >= 1.0 {
                    self.set_action(Action::Move);
                }
            }
            Action::GuardBreak => {
                want_vel = -fwd * combat::sample_curve(&[(0.0, 1.5), (0.3, 0.0)], tn);
                if tn >= 1.0 {
                    self.set_action(Action::Move);
                }
            }
            Action::Drink { healed } => {
                want_vel = intent.mv.clamp_length_max(1.0) * 0.8;
                if !healed && tn >= 0.55 {
                    self.action = Action::Drink { healed: true };
                    ctx.emit(ActorEvent::Healed(self.id));
                }
                if self.action_t > 0.3 && self.action_t - dt <= 0.3 {
                    ctx.emit(ActorEvent::Sound { sfx: Sfx::Drink, pos: self.pos, vol: 0.8 });
                }
                if tn >= 1.0 {
                    self.set_action(Action::Move);
                }
            }
            Action::Use { tag, dur, fired, dir, clip } => {
                if tag == UseTag::FogWalk {
                    want_vel = dir * 1.2;
                }
                let fire_at = match tag {
                    UseTag::Pickup => 0.45,
                    UseTag::PushDoor => 0.35,
                    UseTag::Push => 0.55,
                    _ => 0.95,
                };
                if !fired && tn >= fire_at {
                    self.action = Action::Use { tag, clip, dur, fired: true, dir };
                    ctx.emit(ActorEvent::UseFired { id: self.id, tag });
                }
                if tn >= 1.0 {
                    if tag == UseTag::SitDown {
                        self.set_action(Action::Sitting);
                    } else {
                        self.set_action(Action::Move);
                    }
                }
            }
            Action::Sitting => {}
            Action::Script { .. } => {
                if tn >= 1.0 {
                    self.set_action(Action::Move);
                }
            }
            Action::Death => {}
        }
        if !matches!(self.action, Action::Move) {
            self.sprinting = false;
        }
        self.physics(want_vel, ctx);
        self.animate(ctx, dt);
    }

    fn fire_cues(&mut self, def: &AttackDef, tn: f32, ctx: &mut Ctx) {
        for c in &def.cues {
            if c.t > self.cue_cursor && c.t <= tn {
                match c.kind {
                    CueKind::Sound(s) => ctx.emit(ActorEvent::Sound { sfx: s, pos: self.center(), vol: 1.0 }),
                    CueKind::Impact { sfx, shake, bone } => {
                        let p = match bone {
                            Some(b) => {
                                let m = self.bone_world(b);
                                let tip = m.transform_point3(Vec3::new(0.0, -0.25, 3.0));
                                Vec3::new(tip.x, self.pos.y, tip.z)
                            }
                            None => self.pos,
                        };
                        ctx.emit(ActorEvent::Sound { sfx, pos: p, vol: 1.0 });
                        ctx.emit(ActorEvent::Shake(shake));
                        ctx.emit(ActorEvent::Dust { pos: p, size: 2.5 });
                    }
                }
            }
        }
        self.cue_cursor = tn;
    }

    fn land(&mut self, ctx: &mut Ctx) {
        let fell = self.air_top - self.pos.y;
        let hard = fell > 3.0;
        ctx.emit(ActorEvent::Landed { id: self.id, height: fell });
        ctx.emit(ActorEvent::Sound { sfx: if self.body.is_demon() { Sfx::DemonSlam } else { Sfx::Land }, pos: self.pos, vol: 1.0 });
        if self.body.is_demon() {
            ctx.emit(ActorEvent::Shake(1.0));
            ctx.emit(ActorEvent::Dust { pos: self.pos, size: 3.0 });
        }
        self.set_action(Action::Land { hard });
    }

    fn physics(&mut self, want_vel: Vec3, ctx: &mut Ctx) {
        let dt = ctx.dt;
        let airborne_action = matches!(self.action, Action::Fall | Action::Jump { .. } | Action::Plunge { .. });
        let accel = if airborne_action { 4.0 } else { 30.0 };
        let cur = flat(self.vel);
        let target = flat(want_vel);
        let new_h = if matches!(self.action, Action::Move) {
            let d = target - cur;
            let step = accel * dt * if target.length_squared() < cur.length_squared() { 1.6 } else { 1.0 };
            if d.length() <= step { target } else { cur + d.normalize() * step }
        } else {
            target
        };
        self.vel.x = new_h.x;
        self.vel.z = new_h.z;
        if self.grounded && !airborne_action {
            self.vel.y = -2.0;
        } else {
            self.vel.y = (self.vel.y - GRAVITY * dt).max(-35.0);
        }
        let was = self.grounded;
        let res = ctx.col.move_character(self.pos, self.vel * dt, self.shape, was && self.vel.y <= 0.0);
        self.pos = res.pos;
        self.grounded = res.grounded;
        if res.grounded {
            self.surface = res.ground_surface;
            if self.vel.y < 0.0 {
                self.vel.y = 0.0;
            }
        }
        if res.hit_ceiling && self.vel.y > 0.0 {
            self.vel.y = 0.0;
        }
        if !self.grounded {
            self.air_top = self.air_top.max(self.pos.y);
        } else if was {
            self.air_top = self.pos.y;
        }
        if self.pos.y < ctx.kill_y && !self.dead {
            ctx.emit(ActorEvent::FellToDeath(self.id));
        }
        // Footsteps.
        if self.grounded && matches!(self.action, Action::Move) {
            let sp = flat(self.vel).length();
            if sp > 0.3 {
                let cyc = (self.anims.cycle)(sp).max(0.5);
                let prev = self.step_phase;
                self.step_phase = (self.step_phase + sp * dt / cyc * 2.0) % 1.0;
                if self.step_phase < prev {
                    ctx.emit(ActorEvent::Footstep { pos: self.pos, surface: self.surface, heavy: self.body.is_demon() });
                }
            }
        }
    }

    fn animate(&mut self, ctx: &mut Ctx, dt: f32) {
        let clips = &ctx.assets.clips;
        let a = self.anims;
        let speed = flat(self.vel).length();
        let local = Quat::from_rotation_y(-self.yaw) * flat(self.vel);
        self.params.speed = speed;
        self.params.move_dir = if speed > 0.1 { Vec2::new(local.x, local.z) / speed } else { Vec2::Y };
        let (clip, fade, restart): (&str, f32, bool) = match &self.action {
            Action::Move => {
                if speed > 0.2 {
                    (a.loco, 0.2, false)
                } else {
                    (a.idle, 0.25, false)
                }
            }
            Action::Attack { atk, .. } => (combat::attack(*atk).clip, 0.1, self.action_t <= dt + 1e-4),
            Action::Roll { .. } => (hc::ROLL, 0.06, self.action_t <= dt + 1e-4),
            Action::Backstep => (hc::BACKSTEP, 0.06, self.action_t <= dt + 1e-4),
            Action::Jump { .. } => (hc::JUMP, 0.1, false),
            Action::Fall => (a.fall, 0.25, false),
            Action::Plunge { .. } => (hc::PLUNGE_FALL, 0.12, false),
            Action::Land { hard } => (if *hard && !self.body.is_demon() { hc::LAND_HARD } else { a.land }, 0.05, self.action_t <= dt + 1e-4),
            Action::Hit { heavy, .. } => (if *heavy || self.body.is_demon() { a.hit_heavy } else { a.hit_light }, 0.05, self.action_t <= dt + 1e-4),
            Action::Knockdown { .. } => (a.knockdown, 0.05, self.action_t <= dt + 1e-4),
            Action::GuardHit { .. } => (hc::GUARD_HIT, 0.04, self.action_t <= dt + 1e-4),
            Action::GuardBreak => (hc::GUARD_BREAK, 0.05, false),
            Action::Drink { .. } => (hc::DRINK, 0.15, false),
            Action::Use { clip, .. } => (clip, 0.15, self.action_t <= dt + 1e-4),
            Action::Sitting => (hc::SIT_LOOP, 0.2, false),
            Action::Script { clip, .. } => (clip, 0.15, self.action_t <= dt + 1e-4),
            Action::Death => (a.death, 0.1, false),
        };
        let id = clips.id(clip);
        self.anim.play(id, fade, restart);
        if matches!(self.action, Action::Move) && speed > 0.2 {
            self.anim.set_speed(speed / (a.cycle)(speed));
        } else {
            self.anim.set_speed(1.0);
        }
        // Shield overlay.
        if self.left.is_some() && !self.body.is_demon() {
            if self.block_amount > 0.0 && matches!(self.action, Action::Move) {
                let block = clips.id(hc::BLOCK);
                match self.anim.overlay.as_mut() {
                    Some(o) => o.weight = self.block_amount,
                    None => {
                        self.anim.overlay = Some(Overlay {
                            play: ashen_engine::anim::Playback { clip: block, time: 0.0, speed: 1.0 },
                            weight: self.block_amount,
                            mask: hb::shield_arm_mask(),
                        })
                    }
                }
            } else {
                self.anim.overlay = None;
            }
        }
        self.anim.update(dt, clips);
        let skel = if self.body.is_demon() { &ctx.assets.demon_skel } else { &ctx.assets.human_skel };
        self.anim.evaluate(clips, skel, &self.params, &mut self.pose);
        skel.model_transforms(&self.pose, &mut self.mats);
    }

    /// World-space blade segment of the right-hand weapon.
    pub fn blade(&self) -> Option<(Vec3, Vec3, f32)> {
        let w = combat::weapon(self.right?);
        let (z0, z1, r) = w.blade;
        if r <= 0.0 {
            return None;
        }
        let bone = if self.body.is_demon() { db::R_HAND } else { hb::R_HAND };
        let m = self.bone_world(bone);
        let off = if self.body.is_demon() { Vec3::new(0.0, -0.25, 0.0) } else { hb::FIST };
        Some((m.transform_point3(off + Vec3::Z * z0), m.transform_point3(off + Vec3::Z * z1), r))
    }

    pub fn hurtboxes(&self) -> &'static [combat::Hurtbox] {
        if self.body.is_demon() { combat::demon_hurtboxes() } else { combat::human_hurtboxes() }
    }

    /// Restores the actor to its spawn state (enemy respawn on rest).
    pub fn respawn(&mut self) {
        self.pos = self.spawn_pos;
        self.yaw = self.spawn_yaw;
        self.vel = Vec3::ZERO;
        self.stats.hp = self.stats.max_hp;
        self.stats.stamina = self.stats.max_stamina;
        self.stats.poise = self.stats.max_poise;
        self.dead = false;
        self.dissolve = 0.0;
        self.remove = false;
        self.grounded = true;
        self.lock = None;
        self.queued = None;
        self.blocking = false;
        self.block_amount = 0.0;
        self.set_action(Action::Move);
    }
}

/// Shared context for actor updates.
pub struct Ctx<'a> {
    pub dt: f32,
    pub col: &'a CollisionWorld,
    pub assets: &'a Assets,
    pub events: &'a mut Vec<ActorEvent>,
    pub face_target: Option<Vec3>,
    pub flasks_available: bool,
    pub used_flask: bool,
    pub kill_y: f32,
}

impl Ctx<'_> {
    pub fn emit(&mut self, e: ActorEvent) {
        self.events.push(e);
    }
}
