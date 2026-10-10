//! Scripted world logic: interactions, pyres, the boss encounter, the
//! boulder trap, death/respawn and progress application.

use crate::actor::*;
use crate::ai::{Brain, DemonBrain, DemonPhase, HuskBrain};
use crate::anims::demon::clip as dc;
use crate::assets::Assets;
use crate::assets::sounds::Sfx;
use crate::asylum::points;
use crate::combat::WeaponKind;
use crate::hud::BannerKind;
use crate::input::{Button, InputTracker};
use crate::props::*;
use crate::world::*;
use ashen_engine::math::*;
use std::f32::consts::PI;

const CORVIN_FIRST: &[&str] = &[
    "...Oh. You... you are no husk. Not yet, at least.",
    "Thank the old flame. I feared I would die alone in this wretched place.",
    "I am Corvin, a knight of Valemark. I came here on an oath... and I fear I shall not see it through.",
    "There is a saying in my house: when the cursed rise from the asylum, they must go north and ring the bells of waking.",
    "I have failed in that. But you... you still have fire in you.",
    "Take this flask. It holds an ember of the first flame, and it will mend your wounds. Rest at a pyre to rekindle it.",
    "And this key. It opens the gate of the east ward. The warden demon waits below... strike it from above, if you can.",
    "Now go. I have no wish to become... one of them... in front of you.",
];

const CORVIN_AGAIN: &[&str] = &[
    "Still here? Go... the bells will not ring themselves.",
    "Strike the demon from the ledge above. It is the only advantage you will have.",
];

const CORVIN_LATE: &[&str] = &["...", "(Sir Corvin no longer stirs. He has found his rest.)"];

impl World {
    // ------------------------------------------------------------- UI states

    pub(crate) fn update_ui_state(&mut self, dt: f32, inp: &InputTracker, assets: &Assets) {
        let advance = inp.pressed(Button::Interact) || inp.pressed(Button::Confirm) || inp.pressed(Button::Light);
        match self.state.clone() {
            WorldState::Playing => {}
            WorldState::Reading(_) => {
                let far = self.interact_target.is_none();
                if inp.pressed(Button::Interact) || inp.pressed(Button::Back) || inp.pressed(Button::Dodge) || far {
                    self.state = WorldState::Playing;
                }
            }
            WorldState::Dialog { lines, idx, reward } => {
                if advance {
                    if idx + 1 < lines.len() {
                        self.state = WorldState::Dialog { lines, idx: idx + 1, reward };
                    } else {
                        self.state = WorldState::Playing;
                        if reward {
                            self.corvin_reward();
                        }
                    }
                } else if inp.pressed(Button::Back) {
                    self.state = WorldState::Playing;
                    if reward {
                        self.corvin_reward();
                    }
                }
            }
            WorldState::Resting => {
                if inp.pressed(Button::Back) || inp.pressed(Button::Interact) || inp.state.move_axis.length() > 0.6 {
                    self.stand_up();
                }
            }
            WorldState::Dead { t } => {
                let t = t + dt;
                self.state = WorldState::Dead { t };
                if t > 4.8 {
                    self.respawn_player(assets);
                }
            }
            WorldState::Ending { t } => {
                self.state = WorldState::Ending { t: t + dt };
            }
        }
    }

    pub fn stand_up(&mut self) {
        if matches!(self.actors[PLAYER].action, Action::Sitting | Action::Use { tag: UseTag::SitDown, .. }) {
            self.actors[PLAYER].begin_use(UseTag::StandUp, Vec3::ZERO);
        }
        if self.state == WorldState::Resting {
            self.state = WorldState::Playing;
        }
    }

    // ---------------------------------------------------------- Interaction

