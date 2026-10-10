//! Top-level game object driven by the platform shell.

use crate::assets::Assets;
use crate::assets::sounds::Sfx;
use crate::asylum;
use crate::hud::{BossBar, Dialog, HudState};
use crate::input::{Button, InputState, InputTracker};
use crate::level::EnvKind;
use crate::props::PropKind;
use crate::save::Progress;
use crate::view::{self, FixedCam};
use crate::world::{MapTemplate, PLAYER, SoundOut, World, WorldState};
use ashen_engine::math::*;
use ashen_engine::render::{RenderList, RenderSettings};
use serde::{Deserialize, Serialize};

pub const TICK: f32 = 1.0 / 60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Title,
    Playing,
    Paused,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub vertical_res: u32,
    pub vertex_snap: bool,
    pub affine: bool,
    pub dither: bool,
    pub force_4_3: bool,
    pub mouse_sens: f32,
    pub stick_sens: f32,
    pub invert_y: bool,
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
    pub show_fps: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            vertical_res: 240,
            vertex_snap: true,
            affine: true,
            dither: true,
            force_4_3: false,
            mouse_sens: 1.0,
            stick_sens: 1.0,
            invert_y: false,
            master: 0.8,
            music: 0.7,
            sfx: 0.9,
            show_fps: false,
        }
    }
}

