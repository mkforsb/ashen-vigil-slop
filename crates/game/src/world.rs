//! The running game world: actors, props, combat resolution and events.
//! Scripted sequences and interactions live in `script.rs`.

use crate::actor::*;
use crate::ai::{ActorView, ArcherBrain, Bark, Brain, DemonBrain, DemonPhase, HuskBrain, ThinkCtx};
use crate::anims::humanoid::clip as hc;
use crate::assets::Assets;
use crate::assets::sounds::Sfx;
use crate::asylum::{MapDef, Named, SpawnDef, SpawnKind, points};
use crate::camera::CameraRig;
use crate::combat::{self, HitShape, HitWindow, Knock, ShieldKind, WeaponKind};
use crate::fx::Fx;
use crate::hud::{Banner, BannerKind};
use crate::input::{Button, InputTracker};
use crate::level::{EnvKind, LevelLight, Zone, surface};
use crate::player::PlayerCtl;
use crate::props::*;
use crate::save::Progress;
use ashen_engine::collision::CollisionWorld;
use ashen_engine::math::*;
use ashen_engine::render::{Environment, MeshHandle};
use ashen_engine::rng::Rng;

/// Immutable level data shared by every world instance (new game, reload).
pub struct MapTemplate {
    pub col: CollisionWorld,
    pub chunks: Vec<(MeshHandle, Aabb)>,
    pub lights: Vec<LevelLight>,
    pub zones: Vec<Zone>,
    pub props: Vec<Prop>,
    pub spawns: Vec<SpawnDef>,
    pub player_start: (Vec3, f32),
    pub named: Named,
    pub kill_y: f32,
}