    pub(crate) fn find_interact(&self) -> Option<Interact> {
        let p = &self.actors[PLAYER];
        if p.dead || !p.grounded || !matches!(p.action, Action::Move) {
            return None;
        }
        if let Some((pos, _)) = self.progress.bloodstain {
            if (Vec3::from(pos) - p.pos).length() < 1.5 {
                return Some(Interact::Bloodstain);
            }
        }
        if let Some(cid) = self.corvin {
            let c = &self.actors[cid as usize];
            if (c.pos - p.pos).length() < 2.4 && (c.pos.y - p.pos.y).abs() < 1.0 {
                return Some(Interact::Npc(cid));
            }
        }
        let fwd = p.forward();
        let mut best: Option<(f32, usize)> = None;
        for (i, prop) in self.props.iter().enumerate() {
            let Some((pt, reach)) = prop.interact_point() else { continue };
            let d = pt - (p.pos + Vec3::Y * 0.9);
            if d.y.abs() > 2.2 {
                continue;
            }
            let fd = flat(d);
            let dist = fd.length();
            if dist > reach {
                continue;
            }
            let facing = if dist > 0.3 { fd.normalize().dot(fwd) } else { 1.0 };
            if dist > 0.6 && facing < -0.3 {
                continue;
            }
            // Prefer what the player is looking at.
            let score = dist + (1.0 - facing) * 0.8;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, i));
            }
        }
        best.map(|(_, i)| Interact::Prop(i))
    }

    pub(crate) fn update_prompt(&mut self) {
        self.interact_target = self.find_interact();
        self.prompt = match (&self.state, self.interact_target) {
            (WorldState::Playing, Some(Interact::Prop(i))) => self.props[i].prompt().map(|s| s.to_string()),
            (WorldState::Playing, Some(Interact::Npc(_))) => Some("Talk".to_string()),
            (WorldState::Playing, Some(Interact::Bloodstain)) => Some("Reclaim your cinders".to_string()),
            _ => None,
        };
        // Keep reading while near the rune.
        if let WorldState::Reading(_) = self.state {
            self.interact_target = self.interact_target.or_else(|| {
                let p = self.actors[PLAYER].pos;
                self.props.iter().position(|pr| matches!(pr.kind, PropKind::Message { .. }) && (pr.pos - p).length() < 2.5).map(Interact::Prop)
            });
        }
    }

    pub(crate) fn try_interact(&mut self, assets: &Assets) {
        if self.state != WorldState::Playing {
            return;
        }
        let Some(target) = self.find_interact() else { return };
        let ppos = self.actors[PLAYER].pos;
        match target {
            Interact::Bloodstain => {
                if let Some((pos, amount)) = self.progress.bloodstain.take() {
                    self.progress.cinders += amount;
                    let to = self.actors[PLAYER].center();
                    self.fx.cinders(Vec3::from(pos) + Vec3::Y * 0.5, to);
                    self.sound_at(Sfx::Cinders, ppos, 0.9);
                    self.popup(format!("Reclaimed {amount} cinders"));
                    self.save_dirty = true;
                }
            }
            Interact::Npc(_) => {
                let (lines, reward) = if self.progress.has("boss_dead") && self.progress.has("corvin_talked") {
                    (CORVIN_LATE, false)
                } else if self.progress.has("corvin_talked") {
                    (CORVIN_AGAIN, false)
                } else {
                    (CORVIN_FIRST, true)
                };
                self.state = WorldState::Dialog { lines, idx: 0, reward };
                if reward {
                    self.progress.set("corvin_talked");
                }
            }
            Interact::Prop(i) => self.interact_prop(i, assets),
        }
    }

    fn corvin_reward(&mut self) {
        if self.progress.flasks_max < 5 {
            self.progress.flasks_max = 5;
            self.progress.flasks = 5;
            self.popup("Received: Ember Flask x5".to_string());
        }
        if self.progress.keys.insert(Key::EastWard) {
            self.popup(format!("Received: {}", Key::EastWard.name()));
        }
        let p = self.actors[PLAYER].pos;
        self.sound_at(Sfx::ItemPickup, p, 0.8);
        self.save_dirty = true;
    }

    fn interact_prop(&mut self, i: usize, _assets: &Assets) {
        let ppos = self.actors[PLAYER].pos;
        let prop_pos = self.props[i].pos;
        let dir_to = flat(prop_pos - ppos).normalize_or(self.actors[PLAYER].forward());
        match self.props[i].kind.clone() {
            PropKind::Pyre { id, lit, .. } => {
                if !lit {
                    if let PropKind::Pyre { lit, .. } = &mut self.props[i].kind {
                        *lit = true;
                    }
                    self.progress.set(&format!("pyre:{id}"));
                    self.show_banner("PYRE KINDLED", BannerKind::Kindled, 3.0);
                    self.sound_at(Sfx::PyreIgnite, prop_pos, 1.0);
                }
                self.actors[PLAYER].begin_use(UseTag::SitDown, dir_to);
                self.rest_at(i);
            }
            PropKind::Door(d) => {
                let side_ok = (ppos - prop_pos).dot(d.side) > 0.0;
                match d.lock {
                    DoorLock::Open => self.start_open(i, dir_to),
                    DoorLock::Locked(k) => {
                        if self.progress.keys.contains(&k) {
                            self.popup(format!("Used: {}", k.name()));
                            self.start_open(i, dir_to);
                        } else {
                            self.popup("Locked.".to_string());
                            self.sound_at(Sfx::DoorLocked, prop_pos, 0.8);
                        }
                    }
                    DoorLock::OneWay => {
                        if side_ok {
                            self.start_open(i, dir_to);
                        } else {
                            self.popup("It will not open from this side.".to_string());
                            self.sound_at(Sfx::DoorLocked, prop_pos, 0.8);
                        }
                    }
                    DoorLock::Barred => {
                        self.popup("It is barred shut.".to_string());
                        self.sound_at(Sfx::DoorLocked, prop_pos, 0.8);
                    }
                }
            }
            PropKind::Gate(g) => match g.lock {
                DoorLock::Open => self.start_open(i, dir_to),
                DoorLock::Locked(k) => {
                    if self.progress.keys.contains(&k) {
                        self.popup(format!("Used: {}", k.name()));
                        self.start_open(i, dir_to);
                    } else {
                        self.popup("Locked.".to_string());
                        self.sound_at(Sfx::DoorLocked, prop_pos, 0.8);
                    }
                }
                _ => {
                    self.popup("It will not budge.".to_string());
                    self.sound_at(Sfx::DoorLocked, prop_pos, 0.8);
                }
            },
            PropKind::Fog(f) => {
                let n = Quat::from_rotation_y(f.yaw) * Vec3::Z;
                let side = (ppos - prop_pos).dot(n).signum();
                let dir = -n * if side == 0.0 { 1.0 } else { side };
                // Line up with the veil's centre.
                let across = Quat::from_rotation_y(f.yaw) * Vec3::X;
                let off = (ppos - prop_pos).dot(across);
                let p = &mut self.actors[PLAYER];
                p.pos = Vec3::new(prop_pos.x, p.pos.y, prop_pos.z) + across * off.clamp(-f.width * 0.3, f.width * 0.3) - dir * 0.6;
                p.begin_use(UseTag::FogWalk, dir);
                self.fog_pass = Some(i);
                if let PropKind::Fog(f) = &mut self.props[i].kind {
                    f.passing = true;
                }
                self.sound_at(Sfx::FogPass, prop_pos, 0.9);
            }
            PropKind::Item { .. } => {
                self.actors[PLAYER].begin_use(UseTag::Pickup, dir_to);
                self.pending = Some(i);
            }
            PropKind::Message { text } => {
                self.state = WorldState::Reading(text);
                self.sound_2d(Sfx::MenuSelect, 0.4);
            }
            _ => {}
        }
    }

    fn start_open(&mut self, i: usize, dir: Vec3) {
        self.actors[PLAYER].begin_use(UseTag::PushDoor, dir);
        self.pending = Some(i);
    }

    pub(crate) fn on_use_fired(&mut self, tag: UseTag, _assets: &Assets) {
        match tag {
            UseTag::Pickup => {
                let Some(i) = self.pending.take() else { return };
                if let PropKind::Item { item, flag, taken } = &mut self.props[i].kind {
                    if *taken {
                        return;
                    }
                    *taken = true;
                    let (item, flag) = (*item, *flag);
                    self.progress.set(flag);
                    match item {
                        ItemKind::Key(k) => {
                            self.progress.keys.insert(k);
                        }
                        ItemKind::Cinders(n) => self.progress.cinders += n,
                    }
                    self.popup(format!("Received: {}", item.label()));
                    let p = self.props[i].pos;
                    self.sound_at(Sfx::ItemPickup, p, 0.9);
                    self.save_dirty = true;
                }
            }
            UseTag::PushDoor => {
                let Some(i) = self.pending.take() else { return };
                let pos = self.props[i].pos;
                match &mut self.props[i].kind {
                    PropKind::Door(d) => {
                        d.lock = DoorLock::Open;
                        d.target = 1.0;
                        let flag = d.flag;
                        self.progress.set(flag);
                    }
                    PropKind::Gate(g) => {
                        g.lock = DoorLock::Open;
                        g.target_lift = g.open_lift;
                        let flag = g.flag;
                        self.progress.set(flag);
                    }
                    _ => {}
                }
                self.sound_at(Sfx::DoorOpen, pos, 1.0);
                self.save_dirty = true;
            }
            UseTag::FogWalk => {
                if let Some(i) = self.fog_pass.take() {
                    if let PropKind::Fog(f) = &mut self.props[i].kind {
                        f.passing = false;
                        if f.boss && !self.progress.has("boss_dead") {
                            self.start_second_encounter();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------- Rest & respawn

    fn pyre_spawn(&self, id: &str) -> Option<(Vec3, f32)> {
        self.props.iter().find_map(|p| match &p.kind {
            PropKind::Pyre { id: pid, .. } if *pid == id => Some((p.pos + Vec3::new(0.0, 0.0, -1.4), 0.0)),
            _ => None,
        })
    }

    fn rest_at(&mut self, i: usize) {
        let PropKind::Pyre { id, .. } = self.props[i].kind.clone() else { return };
        self.progress.last_pyre = Some(id.to_string());
        let p = &mut self.actors[PLAYER];
        p.stats.hp = p.stats.max_hp;
        p.stats.stamina = p.stats.max_stamina;
        self.progress.flasks = self.progress.flasks_max;
        self.reset_enemies();
        self.state = WorldState::Resting;
        self.actors[PLAYER].lock = None;
        self.save_dirty = true;
    }

    /// Brings fallen foes back (rest or death), resets the boss if alive.
    pub(crate) fn reset_enemies(&mut self) {
        let boulder_done = self.progress.has("boulder_done");
        for i in 1..self.actors.len() {
            let a = &mut self.actors[i];
            match &mut a.brain {
                Brain::Npc => continue,
                Brain::Demon(_) => continue,
                Brain::Husk(h) => {
                    h.state = crate::ai::HuskState::Idle;
                    h.cd = 0.8;
                }
                Brain::Archer(ar) => {
                    ar.aware = false;
                    ar.cd = 1.5;
                }
                Brain::Pusher { triggered } => {
                    *triggered = false;
                }
                _ => {}
            }
            a.respawn();
            if Some(a.id) == self.pusher && boulder_done {
                let home = a.spawn_pos;
                a.brain = Brain::Husk(HuskBrain::new(home, false));
            }
        }
        for p in self.props.iter_mut() {
            if let PropKind::Urn { broken, respawn: true, .. } = &mut p.kind {
                *broken = false;
            }
        }
        self.projectiles.clear();
        if !self.progress.has("boss_dead") {
            self.reset_demon();
        }
    }

    pub(crate) fn reset_demon(&mut self) {
        let Some(id) = self.demon else { return };
        let escaped = self.progress.has("asylum_escaped");
        let d = &mut self.actors[id as usize];
        d.respawn();
        d.pos = if escaped { points::DEMON_WAIT } else { points::DEMON_PERCH };
        d.yaw = PI;
        d.brain = Brain::Demon(DemonBrain::new(if escaped { DemonPhase::Waiting } else { DemonPhase::Perched }));
        self.encounter = Encounter::None;
        self.demon_drop = None;
        let hall = self.named.hall_door;
        if let PropKind::Door(door) = &mut self.props[hall].kind {
            door.lock = if escaped { DoorLock::Barred } else { DoorLock::Open };
            door.target = 0.0;
            door.open = 0.0;
        }
        let gate = self.named.escape_gate;
        if let PropKind::Gate(g) = &mut self.props[gate].kind {
            if escaped {
                g.lift = 0.0;
                g.target_lift = 0.0;
            } else {
                g.lift = g.open_lift;
                g.target_lift = g.open_lift;
            }
        }
        let fog = self.named.boss_fog;
        if let PropKind::Fog(f) = &mut self.props[fog].kind {
            f.active = true;
        }
    }

    pub(crate) fn player_died(&mut self, _assets: &Assets) {
        self.progress.deaths += 1;
        let pos = self.actors[PLAYER].pos;
        self.progress.bloodstain = (self.progress.cinders > 0 && pos.y > self.kill_y + 1.0).then_some((pos.to_array(), self.progress.cinders));
        self.progress.cinders = 0;
        self.state = WorldState::Dead { t: 0.0 };
        self.show_banner("YOU PERISHED", BannerKind::Death, 4.6);
        self.sound_2d(Sfx::YouDied, 0.9);
        self.actors[PLAYER].lock = None;
        self.save_dirty = true;
    }

    fn respawn_player(&mut self, _assets: &Assets) {
        let spawn = self.progress.last_pyre.clone().and_then(|id| self.pyre_spawn(&id));
        let (pos, yaw) = spawn.unwrap_or((Vec3::new(-0.7, 0.0, -44.6), 0.0));
        let p = &mut self.actors[PLAYER];
        p.spawn_pos = pos;
        p.spawn_yaw = yaw;
        p.respawn();
        self.progress.flasks = self.progress.flasks_max;
        self.reset_enemies();
        self.state = WorldState::Playing;
        self.fx.clear();
        self.camera.reset_behind(yaw, pos + Vec3::Y * 1.55);
        self.banner = None;
        self.save_dirty = true;
    }

    // ------------------------------------------------------- Boss encounter

    fn demon_brain(&mut self) -> Option<&mut DemonBrain> {
        let id = self.demon?;
        match &mut self.actors[id as usize].brain {
            Brain::Demon(b) => Some(b),
            _ => None,
        }
    }

    fn start_second_encounter(&mut self) {
        if self.encounter != Encounter::None {
            return;
        }
        self.encounter = Encounter::Active;
        self.ledge_timer = 0.0;
    }

    pub(crate) fn boss_defeated(&mut self) {
        self.progress.set("boss_dead");
        self.encounter = Encounter::Won { t: 0.0 };
        self.sound_2d(Sfx::Victory, 0.8);
        let hall = self.named.hall_door;
        if let PropKind::Door(d) = &mut self.props[hall].kind {
            d.lock = DoorLock::Open;
        }
        let fog = self.named.boss_fog;
        if let PropKind::Fog(f) = &mut self.props[fog].kind {
            f.active = false;
        }
        if let Some(id) = self.demon {
            let from = self.actors[id as usize].center();
            let to = self.actors[PLAYER].center();
            self.fx.cinders(from, to);
            self.fx.cinders(from + Vec3::Y, to);
        }
        self.save_dirty = true;
    }

    pub(crate) fn update_scripts(&mut self, dt: f32, _assets: &Assets) {
        let ppos = self.actors[PLAYER].pos;
        let playing = !matches!(self.state, WorldState::Dead { .. });

        // Opening banner.
        if !self.area_shown && self.time > 2.0 {
            self.area_shown = true;
            if !self.progress.has("intro_seen") {
                self.progress.set("intro_seen");
                self.show_banner("THE HOLLOW ASYLUM", BannerKind::Area, 4.0);
            }
        }

        // First encounter: the demon drops in when the player enters the arena.
        let in_arena = ppos.x.abs() < 11.5 && ppos.z > 17.4 && ppos.z < 47.0 && ppos.y < 6.0;
        if playing
            && self.encounter == Encounter::None
            && in_arena
            && !self.progress.has("boss_dead")
            && !self.progress.has("asylum_escaped")
            && self.demon_brain().is_some_and(|b| b.phase == DemonPhase::Perched)
        {
            self.encounter = Encounter::Intro { t: 0.0 };
            let hall = self.named.hall_door;
            if let PropKind::Door(d) = &mut self.props[hall].kind {
                d.target = 0.0;
                d.lock = DoorLock::Barred;
                d.speed = 1.5;
            }
            if let Some(b) = self.demon_brain() {
                b.phase = DemonPhase::Dropping;
            }
            let id = self.demon.unwrap() as usize;
            let to = Vec3::new(ppos.x.clamp(-6.0, 6.0) * 0.5, 3.0, (ppos.z + 11.0).clamp(26.0, 34.0));
            self.demon_drop = Some(DropMotion { from: points::DEMON_PERCH, to, t: 0.0, dur: 1.6, apex: 3.5 });
            let d = &mut self.actors[id];
            d.set_action(Action::Script { clip: dc::FALL, dur: 1.6 });
            d.yaw = dir_yaw(flat(ppos - to));
            let c = d.center();
            self.sound_at(Sfx::DemonRoar, c, 1.0);
            self.sound_at(Sfx::DemonWing, c, 1.0);
        }

        if let Some(mut m) = self.demon_drop {
            m.t += dt;
            let u = (m.t / m.dur).min(1.0);
            let id = self.demon.unwrap() as usize;
            let d = &mut self.actors[id];
            d.pos = m.from.lerp(m.to, u) + Vec3::Y * m.apex * 4.0 * u * (1.0 - u);
            d.vel = Vec3::ZERO;
            if u >= 1.0 {
                d.pos = m.to;
                d.grounded = true;
                d.set_action(Action::Land { hard: true });
                self.demon_drop = None;
                self.sound_at(Sfx::DemonSlam, m.to, 1.0);
                self.camera.add_shake(1.2);
                self.fx.dust(m.to, 3.5);
            } else {
                self.demon_drop = Some(m);
            }
        }

        match self.encounter {
            Encounter::Intro { t } => {
                let t2 = t + dt;
                self.encounter = Encounter::Intro { t: t2 };
                let land = 1.6 + dc::LAND_DUR * 0.8;
                if t < land && t2 >= land {
                    if let Some(id) = self.demon {
                        let d = &mut self.actors[id as usize];
                        d.set_action(Action::Script { clip: dc::ROAR, dur: dc::ROAR_DUR });
                        let c = d.center();
                        self.sound_at(Sfx::DemonRoar, c, 1.0);
                        self.camera.add_shake(0.5);
                    }
                }
                if t2 >= land + dc::ROAR_DUR * 0.85 {
                    self.encounter = Encounter::Active;
                    if let Some(b) = self.demon_brain() {
                        b.phase = DemonPhase::Combat;
                        b.cd = 0.5;
                    }
                }
            }
            Encounter::Active => {
                // Escape through the side gate during the first encounter.
                let first = !self.progress.has("asylum_escaped");
                if first && ppos.x > 13.4 && ppos.z > 18.3 && ppos.z < 21.7 && ppos.y < 4.0 {
                    let gate = self.named.escape_gate;
                    if let PropKind::Gate(g) = &mut self.props[gate].kind {
                        g.target_lift = 0.0;
                        g.speed = 6.0;
                        g.lock = DoorLock::Barred;
                    }
                    self.progress.set("asylum_escaped");
                    self.encounter = Encounter::None;
                    if let Some(id) = self.demon {
                        let d = &mut self.actors[id as usize];
                        d.set_action(Action::Move);
                        d.pos = points::DEMON_WAIT;
                        d.yaw = PI;
                        d.brain = Brain::Demon(DemonBrain::new(DemonPhase::Waiting));
                    }
                    if self.actors[PLAYER].lock == self.demon {
                        self.actors[PLAYER].lock = None;
                    }
                    self.save_dirty = true;
                } else if self.demon_brain().is_some_and(|b| b.phase == DemonPhase::Waiting) {
                    // Second encounter: wakes when the player drops or lingers.
                    self.ledge_timer += dt;
                    if ppos.y < 9.0 || self.ledge_timer > 6.0 {
                        if let Some(b) = self.demon_brain() {
                            b.phase = DemonPhase::Combat;
                            b.cd = 1.2;
                        }
                        if let Some(id) = self.demon {
                            let c = self.actors[id as usize].center();
                            self.sound_at(Sfx::DemonRoar, c, 0.9);
                        }
                    }
                }
            }
            Encounter::Won { t } => {
                let t2 = t + dt;
                self.encounter = Encounter::Won { t: t2 };
                if t < 1.6 && t2 >= 1.6 {
                    self.show_banner("FOE VANQUISHED", BannerKind::Victory, 4.0);
                }
                if t < 4.0 && t2 >= 4.0 {
                    self.progress.cinders += 2000;
                    self.progress.keys.insert(Key::Great);
                    self.popup("Received: 2000 cinders".to_string());
                    self.popup(format!("Received: {}", Key::Great.name()));
                    let p = self.actors[PLAYER].pos;
                    self.sound_at(Sfx::ItemPickup, p, 0.9);
                    self.encounter = Encounter::None;
                    self.save_dirty = true;
                }
            }
            Encounter::None => {}
        }
        // Remove the demon once it has dissolved.
        if let Some(id) = self.demon {
            let d = &mut self.actors[id as usize];
            if d.dead && d.dissolve >= 1.0 {
                d.remove = true;
            }
        }

        // Boss music follows the fight.
        let fight = match self.encounter {
            Encounter::Intro { t } => t > 1.6,
            Encounter::Active => self.demon_brain_phase() == Some(DemonPhase::Combat),
            _ => false,
        };
        let target = if fight && playing { 1.0 } else { 0.0 };
        self.music_gain = approach(self.music_gain, target, dt * if target > self.music_gain { 0.8 } else { 0.35 });

        self.update_boulder(dt);

        // The end of the area.
        if playing && self.state == WorldState::Playing && (ppos - points::NEST).length() < 4.0 && !self.progress.has("ended") {
            self.progress.set("ended");
            self.state = WorldState::Ending { t: 0.0 };
            self.sound_2d(Sfx::Crow, 1.0);
            self.sound_2d(Sfx::Bell, 0.8);
            self.save_dirty = true;
        }
    }

    pub fn demon_brain_phase(&self) -> Option<DemonPhase> {
        let id = self.demon?;
        match &self.actors[id as usize].brain {
            Brain::Demon(b) => Some(b.phase),
            _ => None,
        }
    }

    pub fn boss_bar_visible(&self) -> bool {
        let alive = self.demon.and_then(|id| self.actor(id)).is_some_and(|d| !d.dead);
        alive && matches!(self.encounter, Encounter::Intro { .. } | Encounter::Active) && !matches!(self.state, WorldState::Dead { .. })
    }

    // ------------------------------------------------------------- Boulder

    fn update_boulder(&mut self, dt: f32) {
        let ppos = self.actors[PLAYER].pos;
        match self.boulder.state {
            BoulderState::Idle => {
                let on_stairs = ppos.x > 13.0 && ppos.x < 17.0 && ppos.z > 21.5 && ppos.z < 29.0 && ppos.y > 8.0;
                if on_stairs && !self.progress.has("boulder_done") {
                    self.boulder.state = BoulderState::Arming(0.0);
                    if let Some(id) = self.pusher {
                        let a = &mut self.actors[id as usize];
                        if !a.dead {
                            a.begin_use(UseTag::Push, Vec3::NEG_Z);
                            let c = a.center();
                            self.sound_at(Sfx::HuskGroan, c, 0.9);
                        }
                    }
                }
            }
            BoulderState::Arming(t) => {
                let t = t + dt;
                self.boulder.state = if t > 0.85 { BoulderState::Rolling } else { BoulderState::Arming(t) };
                if t > 0.85 {
                    self.boulder.pos = points::BOULDER_START + Vec3::Y * BOULDER_R;
                    self.boulder.speed = 1.0;
                    self.camera.add_shake(0.2);
                }
            }
            BoulderState::Rolling => {
                let b = &mut self.boulder;
                b.speed = (b.speed + dt * 9.0).min(9.5);
                b.pos.z -= b.speed * dt;
                if let Some(g) = self.col.ground_below(b.pos + Vec3::Y * 2.0, 6.0, 0.0, 0.0) {
                    b.pos.y = g.point.y + BOULDER_R;
                }
                b.roll += b.speed / BOULDER_R * dt;
                b.hit_cd -= dt;
                let bpos = b.pos;
                if (self.time * 6.0).fract() < dt * 6.0 {
                    self.camera.add_shake(0.08);
                }
                let p = &self.actors[PLAYER];
                let hit = (p.center() - bpos).length() < BOULDER_R + 0.45 && !p.has_iframes() && !p.dead;
                if hit && self.boulder.hit_cd <= 0.0 {
                    self.boulder.hit_cd = 1.0;
                    let p = &mut self.actors[PLAYER];
                    p.stats.hp -= 190.0 * (1.0 - p.stats.absorb);
                    p.flash = 1.0;
                    p.set_action(Action::Knockdown { dir: Vec3::NEG_Z });
                    self.sound_at(Sfx::HitArmor, bpos, 1.0);
                    self.camera.add_shake(0.8);
                    if self.actors[PLAYER].stats.hp <= 0.0 {
                        self.kill_actor(PLAYER);
                    }
                }
                if bpos.z <= 15.1 {
                    self.boulder.state = BoulderState::Done;
                    self.progress.set("boulder_done");
                    self.progress.set("wall_broken");
                    let wall = self.named.weak_wall;
                    if let PropKind::Breakable { broken, .. } = &mut self.props[wall].kind {
                        *broken = true;
                    }
                    let crash = Vec3::new(15.0, 7.0, 14.2);
                    self.sound_at(Sfx::WallCrash, crash, 1.0);
                    self.camera.add_shake(1.0);
                    self.fx.dust(crash, 2.5);
                    self.fx.dust(crash + Vec3::Y * 1.5, 1.5);
                    if let Some(id) = self.pusher {
                        let home = self.actors[id as usize].spawn_pos;
                        self.actors[id as usize].brain = Brain::Husk(HuskBrain::new(home, false));
                    }
                    self.save_dirty = true;
                }
            }
            BoulderState::Done => {}
        }
    }

    // ------------------------------------------------------------ Progress

    /// Applies saved progress to a fresh world.
    pub(crate) fn apply_progress(&mut self, assets: &Assets, loaded: bool) {
        let prog = self.progress.clone();
        for p in self.props.iter_mut() {
            match &mut p.kind {
                PropKind::Door(d) => {
                    if prog.has(d.flag) {
                        d.open = 1.0;
                        d.target = 1.0;
                        d.lock = DoorLock::Open;
                    }
                }
                PropKind::Gate(g) => {
                    if prog.has(g.flag) {
                        g.lift = g.open_lift;
                        g.target_lift = g.open_lift;
                        g.lock = DoorLock::Open;
                    }
                }
                PropKind::Item { flag, taken, .. } => *taken = prog.has(flag),
                PropKind::Pyre { id, lit, .. } => *lit = prog.has(&format!("pyre:{id}")),
                PropKind::Breakable { flag, broken, .. } => *broken = prog.has(flag),
                _ => {}
            }
        }
        // The escape gate's flag means "slammed shut", not "opened".
        let gate = self.named.escape_gate;
        if let PropKind::Gate(g) = &mut self.props[gate].kind {
            g.lock = DoorLock::Barred;
        }
        if prog.has("boulder_done") {
            self.boulder.state = BoulderState::Done;
            if let Some(id) = self.pusher {
                let home = self.actors[id as usize].spawn_pos;
                self.actors[id as usize].brain = Brain::Husk(HuskBrain::new(home, false));
                self.actors[id as usize].equip(Some(WeaponKind::BrokenSword), None, assets);
            }
        }
        if prog.has("boss_dead") {
            if let Some(id) = self.demon {
                let d = &mut self.actors[id as usize];
                d.dead = true;
                d.remove = true;
            }
            let fog = self.named.boss_fog;
            if let PropKind::Fog(f) = &mut self.props[fog].kind {
                f.active = false;
            }
            let gate = self.named.escape_gate;
            if let PropKind::Gate(g) = &mut self.props[gate].kind {
                g.lift = 0.0;
                g.target_lift = 0.0;
            }
        } else {
            self.reset_demon();
        }
        if loaded {
            let spawn = prog.last_pyre.clone().and_then(|id| self.pyre_spawn(&id));
            if let Some((pos, yaw)) = spawn {
                let p = &mut self.actors[PLAYER];
                p.pos = pos;
                p.yaw = yaw;
                p.spawn_pos = pos;
            }
            self.progress.flasks = self.progress.flasks_max;
            self.area_shown = true;
        } else {
            // Wake up in the cell.
            self.actors[PLAYER].set_action(Action::Sitting);
            self.actors[PLAYER].begin_use(UseTag::StandUp, Vec3::Z);
        }
    }
}
