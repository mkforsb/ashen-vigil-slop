//! Interactive and decorative props.

use ashen_engine::collision::CollisionWorld;
use ashen_engine::math::*;
use ashen_engine::render::MeshHandle;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Key {
    Cell,
    EastWard,
    Great,
}

impl Key {
    pub fn name(self) -> &'static str {
        match self {
            Key::Cell => "Cell Key",
            Key::EastWard => "East Ward Key",
            Key::Great => "Great Asylum Key",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Key::Cell => "A rusted key, dropped through the ceiling by an unseen hand.",
            Key::EastWard => "Opens the gate to the upper east ward of the asylum.",
            Key::Great => "Heavy iron key to the asylum's great outer door.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Key(Key),
    Cinders(u32),
}

impl ItemKind {
    pub fn label(self) -> String {
        match self {
            ItemKind::Key(k) => k.name().to_string(),
            ItemKind::Cinders(n) => format!("Faded Ember ({n} cinders)"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorLock {
    Open,
    Locked(Key),
    /// Can only be opened from the side its `side` normal points to.
    OneWay,
    /// Cannot be opened at all right now.
    Barred,
}

#[derive(Clone, Debug)]
pub struct Leaf {
    pub mesh: MeshHandle,
    /// Hinge point in world space (bottom of the hinge edge).
    pub hinge: Vec3,
    pub closed_yaw: f32,
    /// Rotation when fully open, relative to closed.
    pub swing: f32,
}

#[derive(Clone, Debug)]
pub struct Door {
    pub leaves: Vec<Leaf>,
    /// 0 = closed, 1 = open.
    pub open: f32,
    pub target: f32,
    pub lock: DoorLock,
    pub group: u16,
    /// Normal pointing to the side the player must stand on (one-way doors).
    pub side: Vec3,
    pub flag: &'static str,
    pub speed: f32,
}

#[derive(Clone, Debug)]
pub struct Gate {
    pub mesh: MeshHandle,
    pub base: Vec3,
    pub yaw: f32,
    pub lift: f32,
    pub target_lift: f32,
    pub open_lift: f32,
    pub group: u16,
    pub flag: &'static str,
    pub lock: DoorLock,
    pub speed: f32,
}

#[derive(Clone, Debug)]
pub struct Fog {
    pub mesh: MeshHandle,
    pub yaw: f32,
    pub width: f32,
    pub boss: bool,
    pub active: bool,
    /// The player is walking through right now (collision off).
    pub passing: bool,
    pub group: u16,
}

#[derive(Clone, Debug)]
pub enum PropKind {
    Decor { mesh: MeshHandle, model: Mat4, radius: f32 },
    Pyre { id: &'static str, name: &'static str, lit: bool },
    Door(Door),
    Gate(Gate),
    Fog(Fog),
    Item { item: ItemKind, flag: &'static str, taken: bool },
    Message { text: &'static str },
    Urn { mesh: MeshHandle, broken: bool, respawn: bool },
    Breakable { mesh: MeshHandle, group: u16, broken: bool, flag: &'static str },
    Water { mesh: MeshHandle },
    Torch,
}

#[derive(Clone, Debug)]
pub struct Prop {
    pub pos: Vec3,
    pub yaw: f32,
    pub kind: PropKind,
}

impl Prop {
    pub fn new(pos: Vec3, yaw: f32, kind: PropKind) -> Self {
        Prop { pos, yaw, kind }
    }

    /// Where the player should stand/aim to interact, and the reach.
    pub fn interact_point(&self) -> Option<(Vec3, f32)> {
        match &self.kind {
            PropKind::Pyre { .. } => Some((self.pos + Vec3::Y * 0.5, 2.0)),
            PropKind::Door(d) if d.open < 0.05 => Some((self.pos + Vec3::Y, 2.2)),
            PropKind::Gate(g) if g.lift < 0.05 && g.lock != DoorLock::Barred => Some((self.pos + Vec3::Y, 2.0)),
            PropKind::Fog(f) if f.active => Some((self.pos + Vec3::Y, 2.0)),
            PropKind::Item { taken: false, .. } => Some((self.pos + Vec3::Y * 0.3, 1.6)),
            PropKind::Message { .. } => Some((self.pos, 1.4)),
            _ => None,
        }
    }

    pub fn prompt(&self) -> Option<&'static str> {
        Some(match &self.kind {
            PropKind::Pyre { lit: false, .. } => "Kindle the pyre",
            PropKind::Pyre { lit: true, .. } => "Rest at the pyre",
            PropKind::Door(_) | PropKind::Gate(_) => "Open",
            PropKind::Fog(_) => "Traverse the mist",
            PropKind::Item { .. } => "Pick up",
            PropKind::Message { .. } => "Read the etched runes",
            _ => return None,
        })
    }

    /// Animates doors and gates; keeps collision in sync.
    pub fn update(&mut self, dt: f32, col: &mut CollisionWorld) -> Option<PropEvent> {
        match &mut self.kind {
            PropKind::Door(d) => {
                let was = d.open;
                d.open = approach(d.open, d.target, dt * d.speed);
                col.set_group_enabled(d.group, d.open < 0.6);
                if was > 0.0 && d.open == 0.0 && d.target == 0.0 {
                    return Some(PropEvent::Slammed(self.pos));
                }
            }
            PropKind::Gate(g) => {
                let was = g.lift;
                g.lift = approach(g.lift, g.target_lift, dt * g.speed);
                col.set_group_enabled(g.group, g.lift < 2.0);
                if was > 0.0 && g.lift == 0.0 && g.target_lift == 0.0 {
                    return Some(PropEvent::Slammed(self.pos));
                }
            }
            PropKind::Fog(f) => col.set_group_enabled(f.group, f.active && !f.passing),
            PropKind::Breakable { group, broken, .. } => col.set_group_enabled(*group, !*broken),
            _ => {}
        }
        None
    }
}

#[derive(Clone, Copy, Debug)]
pub enum PropEvent {
    Slammed(Vec3),
}

/// Arrows and other projectiles.
#[derive(Clone, Debug)]
pub struct Projectile {
    pub pos: Vec3,
    pub vel: Vec3,
    pub owner: u32,
    pub damage: f32,
    pub stuck: Option<f32>,
    pub alive: bool,
}