impl Settings {
    pub fn render(&self) -> RenderSettings {
        RenderSettings { vertical_res: self.vertical_res, vertex_snap: self.vertex_snap, affine: self.affine, dither: self.dither, force_4_3: self.force_4_3 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiCommand {
    NewGame,
    Continue(String),
    Pause,
    Resume,
    QuitToTitle,
    LeavePyre,
    Settings(Settings),
    ToggleDebug,
    /// Developer console command, e.g. `warp 0 3 4 0`, `key Great`, `flag boss_dead`, `heal`.
    Debug(String),
}

/// A looping sound the platform should keep playing this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopOut {
    pub key: u32,
    pub sfx: Sfx,
    pub gain: f32,
    pub pan: f32,
}

#[derive(Clone, Debug, Default)]
pub struct AudioFrame {
    pub oneshots: Vec<SoundOut>,
    pub loops: Vec<LoopOut>,
}

pub struct Game {
    pub assets: Assets,
    map: MapTemplate,
    pub world: World,
    pub mode: Mode,
    input: InputTracker,
    acc: f32,
    pub settings: Settings,
    pending_save: Option<String>,
    sounds: Vec<SoundOut>,
    pub time: f32,
    fps: f32,
}

impl Game {
    pub fn new() -> Self {
        let mut assets = Assets::build();
        let map = MapTemplate::new(asylum::build(&mut assets), &mut assets);
        let world = Self::title_world(&map, &assets);
        Game {
            assets,
            map,
            world,
            mode: Mode::Title,
            input: InputTracker::default(),
            acc: 0.0,
            settings: Settings::default(),
            pending_save: None,
            sounds: Vec::new(),
            time: 0.0,
            fps: 60.0,
        }
    }

    fn title_world(map: &MapTemplate, assets: &Assets) -> World {
        let mut w = World::new(map, assets, None);
        for p in w.props.iter_mut() {
            if let PropKind::Pyre { lit, .. } = &mut p.kind {
                *lit = true;
            }
        }
        w.env_kind = EnvKind::Courtyard;
        w.area_shown = true;
        w
    }

    pub fn command(&mut self, c: UiCommand) {
        match c {
            UiCommand::NewGame => {
                self.world = World::new(&self.map, &self.assets, None);
                self.mode = Mode::Playing;
                self.input.reset();
                self.apply_settings();
                self.pending_save = Some(self.world.progress.to_json());
            }
            UiCommand::Continue(json) => {
                if let Some(p) = Progress::from_json(&json) {
                    self.world = World::new(&self.map, &self.assets, Some(p));
                    self.mode = Mode::Playing;
                    self.input.reset();
                    self.apply_settings();
                }
            }
            UiCommand::Pause => {
                if self.mode == Mode::Playing {
                    self.mode = Mode::Paused;
                }
            }
            UiCommand::Resume => {
                if self.mode == Mode::Paused {
                    self.mode = Mode::Playing;
                    self.input.reset();
                }
            }
            UiCommand::QuitToTitle => {
                if self.mode != Mode::Title {
                    self.pending_save = Some(self.world.progress.to_json());
                }
                self.world = Self::title_world(&self.map, &self.assets);
                self.mode = Mode::Title;
            }
            UiCommand::LeavePyre => self.world.stand_up(),
            UiCommand::Settings(s) => {
                self.settings = s;
                self.apply_settings();
            }
            UiCommand::ToggleDebug => self.world.debug = !self.world.debug,
            UiCommand::Debug(cmd) => self.debug_command(&cmd),
        }
    }

    fn debug_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.as_slice() == ["fps"] {
            self.settings.show_fps = true;
            return;
        }
        let w = &mut self.world;
        match parts.as_slice() {
            ["warp", x, y, z, rest @ ..] => {
                let p = Vec3::new(x.parse().unwrap_or(0.0), y.parse().unwrap_or(0.0), z.parse().unwrap_or(0.0));
                let yaw = rest.first().and_then(|v| v.parse::<f32>().ok()).map(deg).unwrap_or(0.0);
                let a = &mut w.actors[PLAYER];
                a.pos = p;
                a.yaw = yaw;
                a.vel = Vec3::ZERO;
                a.set_action(crate::actor::Action::Move);
                w.camera.reset_behind(yaw, p + Vec3::Y * 1.55);
            }
            ["key", k] => {
                let key = match *k {
                    "Cell" => Some(crate::props::Key::Cell),
                    "East" => Some(crate::props::Key::EastWard),
                    "Great" => Some(crate::props::Key::Great),
                    _ => None,
                };
                if let Some(k) = key {
                    w.progress.keys.insert(k);
                }
            }
            ["flag", f] => w.progress.set(f),
            ["flasks", n] => {
                w.progress.flasks_max = n.parse().unwrap_or(5);
                w.progress.flasks = w.progress.flasks_max;
            }
            ["heal"] => {
                let a = &mut w.actors[PLAYER];
                a.stats.hp = a.stats.max_hp;
            }
            ["god"] => {
                let a = &mut w.actors[PLAYER];
                a.stats.max_hp = 1e6;
                a.stats.hp = 1e6;
            }
            ["reset"] => w.reset_enemies(),
            ["encounter"] => {
                w.encounter = crate::world::Encounter::Active;
                w.ledge_timer = 0.0;
            }
            _ => {}
        }
    }

    fn apply_settings(&mut self) {
        self.world.camera.invert_y = self.settings.invert_y;
    }

    /// Advances the simulation by a frame's worth of fixed ticks.
    pub fn frame(&mut self, dt: f32, input: &InputState) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        if dt > 0.0 {
            self.fps = lerp(self.fps, 1.0 / dt, 0.05);
        }
        let mut input = input.clone();
        input.look *= if input.gamepad { self.settings.stick_sens } else { self.settings.mouse_sens };
        self.input.begin_frame(&input);
        if self.mode == Mode::Playing && self.input.pressed(Button::Pause) {
            self.mode = Mode::Paused;
        }
        match self.mode {
            Mode::Title => {
                // Ambient-only simulation for the backdrop.
                self.world.time += dt;
                self.world.fx.update(dt);
                let pyre = Vec3::new(0.0, 3.0, 4.0);
                self.world.fx.flame(pyre + Vec3::Y * 0.25, 2.2, dt);
                self.world.env = ashen_engine::render::Environment {
                    ambient: Vec3::new(0.16, 0.16, 0.19),
                    sun_dir: Vec3::new(0.45, -1.0, 0.35),
                    sun_color: Vec3::new(0.12, 0.12, 0.14),
                    fog_color: Vec3::new(0.06, 0.06, 0.08),
                    fog_near: 6.0,
                    fog_far: 40.0,
                };
                self.world.fade = 0.0;
            }
            Mode::Paused => {}
            Mode::Playing => {
                self.acc += dt;
                let mut first = true;
                while self.acc >= TICK {
                    self.acc -= TICK;
                    self.world.update(TICK, &self.input, &self.assets);
                    self.sounds.append(&mut self.world.sounds);
                    if first {
                        self.input.consume_edges(TICK);
                        first = false;
                    }
                }
                if self.world.save_dirty {
                    self.world.save_dirty = false;
                    self.pending_save = Some(self.world.progress.to_json());
                }
            }
        }
    }

    pub fn render(&mut self, aspect: f32, list: &mut RenderList) {
        let fixed = (self.mode == Mode::Title).then(|| {
            let a = self.time * 0.05;
            FixedCam { eye: Vec3::new(3.2 * a.cos(), 4.7, 4.0 - 4.6 + 1.2 * a.sin()), target: Vec3::new(0.0, 3.7, 4.0) }
        });
        view::build(&mut self.world, &self.assets, aspect, fixed, list);
    }