impl MapTemplate {
    pub fn new(map: MapDef, assets: &mut Assets) -> Self {
        let chunks = map.geo.chunks.into_iter().map(|(m, b)| (assets.add_mesh(m), b)).collect();
        MapTemplate {
            col: map.geo.col,
            chunks,
            lights: map.geo.lights,
            zones: map.geo.zones,
            props: map.props,
            spawns: map.spawns,
            player_start: map.player_start,
            named: map.named,
            kill_y: map.kill_y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Encounter {
    None,
    /// The demon is dropping in; `t` since start.
    Intro { t: f32 },
    Active,
    Won { t: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropMotion {
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
    pub dur: f32,
    pub apex: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoulderState {
    Idle,
    Arming(f32),
    Rolling,
    Done,
}

#[derive(Clone, Copy, Debug)]
pub struct Boulder {
    pub state: BoulderState,
    pub pos: Vec3,
    pub speed: f32,
    pub roll: f32,
    pub hit_cd: f32,
}

pub const BOULDER_R: f32 = 0.95;

#[derive(Clone, Debug, PartialEq)]
pub enum WorldState {
    Playing,
    Reading(&'static str),
    Dialog { lines: &'static [&'static str], idx: usize, reward: bool },
    Resting,
    Dead { t: f32 },
    Ending { t: f32 },
}

/// One-shot sound with precomputed spatialisation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundOut {
    pub sfx: Sfx,
    pub gain: f32,
    pub pan: f32,
    pub rate: f32,
}

pub struct World {
    pub col: CollisionWorld,
    pub chunks: Vec<(MeshHandle, Aabb)>,
    pub lights: Vec<LevelLight>,
    pub zones: Vec<Zone>,
    pub props: Vec<Prop>,
    pub named: Named,
    pub kill_y: f32,
    pub actors: Vec<Actor>,
    pub demon: Option<ActorId>,
    pub corvin: Option<ActorId>,
    pub pusher: Option<ActorId>,
    pub ctl: PlayerCtl,
    pub camera: CameraRig,
    pub projectiles: Vec<Projectile>,
    pub fx: Fx,
    pub progress: Progress,
    pub state: WorldState,
    pub encounter: Encounter,
    pub demon_drop: Option<DropMotion>,
    pub boulder: Boulder,
    pub time: f32,
    pub rng: Rng,
    pub env: Environment,
    pub env_kind: EnvKind,
    pub sounds: Vec<SoundOut>,
    pub music_gain: f32,
    pub banner: Option<(Banner, f32)>,
    pub popups: Vec<(String, f32)>,
    pub prompt: Option<String>,
    pub save_dirty: bool,
    pub hp_lag: f32,
    pub boss_lag: f32,
    pub fade: f32,
    pub fog_pass: Option<usize>,
    pub interact_target: Option<Interact>,
    pub ledge_timer: f32,
    pub area_shown: bool,
    pub debug: bool,
    pub events: Vec<ActorEvent>,
    /// Prop awaiting the player's interaction animation (door, item).
    pub pending: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Interact {
    Prop(usize),
    Npc(ActorId),
    Bloodstain,
}

pub const PLAYER: usize = 0;

fn env_preset(k: EnvKind) -> Environment {
    match k {
        EnvKind::Dungeon => Environment {
            ambient: Vec3::new(0.13, 0.12, 0.12),
            sun_dir: Vec3::new(0.3, -1.0, 0.2),
            sun_color: Vec3::ZERO,
            fog_color: Vec3::new(0.015, 0.014, 0.018),
            fog_near: 5.0,
            fog_far: 30.0,
        },
        EnvKind::Cell => Environment {
            ambient: Vec3::new(0.10, 0.10, 0.12),
            sun_dir: Vec3::new(0.1, -1.0, 0.1),
            sun_color: Vec3::new(0.05, 0.06, 0.08),
            fog_color: Vec3::new(0.01, 0.01, 0.015),
            fog_near: 4.0,
            fog_far: 24.0,
        },
        EnvKind::Courtyard => Environment {
            ambient: Vec3::new(0.30, 0.31, 0.34),
            sun_dir: Vec3::new(0.45, -1.0, 0.35),
            sun_color: Vec3::new(0.36, 0.36, 0.38),
            fog_color: Vec3::new(0.28, 0.29, 0.31),
            fog_near: 16.0,
            fog_far: 70.0,
        },
        EnvKind::Arena => Environment {
            ambient: Vec3::new(0.30, 0.30, 0.33),
            sun_dir: Vec3::new(-0.35, -1.0, 0.45),
            sun_color: Vec3::new(0.40, 0.39, 0.40),
            fog_color: Vec3::new(0.30, 0.31, 0.33),
            fog_near: 20.0,
            fog_far: 80.0,
        },
        EnvKind::Outside => Environment {
            ambient: Vec3::new(0.36, 0.37, 0.40),
            sun_dir: Vec3::new(-0.4, -0.9, 0.3),
            sun_color: Vec3::new(0.50, 0.48, 0.45),
            fog_color: Vec3::new(0.42, 0.43, 0.46),
            fog_near: 18.0,
            fog_far: 85.0,
        },
    }
}

fn lerp_env(a: &Environment, b: &Environment, t: f32) -> Environment {
    Environment {
        ambient: a.ambient.lerp(b.ambient, t),
        sun_dir: a.sun_dir.lerp(b.sun_dir, t),
        sun_color: a.sun_color.lerp(b.sun_color, t),
        fog_color: a.fog_color.lerp(b.fog_color, t),
        fog_near: lerp(a.fog_near, b.fog_near, t),
        fog_far: lerp(a.fog_far, b.fog_far, t),
    }
}

impl World {
    pub fn new(map: &MapTemplate, assets: &Assets, progress: Option<Progress>) -> Self {
        let mut w = World {
            col: map.col.clone(),
            chunks: map.chunks.clone(),
            lights: map.lights.clone(),
            zones: map.zones.clone(),
            props: map.props.clone(),
            named: map.named,
            kill_y: map.kill_y,
            actors: Vec::new(),
            demon: None,
            corvin: None,
            pusher: None,
            ctl: PlayerCtl::default(),
            camera: CameraRig::default(),
            projectiles: Vec::new(),
            fx: Fx::default(),
            progress: progress.clone().unwrap_or_default(),
            state: WorldState::Playing,
            encounter: Encounter::None,
            demon_drop: None,
            boulder: Boulder { state: BoulderState::Idle, pos: points::BOULDER_START + Vec3::Y * BOULDER_R, speed: 0.0, roll: 0.0, hit_cd: 0.0 },
            time: 0.0,
            rng: Rng::new(0xA5E4),
            env: env_preset(EnvKind::Cell),
            env_kind: EnvKind::Cell,
            sounds: Vec::new(),
            music_gain: 0.0,
            banner: None,
            popups: Vec::new(),
            prompt: None,
            save_dirty: false,
            hp_lag: 0.0,
            boss_lag: 1.0,
            fade: 1.0,
            fog_pass: None,
            interact_target: None,
            ledge_timer: 0.0,
            area_shown: false,
            debug: false,
            events: Vec::new(),
            pending: None,
        };
        // Player.
        let (start, yaw) = map.player_start;
        let mut p = Actor::new(0, "Knight", BodyKind::Knight, Team::Player, start, yaw, assets);
        p.equip(Some(WeaponKind::Longsword), Some(ShieldKind::KiteShield), assets);
        p.brain = Brain::Player;
        w.actors.push(p);
        for s in &map.spawns {
            w.spawn(*s, assets);
        }
        w.apply_progress(assets, progress.is_some());
        let pp = w.actors[PLAYER].pos;
        let py = w.actors[PLAYER].yaw;
        w.camera.reset_behind(py, pp + Vec3::Y * 1.55);
        w.hp_lag = w.actors[PLAYER].stats.hp;
        w
    }

    fn spawn(&mut self, s: SpawnDef, assets: &Assets) {
        let id = self.actors.len() as ActorId;
        let a = match s.kind {
            SpawnKind::Husk | SpawnKind::PassiveHusk | SpawnKind::Pusher => {
                let body = if s.kind == SpawnKind::PassiveHusk { BodyKind::HuskPale } else { BodyKind::Husk };
                let mut a = Actor::new(id, "Husk", body, Team::Hostile, s.pos, s.yaw, assets);
                match s.kind {
                    SpawnKind::Husk => {
                        a.equip(Some(WeaponKind::BrokenSword), None, assets);
                        a.brain = Brain::Husk(HuskBrain::new(s.pos, false));
                        a.souls = 40;
                    }
                    SpawnKind::PassiveHusk => {
                        a.brain = Brain::Husk(HuskBrain::new(s.pos, true));
                        a.anims.idle = hc::HUSK_CROUCH;
                        a.souls = 15;
                        a.stats = Stats::husk(60.0);
                    }
                    _ => {
                        a.brain = Brain::Pusher { triggered: false };
                        a.souls = 40;
                        self.pusher = Some(id);
                    }
                }
                a
            }
            SpawnKind::Archer => {
                let mut a = Actor::new(id, "Husk Archer", BodyKind::Husk, Team::Hostile, s.pos, s.yaw, assets);
                a.equip(Some(WeaponKind::Bow), None, assets);
                a.brain = Brain::Archer(ArcherBrain { cd: 1.0, aim: 0.0, range: 22.0, aware: false });
                a.anims.idle = hc::ARCHER_AIM;
                a.anims.loco = hc::ARCHER_AIM;
                a.stats = Stats::husk(80.0);
                a.souls = 50;
                a
            }
            SpawnKind::Demon => {
                let mut a = Actor::new(id, "Warden Demon", BodyKind::Demon, Team::Hostile, s.pos, s.yaw, assets);
                a.equip(Some(WeaponKind::DemonClub), None, assets);
                a.brain = Brain::Demon(DemonBrain::new(DemonPhase::Perched));
                a.souls = 2000;
                self.demon = Some(id);
                a
            }
            SpawnKind::Corvin => {
                let mut a = Actor::new(id, "Sir Corvin", BodyKind::Corvin, Team::Neutral, s.pos, s.yaw, assets);
                a.equip(Some(WeaponKind::Longsword), Some(ShieldKind::KiteShield), assets);
                a.brain = Brain::Npc;
                a.anims.idle = hc::NPC_SLUMP;
                a.anims.loco = hc::NPC_SLUMP;
                a.invulnerable = true;
                self.corvin = Some(id);
                a
            }
        };
        self.actors.push(a);
    }

    pub fn player(&self) -> &Actor {
        &self.actors[PLAYER]
    }

    pub fn actor(&self, id: ActorId) -> Option<&Actor> {
        self.actors.get(id as usize)
    }

    // ------------------------------------------------------------------ Sound

    pub fn sound_at(&mut self, sfx: Sfx, pos: Vec3, vol: f32) {
        let cam = self.camera.pos;
        let fwd = self.camera.forward();
        let right = Vec3::new(-fwd.z, 0.0, fwd.x).normalize_or(Vec3::X);
        let d = pos - cam;
        let dist = d.length();
        let far = match sfx {
            Sfx::DemonRoar | Sfx::DemonSlam | Sfx::DemonDeath | Sfx::GateSlam | Sfx::WallCrash | Sfx::BoulderRoll => 70.0,
            Sfx::StepStone | Sfx::StepArmor | Sfx::StepHusk | Sfx::StepWater => 18.0,
            _ => 35.0,
        };
        let near = 2.5;
        let att = (1.0 - ((dist - near) / (far - near)).clamp(0.0, 1.0)).powf(1.6);
        if att <= 0.01 {
            return;
        }
        // Muffle sounds behind walls a little.
        let occl = if dist > 3.0 && !self.col.line_of_sight(cam, pos + Vec3::Y * 0.5) { 0.55 } else { 1.0 };
        let pan = if dist > 0.5 { (d / dist).dot(right) * 0.75 } else { 0.0 };
        let j = sfx.pitch_jitter();
        let rate = 1.0 + self.rng.signed() * j;
        self.sounds.push(SoundOut { sfx, gain: vol * att * occl, pan, rate });
    }

    pub fn sound_2d(&mut self, sfx: Sfx, vol: f32) {
        self.sounds.push(SoundOut { sfx, gain: vol, pan: 0.0, rate: 1.0 });
    }

    pub fn show_banner(&mut self, text: &str, kind: BannerKind, dur: f32) {
        self.banner = Some((Banner { text: text.to_string(), kind, alpha: 0.0 }, dur));
    }

    pub fn popup(&mut self, text: String) {
        self.popups.push((text, 4.0));
        if self.popups.len() > 4 {
            self.popups.remove(0);
        }
    }

    // ------------------------------------------------------------- Main loop

    pub fn update(&mut self, dt: f32, inp: &InputTracker, assets: &Assets) {
        self.time += dt;
        self.progress.playtime += dt;
        self.sounds.clear();
        let was_playing = self.state == WorldState::Playing;
        self.update_ui_state(dt, inp, assets);

        // Player intent.
        let lock_pos = self.lock_target_pos();
        let mut intents = vec![Intent::default(); self.actors.len()];
        let mut extras = vec![crate::ai::Extra::default(); self.actors.len()];
        // A press that closed a menu this tick must not also act in the world.
        let controllable = was_playing && matches!(self.state, WorldState::Playing);
        if controllable {
            if inp.pressed(Button::LockOn) {
                self.toggle_lock(assets);
            }
            let cam_yaw = self.camera.yaw;
            let p = &self.actors[PLAYER];
            intents[PLAYER] = self.ctl.intent(inp, p, cam_yaw, lock_pos, dt);
            if inp.pressed(Button::Interact) {
                self.try_interact(assets);
            }
            if matches!(self.actors[PLAYER].action, Action::Sitting) && (inp.pressed(Button::Back) || inp.state.move_axis.length() > 0.5) {
                self.stand_up();
            }
        } else if matches!(self.state, WorldState::Resting | WorldState::Dialog { .. } | WorldState::Reading(_)) {
            // Allow nothing but blocking stance while in menus.
            intents[PLAYER].face = lock_pos;
        }

        // AI.
        let views: Vec<ActorView> = self.actors.iter().map(view_of).collect();
        let player_view = Some(views[PLAYER]).filter(|_| !matches!(self.state, WorldState::Dead { .. }));
        let mut barks = Vec::new();
        {
            let mut tctx = ThinkCtx { dt, col: &self.col, rng: &mut self.rng, player: player_view, barks: &mut barks };
            for i in 1..self.actors.len() {
                if self.actors[i].dead {
                    continue;
                }
                let mut brain = std::mem::replace(&mut self.actors[i].brain, Brain::None);
                let (it, ex) = brain.think(&views[i], &self.actors[i].action, &mut tctx);
                self.actors[i].brain = brain;
                intents[i] = it;
                extras[i] = ex;
            }
        }
        for (id, b) in barks {
            let pos = self.actors[id as usize].center();
            match b {
                Bark::Alert => {
                    if !self.actors[id as usize].body.is_demon() {
                        self.sound_at(Sfx::HuskGroan, pos, 0.7);
                    }
                }
                Bark::Shoot => {}
            }
        }

        // Actors.
        let flasks_available = self.progress.flasks > 0;
        let mut events = Vec::new();
        let mut used_flask = false;
        for i in 0..self.actors.len() {
            if self.actors[i].remove {
                continue;
            }
            let face = intents[i].face;
            let mut ctx = Ctx {
                dt,
                col: &self.col,
                assets,
                events: &mut events,
                face_target: face,
                flasks_available: i == PLAYER && flasks_available,
                used_flask: false,
                kill_y: self.kill_y,
            };
            self.actors[i].update(&intents[i], &mut ctx);
            if i == PLAYER && ctx.used_flask {
                used_flask = true;
            }
        }
        if used_flask {
            self.progress.flasks = self.progress.flasks.saturating_sub(1);
        }
        // Archers shoot.
        for (i, ex) in extras.iter().enumerate() {
            if let Some(target) = ex.shoot_at {
                self.fire_arrow(i, target);
            }
        }

        self.events.extend(events);
        self.separate();
        self.resolve_hits();
        self.update_plunge();
        self.update_projectiles(dt);
        // Handlers may raise follow-up events (e.g. a fatal fall -> death).
        for _ in 0..8 {
            if self.events.is_empty() {
                break;
            }
            for e in std::mem::take(&mut self.events) {
                self.handle_event(e, assets);
            }
        }

        // A veil walk that got interrupted (hit, death) closes the veil again.
        if let Some(i) = self.fog_pass {
            let walking = matches!(self.actors[PLAYER].action, Action::Use { tag: UseTag::FogWalk, .. });
            if !walking {
                self.fog_pass = None;
                if let PropKind::Fog(f) = &mut self.props[i].kind {
                    f.passing = false;
                }
            }
        }
        self.update_scripts(dt, assets);
        for i in 0..self.props.len() {
            if let Some(PropEvent::Slammed(p)) = self.props[i].update(dt, &mut self.col) {
                self.sound_at(Sfx::GateSlam, p, 1.0);
                self.camera.add_shake(0.3);
            }
        }
        self.update_ambient_fx(dt);
        self.fx.update(dt);

        // Camera.
        let look = if controllable { inp.state.look } else { Vec2::ZERO };
        let lock = self.lock_target_pos();
        let ppos = self.actors[PLAYER].pos;
        self.camera.update(dt, ppos, lock, look, &self.col);

        // Environment blend.
        let probe = ppos + Vec3::Y;
        let kind = self.zones.iter().find(|z| z.bounds.contains(probe)).map(|z| z.env).unwrap_or(self.env_kind);
        self.env_kind = kind;
        let target = env_preset(kind);
        self.env = lerp_env(&self.env, &target, 1.0 - (-2.5 * dt).exp());

        // HUD trails.
        let hp = self.actors[PLAYER].stats.hp;
        self.hp_lag = if hp < self.hp_lag { approach(self.hp_lag, hp, dt * 120.0 * if self.hp_lag - hp > 1.0 { 1.0 } else { 0.0 } + dt * 10.0) } else { hp };
        if let Some(d) = self.demon.and_then(|id| self.actor(id)) {
            let f = d.stats.hp / d.stats.max_hp;
            self.boss_lag = if f < self.boss_lag { approach(self.boss_lag, f, dt * 0.25) } else { f };
        }
        if let Some((_, t)) = self.banner.as_mut() {
            *t -= dt;
        }
        if self.banner.as_ref().is_some_and(|(_, t)| *t <= 0.0) {
            self.banner = None;
        }
        for p in self.popups.iter_mut() {
            p.1 -= dt;
        }
        self.popups.retain(|p| p.1 > 0.0);
        self.fade = match self.state {
            WorldState::Dead { t } => smoothstep(3.0, 4.5, t),
            WorldState::Ending { t } => smoothstep(0.5, 3.0, t),
            _ => (self.fade - dt * 1.2).max(0.0),
        };
        self.update_prompt();
        // Unlock if the target died or wandered off.
        if let Some(t) = self.actors[PLAYER].lock {
            let ok = self.actor(t).is_some_and(|a| !a.dead && (a.pos - ppos).length() < 28.0);
            if !ok {
                self.actors[PLAYER].lock = None;
            }
        }
    }

    pub fn lock_target_pos(&self) -> Option<Vec3> {
        let id = self.actors[PLAYER].lock?;
        let a = self.actor(id)?;
        Some(if a.body.is_demon() { a.bone_world(crate::rig::demon::CHEST).transform_point3(Vec3::new(0.0, 0.3, 0.2)) } else { a.center() })
    }

    fn toggle_lock(&mut self, _assets: &Assets) {
        if self.actors[PLAYER].lock.is_some() {
            self.actors[PLAYER].lock = None;
            return;
        }
        let p = self.actors[PLAYER].center();
        let fwd = flat(self.camera.forward()).normalize_or(Vec3::Z);
        let mut best: Option<(f32, ActorId)> = None;
        for a in self.actors.iter().skip(1) {
            if a.dead || a.team != Team::Hostile || a.remove {
                continue;
            }
            let c = a.center();
            let d = c - p;
            let dist = d.length();
            if dist > 22.0 {
                continue;
            }
            let ang = flat(d).normalize_or(fwd).dot(fwd).clamp(-1.0, 1.0).acos();
            if ang > deg(60.0) && dist > 3.0 {
                continue;
            }
            if !self.col.line_of_sight(p, c) {
                continue;
            }
            let score = ang * 8.0 + dist * 0.3;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, a.id));
            }
        }
        match best {
            Some((_, id)) => self.actors[PLAYER].lock = Some(id),
            None => {
                // No target: snap the camera behind the player.
                let yaw = self.actors[PLAYER].yaw;
                self.camera.yaw = yaw;
                self.camera.pitch = 0.25;
            }
        }
    }

    // ------------------------------------------------------------ Physics

    /// Pushes overlapping characters apart (heavier ones move less) and keeps
    /// them out of intact urns.
    fn separate(&mut self) {
        let n = self.actors.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (&self.actors[i], &self.actors[j]);
                if a.dead || b.dead || a.remove || b.remove {
                    continue;
                }
                // Vertical overlap.
                let (a0, a1) = (a.pos.y, a.pos.y + a.shape.height);
                let (b0, b1) = (b.pos.y, b.pos.y + b.shape.height);
                if a1 < b0 || b1 < a0 {
                    continue;
                }
                let d = flat(b.pos - a.pos);
                let min = a.shape.radius + b.shape.radius;
                let dist = d.length();
                if dist >= min || dist < 1e-4 {
                    continue;
                }
                let push = d / dist * (min - dist);
                let (ma, mb) = (a.mass, b.mass);
                let wa = mb / (ma + mb);
                let wb = ma / (ma + mb);
                let pa = self.actors[i].pos - push * wa;
                let pb = self.actors[j].pos + push * wb;
                // Validate against walls.
                let sa = self.actors[i].shape;
                let sb = self.actors[j].shape;
                let ga = self.actors[i].grounded;
                let gb = self.actors[j].grounded;
                self.actors[i].pos = self.col.move_character(self.actors[i].pos, pa - self.actors[i].pos, sa, ga).pos;
                self.actors[j].pos = self.col.move_character(self.actors[j].pos, pb - self.actors[j].pos, sb, gb).pos;
            }
        }
        // Urns.
        let mut broke = Vec::new();
        for (pi, prop) in self.props.iter().enumerate() {
            let PropKind::Urn { broken: false, .. } = prop.kind else { continue };
            for a in self.actors.iter_mut() {
                if a.dead || (a.pos.y - prop.pos.y).abs() > 1.0 {
                    continue;
                }
                let d = flat(a.pos - prop.pos);
                let min = a.shape.radius + 0.32;
                let dist = d.length();
                if dist < min {
                    let smashing = a.body.is_demon() || matches!(a.action, Action::Roll { .. }) || a.sprinting || flat(a.vel).length() > 4.5;
                    if smashing {
                        broke.push(pi);
                    } else if dist > 1e-4 {
                        a.pos += d / dist * (min - dist);
                    }
                }
            }
        }
        for pi in broke {
            self.break_urn(pi);
        }
    }

    pub fn break_urn(&mut self, pi: usize) {
        if let PropKind::Urn { broken, .. } = &mut self.props[pi].kind {
            if !*broken {
                *broken = true;
                let p = self.props[pi].pos;
                self.fx.shards(p);
                self.sound_at(Sfx::UrnBreak, p, 0.9);
            }
        }
    }

    // ------------------------------------------------------------ Combat

    /// World-space capsules for an attacker's active hit windows.
    fn hit_shapes(&self, a: &Actor) -> Vec<(Capsule, HitWindow)> {
        let mut out = Vec::new();
        let Action::Attack { atk, .. } = &a.action else { return out };
        let def = combat::attack(*atk);
        let tn = a.action_tn();
        for w in &def.windows {
            if tn < w.start || tn > w.end {
                continue;
            }
            match w.shape {
                HitShape::Weapon => {
                    if let Some((b0, b1, r)) = a.blade() {
                        out.push((Capsule::new(b0, b1, r), *w));
                        if let Some((p0, p1)) = a.prev_blade {
                            out.push((Capsule::new(p1, b1, r), *w));
                            out.push((Capsule::new((p0 + p1) * 0.5, (b0 + b1) * 0.5, r), *w));
                        }
                    }
                }
                HitShape::Bone { bone, a: pa, b: pb, r } => {
                    let m = a.bone_world(bone);
                    out.push((Capsule::new(m.transform_point3(pa), m.transform_point3(pb), r), *w));
                }
                HitShape::Sphere { center, r } => {
                    let c = a.world().transform_point3(center) - Vec3::Y * a.lift;
                    out.push((Capsule::new(c, c, r), *w));
                }
            }
        }
        out
    }

    fn hurt_capsules(&self, t: &Actor) -> Vec<Capsule> {
        t.hurtboxes()
            .iter()
            .map(|h| {
                let m = t.bone_world(h.bone);
                Capsule::new(m.transform_point3(h.a), m.transform_point3(h.b), h.r)
            })
            .collect()
    }

    fn hostile(a: &Actor, b: &Actor) -> bool {
        match (a.team, b.team) {
            (Team::Player, Team::Hostile) | (Team::Hostile, Team::Player) => true,
            _ => false,
        }
    }

    fn resolve_hits(&mut self) {
        let mut hits: Vec<(usize, usize, HitWindow, Vec3)> = Vec::new();
        let mut urn_hits = Vec::new();
        for ai in 0..self.actors.len() {
            let a = &self.actors[ai];
            if a.dead || a.hitstop > 0.0 {
                continue;
            }
            let shapes = self.hit_shapes(a);
            if shapes.is_empty() {
                continue;
            }
            for ti in 0..self.actors.len() {
                let t = &self.actors[ti];
                if ti == ai || t.dead || t.remove || !Self::hostile(a, t) || a.hit_list.contains(&t.id) {
                    continue;
                }
                // Cheap reject.
                if (t.center() - a.center()).length() > 14.0 {
                    continue;
                }
                let hurt = self.hurt_capsules(t);
                'found: for (cap, w) in &shapes {
                    for h in &hurt {
                        if let Some(p) = cap.intersects(h) {
                            hits.push((ai, ti, *w, p));
                            break 'found;
                        }
                    }
                }
            }
            if a.team == Team::Player || a.body.is_demon() {
                for (pi, prop) in self.props.iter().enumerate() {
                    if let PropKind::Urn { broken: false, .. } = prop.kind {
                        let c = Capsule::new(prop.pos + Vec3::Y * 0.2, prop.pos + Vec3::Y * 0.8, 0.3);
                        if shapes.iter().any(|(cap, _)| cap.intersects(&c).is_some()) {
                            urn_hits.push(pi);
                        }
                    }
                }
            }
        }
        for pi in urn_hits {
            self.break_urn(pi);
        }
        for (ai, ti, w, p) in hits {
            if self.actors[ti].has_iframes() || self.actors[ai].hit_list.contains(&self.actors[ti].id) {
                continue;
            }
            let tid = self.actors[ti].id;
            self.actors[ai].hit_list.push(tid);
            self.apply_hit(ai, ti, &w, p);
        }
        // Remember blade positions for swept tests next tick.
        for a in self.actors.iter_mut() {
            a.prev_blade = if matches!(a.action, Action::Attack { .. }) { a.blade().map(|(b0, b1, _)| (b0, b1)) } else { None };
        }
    }

    fn apply_hit(&mut self, ai: usize, ti: usize, w: &HitWindow, point: Vec3) {
        let atk_pos = self.actors[ai].pos;
        let atk_power = self.actors[ai].right.map(|k| combat::weapon(k).damage).unwrap_or(1.0);
        let raw = atk_power * w.mv;
        let attacker_demon = self.actors[ai].body.is_demon();
        let attacker_husk = matches!(self.actors[ai].body, BodyKind::Husk | BodyKind::HuskPale);
        let t = &mut self.actors[ti];
        let to_att = flat(atk_pos - t.pos).normalize_or(t.forward());
        let facing = t.forward().dot(to_att) > deg(75.0).cos();
        let blocked = t.blocking && !w.unblockable && facing && t.left.is_some();
        let target_demon = t.body.is_demon();
        let target_player = t.team == Team::Player;
        let dir = -to_att;
        if blocked {
            let sh = combat::shield(t.left.unwrap());
            let drain = w.stamina * (1.0 - sh.stability / 100.0) + 6.0;
            t.stats.stamina -= drain;
            t.stamina_delay = 0.9;
            let leak = raw * (1.0 - sh.absorb);
            if t.stats.stamina <= 0.0 {
                t.stats.stamina = 0.0;
                let chip = combat::mitigate(raw * 0.25, t.stats.absorb);
                t.stats.hp -= chip;
                t.set_action(Action::GuardBreak);
                t.blocking = false;
                t.hitstop = 0.08;
                self.sound_at(Sfx::GuardBreak, point, 1.0);
            } else {
                if leak > 0.5 {
                    t.stats.hp -= combat::mitigate(leak, t.stats.absorb);
                }
                t.set_action(Action::GuardHit { dir });
                t.hitstop = 0.06;
                self.sound_at(Sfx::Block, point, 1.0);
            }
            self.fx.sparks(point, -dir, 12);
            if attacker_husk {
                // Weak weapons bounce off a raised shield.
                self.actors[ai].set_action(Action::Hit { heavy: false, dir: to_att });
            } else {
                self.actors[ai].hitstop = 0.05;
            }
            if attacker_demon {
                self.camera.add_shake(0.5);
            }
        } else {
            let dmg = combat::mitigate(raw, t.stats.absorb);
            t.stats.hp -= dmg;
            t.flash = 1.0;
            t.last_hit_by = Some(ai as ActorId);
            let stop = (0.05 + dmg / 3000.0).min(0.14);
            t.hitstop = stop;
            t.react(w.knock, w.poise, atk_pos);
            if !attacker_demon {
                self.actors[ai].hitstop = stop;
            }
            let sfx = if target_demon {
                Sfx::HitDemon
            } else if target_player {
                Sfx::HitArmor
            } else {
                Sfx::HitFlesh
            };
            self.sound_at(sfx, point, 1.0);
            self.fx.blood(point, dir, if target_demon { 18 } else { 10 });
            if target_player {
                self.sound_at(Sfx::Hurt, point, 0.7);
                self.camera.add_shake(if attacker_demon { 0.8 } else { 0.25 });
            }
            if target_demon && self.rng.chance(0.3) {
                let c = self.actors[ti].center();
                self.sound_at(Sfx::DemonHurt, c, 0.6);
            }
            if self.actors[ti].stats.hp <= 0.0 {
                self.kill_actor(ti);
            }
        }
    }