    /// Takes the sounds produced since the last call.
    pub fn audio(&mut self) -> AudioFrame {
        let s = &self.settings;
        let mut f = AudioFrame { oneshots: std::mem::take(&mut self.sounds), loops: Vec::new() };
        for o in f.oneshots.iter_mut() {
            let bus = match o.sfx.bus() {
                crate::assets::sounds::Bus::Music => s.music,
                _ => s.sfx,
            };
            o.gain *= s.master * bus;
        }
        let w = &self.world;
        let cam = w.camera.pos;
        let fwd = w.camera.forward();
        let right = Vec3::new(-fwd.z, 0.0, fwd.x).normalize_or(Vec3::X);
        let spatial = |pos: Vec3, far: f32| {
            let d = pos - cam;
            let dist = d.length();
            let g = (1.0 - (dist / far).clamp(0.0, 1.0)).powi(2);
            let pan = if dist > 0.5 { (d / dist).dot(right) * 0.7 } else { 0.0 };
            (g, pan)
        };
        if self.mode != Mode::Title {
            let amb = if matches!(w.env_kind, EnvKind::Dungeon | EnvKind::Cell) { Sfx::AmbientDungeon } else { Sfx::AmbientWind };
            f.loops.push(LoopOut { key: 1, sfx: amb, gain: 0.35 * s.master * s.sfx, pan: 0.0 });
            if w.music_gain > 0.001 {
                f.loops.push(LoopOut { key: 2, sfx: Sfx::BossMusic, gain: w.music_gain * 0.75 * s.master * s.music, pan: 0.0 });
            }
            if matches!(w.boulder.state, crate::world::BoulderState::Rolling) {
                let (g, pan) = spatial(w.boulder.pos, 40.0);
                f.loops.push(LoopOut { key: 3, sfx: Sfx::BoulderRoll, gain: g * s.master * s.sfx, pan });
            }
        }
        // Nearest lit pyre crackle.
        let mut best: Option<(f32, f32)> = None;
        for p in &w.props {
            if let PropKind::Pyre { lit: true, .. } = p.kind {
                let (g, pan) = spatial(p.pos, 14.0);
                if g > best.map(|b| b.0).unwrap_or(0.0) {
                    best = Some((g, pan));
                }
            }
        }
        if let Some((g, pan)) = best {
            f.loops.push(LoopOut { key: 4, sfx: Sfx::PyreLoop, gain: g * 0.8 * s.master * s.sfx, pan });
        }
        f
    }

    pub fn take_save(&mut self) -> Option<String> {
        self.pending_save.take()
    }

    pub fn hud(&self) -> HudState {
        let w = &self.world;
        let mut h = HudState { gamepad: self.input.state.gamepad, ..Default::default() };
        if self.mode == Mode::Title {
            return h;
        }
        let p = &w.actors[PLAYER];
        h.visible = !matches!(w.state, WorldState::Ending { .. });
        h.hp = p.stats.hp.max(0.0);
        h.hp_max = p.stats.max_hp;
        h.hp_lag = w.hp_lag.max(0.0);
        h.stamina = p.stats.stamina.max(0.0);
        h.stamina_max = p.stats.max_stamina;
        h.flasks = w.progress.flasks;
        h.flasks_max = w.progress.flasks_max;
        h.cinders = w.progress.cinders;
        if w.boss_bar_visible() {
            if let Some(d) = w.demon.and_then(|id| w.actor(id)) {
                h.boss = Some(BossBar { name: "Warden Demon".into(), hp: (d.stats.hp / d.stats.max_hp).max(0.0), lag: w.boss_lag.max(0.0) });
            }
        }
        h.prompt = w.prompt.clone();
        h.banner = w.banner.as_ref().map(|(b, t)| {
            let mut b = b.clone();
            b.alpha = (t * 1.5).min(1.0).min((4.6 - t).max(0.0) * 2.0 + 0.0).max(0.0).min(1.0);
            b
        });
        h.popups = w.popups.iter().map(|p| p.0.clone()).collect();
        match &w.state {
            WorldState::Reading(t) => h.message = Some(t.to_string()),
            WorldState::Dialog { lines, idx, .. } => {
                h.dialog = Some(Dialog { speaker: "Sir Corvin".into(), line: lines[*idx].to_string(), more: idx + 1 < lines.len() })
            }
            WorldState::Resting => {
                let name = w
                    .props
                    .iter()
                    .find_map(|p| match &p.kind {
                        PropKind::Pyre { id, name, .. } if Some(id.to_string()) == w.progress.last_pyre => Some(name.to_string()),
                        _ => None,
                    })
                    .unwrap_or_default();
                h.resting = Some(name);
            }
            WorldState::Ending { t } => h.ending = Some(*t),
            _ => {}
        }
        if w.debug || self.settings.show_fps {
            h.debug = Some(format!(
                "{:.0} fps  pos {:.1} {:.1} {:.1}  {:?}",
                self.fps,
                p.pos.x,
                p.pos.y,
                p.pos.z,
                format!("{:?}", p.action).split([' ', '{', '(']).next().unwrap_or("")
            ));
        }
        h
    }

    pub fn is_playing(&self) -> bool {
        self.mode == Mode::Playing
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}