    pub fn kill_actor(&mut self, ti: usize) {
        let a = &mut self.actors[ti];
        if a.dead {
            return;
        }
        a.dead = true;
        a.stats.hp = 0.0;
        a.blocking = false;
        a.set_action(Action::Death);
        let id = a.id;
        self.events.push(ActorEvent::Died(id));
    }

    /// Plunging attacks: falling with the sword pointed down onto a foe.
    fn update_plunge(&mut self) {
        let p = &self.actors[PLAYER];
        let Action::Plunge { hit: false } = p.action else { return };
        if p.vel.y > -3.0 {
            return;
        }
        let pc = p.pos + Vec3::Y * 0.3;
        let blade = p.blade();
        let mut target = None;
        for (i, t) in self.actors.iter().enumerate().skip(1) {
            if t.dead || t.team != Team::Hostile {
                continue;
            }
            let near = flat(t.pos - p.pos).length() < t.shape.radius + 0.6 && pc.y > t.pos.y + 1.0 && pc.y < t.pos.y + t.shape.height + 2.0;
            let touching = blade.is_some_and(|(b0, b1, r)| {
                let c = Capsule::new(b0, b1 - Vec3::Y * 0.6, r + 0.15);
                self.hurt_capsules(t).iter().any(|h| c.intersects(h).is_some())
            });
            if near || touching {
                target = Some(i);
                break;
            }
        }
        let Some(ti) = target else { return };
        let w = combat::attack(combat::Atk::LsPlunge).windows[0];
        let raw = combat::weapon(WeaponKind::Longsword).damage * w.mv;
        let point = self.actors[PLAYER].pos + Vec3::Y * 0.5;
        self.actors[PLAYER].action = Action::Plunge { hit: true };
        self.actors[PLAYER].hitstop = 0.15;
        let t = &mut self.actors[ti];
        let dmg = combat::mitigate(raw, t.stats.absorb);
        t.stats.hp -= dmg;
        t.flash = 1.0;
        t.hitstop = 0.15;
        // A plunge always breaks poise.
        t.stats.poise = 0.0;
        t.react(Knock::Knockdown, 999.0, point - Vec3::Z);
        let is_demon = t.body.is_demon();
        self.sound_at(if is_demon { Sfx::HitDemon } else { Sfx::HitFlesh }, point, 1.0);
        if is_demon {
            let c = self.actors[ti].center();
            self.sound_at(Sfx::DemonHurt, c, 1.0);
        }
        self.fx.blood(point, Vec3::NEG_Y, 30);
        self.camera.add_shake(0.9);
        if self.actors[ti].stats.hp <= 0.0 {
            self.kill_actor(ti);
        }
    }

    fn fire_arrow(&mut self, i: usize, target: Vec3) {
        let a = &mut self.actors[i];
        if a.dead || !matches!(a.action, Action::Move) {
            return;
        }
        a.set_action(Action::Script { clip: hc::ARCHER_SHOOT, dur: hc::ARCHER_SHOOT_DUR });
        let from = a.bone_world(crate::rig::humanoid::L_HAND).transform_point3(crate::rig::humanoid::FIST) + Vec3::Y * 0.05;
        let flight = (target - from).length() / 22.0;
        let mut vel = (target - from) / flight.max(0.05);
        vel.y += 0.5 * 6.0 * flight; // compensate gravity
        let owner = a.id;
        self.projectiles.push(Projectile { pos: from, vel, owner, damage: 55.0, stuck: None, alive: true });
        self.sound_at(Sfx::ArrowShoot, from, 0.9);
    }

    fn update_projectiles(&mut self, dt: f32) {
        let mut hits = Vec::new();
        for (k, pr) in self.projectiles.iter_mut().enumerate() {
            if let Some(t) = pr.stuck.as_mut() {
                *t -= dt;
                if *t <= 0.0 {
                    pr.alive = false;
                }
                continue;
            }
            pr.vel.y -= 6.0 * dt;
            let step = pr.vel * dt;
            if let Some(h) = self.col.raycast(pr.pos, step, 1.0) {
                pr.pos = h.point;
                pr.stuck = Some(3.0);
                hits.push((k, None, h.point));
                continue;
            }
            let next = pr.pos + step;
            let seg = Capsule::new(pr.pos, next, 0.05);
            pr.pos = next;
            let p = &self.actors[PLAYER];
            if !p.dead && !p.has_iframes() && (p.center() - next).length() < 2.0 {
                let body = Capsule::new(p.pos + Vec3::Y * 0.2, p.pos + Vec3::Y * 1.7, 0.35);
                if seg.intersects(&body).is_some() {
                    pr.alive = false;
                    hits.push((k, Some(PLAYER), next));
                }
            }
            if pr.pos.y < self.kill_y {
                pr.alive = false;
            }
        }
        for (k, who, point) in hits {
            match who {
                None => self.sound_at(Sfx::ArrowHit, point, 0.6),
                Some(ti) => {
                    let w = HitWindow { start: 0.0, end: 1.0, shape: HitShape::Weapon, mv: 1.0, poise: 12.0, stamina: 22.0, knock: Knock::Flinch, unblockable: false };
                    let owner = self.projectiles[k].owner as usize;
                    // Damage comes from the bow rating via the owner's weapon.
                    let _ = owner;
                    let pr_vel = self.projectiles[k].vel;
                    let fake_from = point - pr_vel.normalize_or(Vec3::Z) * 3.0;
                    self.apply_arrow(ti, &w, point, fake_from);
                }
            }
        }
        self.projectiles.retain(|p| p.alive);
    }

    fn apply_arrow(&mut self, ti: usize, w: &HitWindow, point: Vec3, from: Vec3) {
        let t = &mut self.actors[ti];
        let to_att = flat(from - t.pos).normalize_or(t.forward());
        let facing = t.forward().dot(to_att) > deg(75.0).cos();
        if t.blocking && facing {
            t.stats.stamina -= w.stamina * 0.4 + 4.0;
            t.stamina_delay = 0.8;
            t.set_action(Action::GuardHit { dir: -to_att });
            self.sound_at(Sfx::Block, point, 0.8);
            self.fx.sparks(point, to_att, 8);
        } else {
            let dmg = combat::mitigate(55.0, t.stats.absorb);
            t.stats.hp -= dmg;
            t.flash = 1.0;
            t.react(w.knock, w.poise, from);
            self.sound_at(Sfx::HitArmor, point, 0.8);
            self.fx.blood(point, -to_att, 6);
            if self.actors[ti].stats.hp <= 0.0 {
                self.kill_actor(ti);
            }
        }
    }

    // ------------------------------------------------------------ Events

    fn handle_event(&mut self, e: ActorEvent, assets: &Assets) {
        match e {
            ActorEvent::Sound { sfx, pos, vol } => self.sound_at(sfx, pos, vol),
            ActorEvent::Shake(s) => {
                let d = self.demon.and_then(|id| self.actor(id)).map(|d| (d.pos - self.actors[PLAYER].pos).length()).unwrap_or(0.0);
                self.camera.add_shake(s * (1.0 - (d / 40.0).min(0.8)));
            }
            ActorEvent::Dust { pos, size } => self.fx.dust(pos, size),
            ActorEvent::Footstep { pos, surface: s, heavy } => {
                let is_player = (pos - self.actors[PLAYER].pos).length() < 0.01;
                let sfx = if heavy {
                    Sfx::StepDemon
                } else if s == surface::WATER {
                    Sfx::StepWater
                } else if is_player {
                    Sfx::StepArmor
                } else {
                    Sfx::StepHusk
                };
                let vol = if heavy { 1.0 } else if is_player { 0.35 } else { 0.3 };
                self.sound_at(sfx, pos, vol);
                if s == surface::WATER {
                    self.fx.splash(pos);
                }
                if heavy {
                    self.camera.add_shake(0.12);
                    self.fx.dust(pos, 0.6);
                }
            }
            ActorEvent::Healed(id) => {
                if id == 0 {
                    let p = &mut self.actors[PLAYER];
                    p.stats.hp = (p.stats.hp + p.stats.max_hp * 0.45).min(p.stats.max_hp);
                    let pos = p.pos;
                    self.fx.heal(pos);
                    self.sound_at(Sfx::Heal, pos, 0.8);
                }
            }
            ActorEvent::Died(id) => self.on_death(id, assets),
            ActorEvent::FellToDeath(id) => {
                let i = id as usize;
                if !self.actors[i].dead {
                    self.kill_actor(i);
                }
            }
            ActorEvent::UseFired { id, tag } => {
                if id == 0 {
                    self.on_use_fired(tag, assets);
                }
            }
            ActorEvent::Plunged { .. } => {}
            ActorEvent::Landed { id, height } => {
                if id == 0 && height > SAFE_FALL {
                    let p = &mut self.actors[PLAYER];
                    let k = ((height - SAFE_FALL) / (LETHAL_FALL - SAFE_FALL)).clamp(0.0, 1.0);
                    let dmg = p.stats.max_hp * k.powf(1.3) * 1.05;
                    p.stats.hp -= dmg;
                    p.flash = 0.6;
                    let pos = p.pos;
                    self.sound_at(Sfx::Hurt, pos, 0.8);
                    self.camera.add_shake(0.4);
                    if self.actors[PLAYER].stats.hp <= 0.0 {
                        self.kill_actor(PLAYER);
                    }
                }
            }
        }
    }

    fn on_death(&mut self, id: ActorId, assets: &Assets) {
        let i = id as usize;
        let pos = self.actors[i].center();
        if i == PLAYER {
            self.sound_at(Sfx::Death, pos, 1.0);
            self.player_died(assets);
            return;
        }
        let body = self.actors[i].body;
        if body.is_demon() {
            self.sound_at(Sfx::DemonDeath, pos, 1.0);
            self.boss_defeated();
        } else {
            self.sound_at(Sfx::HuskDeath, pos, 0.8);
            self.progress.kills += 1;
            let souls = self.actors[i].souls;
            if souls > 0 {
                let to = self.actors[PLAYER].center();
                self.fx.cinders(pos, to);
                self.progress.cinders += souls;
                self.sound_at(Sfx::Cinders, pos, 0.6);
            }
        }
        if self.actors[PLAYER].lock == Some(id) {
            self.actors[PLAYER].lock = None;
        }
    }

    fn update_ambient_fx(&mut self, dt: f32) {
        for i in 0..self.props.len() {
            let pos = self.props[i].pos;
            if (pos - self.camera.pos).length_squared() > 45.0 * 45.0 {
                continue;
            }
            match &self.props[i].kind {
                PropKind::Torch => self.fx.flame(pos, 0.7, dt),
                PropKind::Pyre { lit: true, .. } => self.fx.flame(pos + Vec3::Y * 0.25, 2.2, dt),
                PropKind::Pyre { lit: false, .. } => {
                    if self.rng.f32() < dt * 2.0 {
                        self.fx.flame(pos + Vec3::Y * 0.2, 0.4, 0.05);
                    }
                }
                PropKind::Item { taken: false, .. } => self.fx.item_glint(pos, dt),
                PropKind::Fog(f) if f.active => {
                    let across = Quat::from_rotation_y(f.yaw) * Vec3::X;
                    let w = f.width;
                    self.fx.mist(pos, across, w, 3.0, dt);
                }
                _ => {}
            }
        }
        // Dissolving corpses shed ash.
        for a in &self.actors {
            if a.dead && a.dissolve > 0.0 && a.dissolve < 1.0 {
                let r = if a.body.is_demon() { 2.5 } else { 0.6 };
                let c = a.center();
                self.fx.motes(c, r, dt);
            }
        }
    }
}

/// Builds the AI snapshot of an actor.
pub fn view_of(a: &Actor) -> ActorView {
    ActorView {
        id: a.id,
        team: a.team,
        pos: a.pos,
        center: a.center(),
        yaw: a.yaw,
        alive: !a.dead,
        busy: !matches!(a.action, Action::Move),
        attacking: matches!(a.action, Action::Attack { .. }),
        grounded: a.grounded,
        hp_frac: a.stats.hp / a.stats.max_hp,
    }
}
