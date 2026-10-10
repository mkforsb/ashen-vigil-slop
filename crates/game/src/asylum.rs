//! The Hollow Asylum: the opening area and the Warden Demon's arena.
//!
//! Layout (top view, +Z forward, +X to the left when facing +Z):
//! start cell -> cell corridor -> flooded room -> stair tunnel -> courtyard
//! (pyre) -> great hall door -> arena. The arena's left gate leads down to
//! the lower ward (pyre), a long hallway, a stairwell, a mist veil, the upper
//! corridor, the shortcut stairs back to the courtyard, the upper hall, the
//! boulder stairs (Sir Corvin's room behind a weak wall), the east ward
//! balcony and finally the ledge above the arena. The great door at the far
//! end of the arena opens onto the cliff path.

use crate::assets::Assets;
use crate::assets::textures::Tex;
use crate::level::{EnvKind, LevelBuilder, LevelGeometry, LightKind, Room, Side, op, surface};
use crate::props::*;
use ashen_engine::math::*;
use ashen_engine::mesh::{MeshBuilder, MeshData, rgb};
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnKind {
    Husk,
    PassiveHusk,
    Archer,
    Pusher,
    Demon,
    Corvin,
}

#[derive(Clone, Copy, Debug)]
pub struct SpawnDef {
    pub kind: SpawnKind,
    pub pos: Vec3,
    pub yaw: f32,
}

/// Indices of props the world script needs to drive.
#[derive(Clone, Copy, Debug, Default)]
pub struct Named {
    pub hall_door: usize,
    pub escape_gate: usize,
    pub boss_fog: usize,
    pub great_door: usize,
    pub weak_wall: usize,
    pub cell_gate: usize,
    pub shortcut: usize,
    pub east_gate: usize,
}

pub struct MapDef {
    pub geo: LevelGeometry,
    pub props: Vec<Prop>,
    pub spawns: Vec<SpawnDef>,
    pub player_start: (Vec3, f32),
    pub named: Named,
    pub kill_y: f32,
}

/// Points of interest used by scripted sequences.
pub mod points {
    use ashen_engine::math::Vec3;
    pub const DEMON_PERCH: Vec3 = Vec3::new(0.0, 11.0, 48.0);
    pub const DEMON_LANDING: Vec3 = Vec3::new(0.0, 3.0, 29.0);
    pub const DEMON_WAIT: Vec3 = Vec3::new(0.0, 3.0, 44.0);
    pub const BOULDER_START: Vec3 = Vec3::new(15.7, 11.0, 31.5);
    pub const NEST: Vec3 = Vec3::new(0.0, 12.0, 95.0);
    /// Arena bounds (floor level).
    pub const ARENA_MIN: Vec3 = Vec3::new(-12.0, 2.0, 16.0);
    pub const ARENA_MAX: Vec3 = Vec3::new(12.0, 10.0, 50.0);
}

const W: [u8; 4] = [255, 255, 255, 255];

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn v2(x: f32, z: f32) -> Vec2 {
    Vec2::new(x, z)
}

fn brick_block(min: Vec3, max: Vec3, tex: Tex) -> MeshData {
    let mut b = MeshBuilder::new();
    b.material(tex.layer(), rgb(200, 195, 185));
    b.cube(min, max, 0.5);
    b.finish()
}

fn water_mesh(min: Vec2, max: Vec2, y: f32) -> MeshData {
    let mut b = MeshBuilder::new();
    b.material(Tex::Water.layer(), rgb(150, 170, 165));
    let o = v(min.x, y, min.y);
    b.grid_quad(o, v(0.0, 0.0, max.y - min.y), v(max.x - min.x, 0.0, 0.0), 6, 4, &|p| Vec2::new(p.x, p.z) / 3.0, &|_| rgb(150, 170, 165));
    b.finish()
}

struct Ctx<'a> {
    b: LevelBuilder,
    props: Vec<Prop>,
    assets: &'a mut Assets,
    urn_i: usize,
}

impl Ctx<'_> {
    fn torch(&mut self, p: Vec3, normal: Vec3) {
        let yaw = dir_yaw(normal);
        self.props.push(Prop::new(p, yaw, PropKind::Decor { mesh: self.assets.props.torch, model: Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), p), radius: 0.6 }));
        self.props.push(Prop::new(p + normal * 0.27 + Vec3::Y * 0.36, 0.0, PropKind::Torch));
        self.b.light(p + normal * 0.5 + Vec3::Y * 0.4, v(1.0, 0.58, 0.28), 8.0, 1.5, LightKind::Torch);
    }

    fn message(&mut self, p: Vec3, text: &'static str) {
        self.props.push(Prop::new(p, 0.0, PropKind::Message { text }));
    }

    fn urn(&mut self, p: Vec3) {
        let mesh = self.assets.props.urns[self.urn_i % self.assets.props.urns.len()];
        self.urn_i += 1;
        self.props.push(Prop::new(p, (self.urn_i as f32 * 1.7) % 6.28, PropKind::Urn { mesh, broken: false, respawn: true }));
    }

    fn decor(&mut self, mesh: ashen_engine::render::MeshHandle, p: Vec3, yaw: f32, radius: f32) {
        self.props.push(Prop::new(p, yaw, PropKind::Decor { mesh, model: Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), p), radius }));
    }

    fn item(&mut self, p: Vec3, item: ItemKind, flag: &'static str, corpse: bool) {
        if corpse {
            let m = self.assets.props.corpses[self.urn_i % 3];
            self.urn_i += 1;
            self.decor(m, p - Vec3::Y * 0.02, 0.0, 1.0);
        }
        self.props.push(Prop::new(p + Vec3::Y * 0.15, 0.0, PropKind::Item { item, flag, taken: false }));
    }

    fn push(&mut self, p: Prop) -> usize {
        self.props.push(p);
        self.props.len() - 1
    }

    /// Double door with leaves hinged at `x0` and `x1` (on a Z-facing wall at `z`),
    /// swinging towards +Z.
    fn double_door(&mut self, x0: f32, x1: f32, z: f32, y: f32, height: f32, lock: DoorLock, flag: &'static str) -> usize {
        let group = self.b.new_group();
        self.b.col.add_box(v(x0, y, z - 0.3), v(x1, y + height, z + 0.3), group, surface::WOOD);
        let mesh = self.assets.props.big_door;
        let leaves = vec![
            Leaf { mesh, hinge: v(x0, y, z), closed_yaw: 0.0, swing: -FRAC_PI_2 * 0.92 },
            Leaf { mesh, hinge: v(x1, y, z), closed_yaw: PI, swing: FRAC_PI_2 * 0.92 },
        ];
        self.push(Prop::new(
            v((x0 + x1) * 0.5, y, z),
            0.0,
            PropKind::Door(Door { leaves, open: 0.0, target: 0.0, lock, group, side: Vec3::NEG_Z, flag, speed: 0.6 }),
        ))
    }

    /// Barred gate across an opening; `yaw` orients the bars' plane normal.
    #[allow(clippy::too_many_arguments)]
    fn gate(&mut self, mesh: ashen_engine::render::MeshHandle, base: Vec3, yaw: f32, half_w: f32, height: f32, open_lift: f32, start_open: bool, lock: DoorLock, flag: &'static str) -> usize {
        let group = self.b.new_group();
        let across = Quat::from_rotation_y(yaw) * Vec3::X;
        let a = base - across * half_w;
        let b = base + across * half_w;
        let min = a.min(b) - Vec3::new(0.12, 0.0, 0.12);
        let max = a.max(b) + Vec3::new(0.12, height, 0.12);
        self.b.col.add_box(min, max, group, surface::STONE);
        let lift = if start_open { open_lift } else { 0.0 };
        self.push(Prop::new(base, yaw, PropKind::Gate(Gate { mesh, base, yaw, lift, target_lift: lift, open_lift, group, flag, lock, speed: 2.5 })))
    }

    fn fog(&mut self, pos: Vec3, yaw: f32, width: f32, height: f32, big: bool, boss: bool) -> usize {
        let group = self.b.new_group();
        let across = Quat::from_rotation_y(yaw) * Vec3::X;
        let a = pos - across * width * 0.5;
        let b = pos + across * width * 0.5;
        self.b.col.add_box(a.min(b) - Vec3::new(0.1, 0.0, 0.1), a.max(b) + Vec3::new(0.1, height, 0.1), group, surface::STONE);
        let mesh = if big { self.assets.props.fog_big } else { self.assets.props.fog_small };
        self.push(Prop::new(pos, yaw, PropKind::Fog(Fog { mesh, yaw, width, boss, active: true, passing: false, group })))
    }
}

pub fn build(assets: &mut Assets) -> MapDef {
    let mut c = Ctx { b: LevelBuilder::new(), props: Vec::new(), assets, urn_i: 0 };
    let mut named = Named::default();
    let mut spawns = Vec::new();
    let dim = rgb(190, 185, 180);
    let mossy = rgb(200, 210, 190);

    // ---------------------------------------------------------------- Cells
    // Start cell with a light shaft from a hole in the ceiling.
    let mut cell = Room::new(-2.0, -46.0, 2.0, -42.0, 0.0, 3.5).holes(&[op(Side::ZMax, -0.8, 0.8, 0.0, 2.6)]).tint(dim).tex(Tex::StoneBrick, Tex::Flagstone, Tex::Plaster);
    cell.ceil = None;
    c.b.room(&cell);
    for (a, b) in [(v2(-2.0, -46.0), v2(2.0, -45.0)), (v2(-2.0, -43.8), v2(2.0, -42.0)), (v2(-2.0, -45.0), v2(-0.6, -43.8)), (v2(0.6, -45.0), v2(2.0, -43.8))] {
        c.b.ceiling_rect(a, b, 3.5, Tex::Plaster, dim);
    }
    // The ceiling hole is faked by a bright shaft above it.
    c.b.room(&Room::new(-0.6, -45.0, 0.6, -43.8, 3.5, 3.0).no_floor().open_sky(6.5).tint(dim));
    c.b.light(v(0.0, 3.2, -44.4), v(0.65, 0.75, 0.95), 6.0, 1.3, LightKind::Static);
    c.b.zone(v(-2.0, -1.0, -46.0), v(2.0, 4.0, -42.0), EnvKind::Cell);
    named.cell_gate = c.gate(c.assets.props.cell_bars, v(0.0, 0.0, -42.0), 0.0, 0.8, 2.6, 2.4, false, DoorLock::Locked(Key::Cell), "door_cell");
    c.item(v(0.8, 0.0, -44.9), ItemKind::Key(Key::Cell), "item_cellkey", true);
    c.message(v(-1.2, 0.0, -44.2), "Etched runes:\n[W A S D] / left stick to move. Mouse / right stick to look.\n[E] / (A) to interact.");

    // Cell corridor with side cells.
    let corridor_cells: [(f32, f32, bool); 6] = [(-36.0, 1.0, false), (-36.0, -1.0, false), (-29.0, 1.0, true), (-29.0, -1.0, false), (-22.5, 1.0, false), (-22.5, -1.0, true)];
    let mut corridor_holes = vec![op(Side::ZMin, -0.8, 0.8, 0.0, 2.6), op(Side::ZMax, -1.5, 1.5, 0.0, 3.6)];
    for &(z, side, _) in &corridor_cells {
        let s = if side > 0.0 { Side::XMax } else { Side::XMin };
        corridor_holes.push(op(s, z - 0.8, z + 0.8, 0.0, 2.6));
    }
    c.b.room(&Room::new(-1.5, -42.0, 1.5, -20.0, 0.0, 3.6).holes(&corridor_holes).tint(dim).tex(Tex::StoneBrick, Tex::Flagstone, Tex::Plaster));
    for &(z, side, open) in &corridor_cells {
        let (x0, x1, s) = if side > 0.0 { (1.5, 4.5, Side::XMin) } else { (-4.5, -1.5, Side::XMax) };
        c.b.room(&Room::new(x0, z - 2.0, x1, z + 2.0, 0.0, 3.0).holes(&[op(s, z - 0.8, z + 0.8, 0.0, 2.6)]).tint(rgb(160, 150, 140)));
        let bars_x = side * 1.5;
        if open {
            // Broken-open cell: bars lie on the floor.
            c.decor(c.assets.props.cell_bars, v(side * 3.2, 0.05, z + 0.4), FRAC_PI_2 * side, 2.0);
            c.decor(c.assets.props.corpses[(z.abs() as usize) % 3], v(side * 3.6, 0.0, z - 1.2), 1.0, 1.0);
        } else {
            let mesh = c.assets.props.cell_bars;
            let yaw = FRAC_PI_2;
            c.props.push(Prop::new(v(bars_x, 0.0, z), yaw, PropKind::Decor { mesh, model: Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), v(bars_x, 0.0, z)), radius: 2.0 }));
            c.b.blocker(v(bars_x - 0.1, 0.0, z - 0.8), v(bars_x + 0.1, 2.6, z + 0.8));
        }
    }
    spawns.push(SpawnDef { kind: SpawnKind::PassiveHusk, pos: v(3.0, 0.0, -29.4), yaw: -FRAC_PI_2 });
    spawns.push(SpawnDef { kind: SpawnKind::PassiveHusk, pos: v(-3.1, 0.0, -22.0), yaw: FRAC_PI_2 });
    c.torch(v(1.5, 1.9, -39.0), Vec3::NEG_X);
    c.torch(v(-1.5, 1.9, -32.5), Vec3::X);
    c.torch(v(1.5, 1.9, -25.5), Vec3::NEG_X);
    c.message(v(0.0, 0.0, -38.0), "Etched runes:\nTap [Space] / (B) to roll. Hold it while moving to sprint.\nTap it standing still to step back.");
    c.message(v(0.0, 0.0, -31.5), "Etched runes:\n[Left click] / (RB) light attack. [Shift + Left click] or [F] / (RT) heavy attack.\nPress again to chain blows.");
    c.b.zone(v(-5.0, -1.0, -42.0), v(5.0, 4.0, -20.0), EnvKind::Dungeon);

    // Flooded room.
    c.b.room(
        &Room::new(-6.0, -20.0, 6.0, -12.0, 0.0, 5.5)
            .holes(&[op(Side::ZMin, -1.5, 1.5, 0.0, 3.6), op(Side::ZMax, -5.5, -2.5, 0.0, 4.5)])
            .tex(Tex::MossBrick, Tex::Flagstone, Tex::Plaster)
            .tint(mossy)
            .surface(surface::WATER),
    );
    let wm = c.assets.add_mesh(water_mesh(v2(-6.0, -20.0), v2(6.0, -12.0), 0.18));
    c.props.push(Prop::new(v(0.0, 0.18, -16.0), 0.0, PropKind::Water { mesh: wm }));
    for x in [-2.2f32, 2.2] {
        c.b.block(v(x - 0.4, 0.0, -16.4), v(x + 0.4, 5.5, -15.6), Tex::MossBrick, mossy);
    }
    c.torch(v(6.0, 2.2, -16.0), Vec3::NEG_X);
    c.torch(v(-6.0, 2.2, -16.0), Vec3::X);
    c.torch(v(-1.6, 2.4, -12.0), Vec3::NEG_Z);
    c.b.light(v(0.0, 4.5, -16.0), v(0.45, 0.6, 0.5), 9.0, 0.6, LightKind::Static);
    c.item(v(4.8, 0.0, -13.2), ItemKind::Cinders(200), "item_ember1", true);
    c.message(v(-1.0, 0.18, -14.0), "Etched runes:\nHold [Right click] / (LB) to raise your shield.\nEach blow you block drains stamina.");
    c.b.zone(v(-6.0, -1.0, -20.0), v(6.0, 6.0, -12.0), EnvKind::Dungeon);

    // Stair tunnel up to the courtyard.
    c.b.room(
        &Room::new(-5.5, -12.0, -2.5, -6.0, 0.0, 7.0)
            .no_floor()
            .holes(&[op(Side::ZMin, -5.5, -2.5, 0.0, 4.5), op(Side::ZMax, -5.5, -2.5, 3.0, 6.0)])
            .tex(Tex::MossBrick, Tex::Flagstone, Tex::Plaster)
            .tint(mossy),
    );
    c.b.stairs(true, -12.0, -6.0, -5.5, -2.5, 0.0, 3.0, Tex::MossBrick, mossy);
    c.b.zone(v(-5.5, -1.0, -12.0), v(-2.5, 7.0, -6.0), EnvKind::Dungeon);

    // ------------------------------------------------------------ Courtyard
    c.b.room(
        &Room::new(-6.0, -6.0, 6.0, 14.0, 3.0, 12.0)
            .open_sky(15.0)
            .holes(&[
                op(Side::ZMin, -5.5, -2.5, 3.0, 6.0),
                op(Side::ZMax, -2.5, 2.5, 3.0, 9.0),
                op(Side::XMax, -4.8, -3.2, 3.0, 5.8),
                op(Side::XMax, 6.0, 12.0, 7.9, 9.6),
            ])
            .tex(Tex::MossBrick, Tex::Flagstone, Tex::Plaster)
            .tint(rgb(215, 215, 205)),
    );
    c.props.push(Prop::new(v(0.0, 3.0, 4.0), 0.0, PropKind::Pyre { id: "pyre_courtyard", name: "Asylum Courtyard", lit: false }));
    c.b.light(v(0.0, 4.0, 4.0), v(1.0, 0.55, 0.25), 10.0, 0.0, LightKind::Pyre);
    c.torch(v(-3.6, 5.8, 14.0), Vec3::NEG_Z);
    c.torch(v(3.6, 5.8, 14.0), Vec3::NEG_Z);
    c.decor(c.assets.props.rubble, v(-4.2, 3.0, 9.5), 0.3, 2.0);
    c.decor(c.assets.props.corpses[1], v(4.3, 3.0, 0.5), 2.2, 1.0);
    c.message(v(1.4, 3.0, 2.2), "Etched runes:\nRest at a pyre to mend your wounds and refill Ember Flasks.\nBut all you have slain will rise again.");
    c.message(v(0.0, 3.0, 11.8), "Etched runes:\nThe warden waits beyond. Turn back, or run for the gate on the left.");
    c.b.zone(v(-6.0, 2.0, -6.0), v(6.0, 20.0, 14.0), EnvKind::Courtyard);

    // Passage + great hall door into the arena.
    c.b.room(&Room::new(-2.5, 14.0, 2.5, 16.0, 3.0, 6.0).holes(&[op(Side::ZMin, -2.5, 2.5, 3.0, 9.0), op(Side::ZMax, -2.5, 2.5, 3.0, 9.0)]).tint(dim));
    named.hall_door = c.double_door(-2.5, 2.5, 15.0, 3.0, 6.0, DoorLock::Open, "door_hall");

    // ---------------------------------------------------------------- Arena
    c.b.room(
        &Room::new(-12.0, 16.0, 12.0, 50.0, 3.0, 16.0)
            .open_sky(19.0)
            .holes(&[
                op(Side::ZMin, -2.5, 2.5, 3.0, 9.0),
                op(Side::XMax, 18.5, 21.5, 3.0, 6.6),
                op(Side::XMax, 46.5, 49.5, 11.0, 14.5),
                op(Side::ZMax, -2.5, 2.5, 3.0, 9.0),
            ])
            .tex(Tex::Plaster, Tex::ArenaFloor, Tex::Plaster)
            .tint(rgb(205, 200, 195)),
    );
    // Wall pilasters for rhythm.
    for i in 0..4 {
        let z = 22.0 + i as f32 * 6.0;
        if (z - 20.0).abs() > 2.5 {
            c.b.block(v(10.9, 3.0, z - 0.6), v(12.0, 19.0, z + 0.6), Tex::StoneBrick, W);
        }
        c.b.block(v(-12.0, 3.0, z - 0.6), v(-10.9, 19.0, z + 0.6), Tex::StoneBrick, W);
    }
    // The perch ledge.
    c.b.block(v(-8.0, 10.4, 46.0), v(12.0, 11.0, 50.0), Tex::StoneBrick, W);
    for x in [-6.0f32, -1.0, 4.0, 9.0] {
        c.b.block(v(x - 0.4, 9.2, 49.0), v(x + 0.4, 10.4, 50.0), Tex::StoneBrick, W);
    }
    // Low invisible lip so the player can't walk off the ledge's back.
    c.b.blocker(v(-8.2, 11.0, 50.0), v(12.0, 14.0, 50.3));
    for z in [20.0f32, 25.0, 30.0, 36.0, 42.0] {
        c.urn(v(-11.0, 3.0, z));
        c.urn(v(-10.4, 3.0, z + 1.1));
    }
    for z in [27.0f32, 33.0, 39.0] {
        c.urn(v(10.6, 3.0, z));
        c.urn(v(10.0, 3.0, z + 1.4));
    }
    for x in [-10.0f32, -6.0, 6.0, 9.5] {
        c.urn(v(x, 3.0, 49.0));
        c.urn(v(x + 0.9, 3.0, 48.6));
    }
    for x in [-9.0f32, 7.0] {
        c.urn(v(x, 3.0, 17.0));
    }
    c.decor(c.assets.props.rubble, v(-7.0, 3.0, 36.0), 1.0, 2.0);
    c.decor(c.assets.props.rubble, v(6.5, 3.0, 24.0), 2.0, 2.0);
    c.torch(v(-3.4, 6.5, 50.0), Vec3::NEG_Z);
    c.torch(v(3.4, 6.5, 50.0), Vec3::NEG_Z);
    c.b.zone(v(-12.0, 2.0, 16.0), v(12.0, 25.0, 50.0), EnvKind::Arena);

    // Escape gate (portcullis) and stair down to the lower ward.
    named.escape_gate = c.gate(c.assets.props.portcullis, v(12.5, 3.0, 20.0), FRAC_PI_2, 1.5, 3.6, 3.4, true, DoorLock::Barred, "gate_escape");
    c.b.room(&Room::new(12.0, 18.5, 13.0, 21.5, 3.0, 3.6).holes(&[op(Side::XMin, 18.5, 21.5, 3.0, 6.6), op(Side::XMax, 18.5, 21.5, 3.0, 6.6)]).tint(dim));

    // ---------------------------------------------------------- Lower ward
    c.b.room(
        &Room::new(13.0, 14.0, 21.0, 26.0, 0.0, 6.6)
            .holes(&[op(Side::XMin, 18.5, 21.5, 3.0, 6.6), op(Side::ZMin, 14.0, 17.0, 0.0, 3.6)])
            .tint(dim),
    );
    c.b.block(v(13.0, 0.0, 18.5), v(14.5, 3.0, 21.5), Tex::StoneBrick, dim);
    c.b.stairs(false, 19.0, 14.5, 18.5, 21.5, 0.0, 3.0, Tex::StoneBrick, dim);
    c.props.push(Prop::new(v(19.5, 0.0, 24.0), 0.0, PropKind::Pyre { id: "pyre_ward", name: "Lower Ward", lit: false }));
    c.b.light(v(19.5, 1.0, 24.0), v(1.0, 0.55, 0.25), 10.0, 0.0, LightKind::Pyre);
    c.torch(v(21.0, 2.4, 17.0), Vec3::NEG_X);
    c.message(v(17.0, 0.0, 23.5), "Etched runes:\n[Q] / (R3) locks on to a foe. [R] / (X) drinks from an Ember Flask.");
    c.b.zone(v(13.0, -1.0, 14.0), v(21.0, 6.6, 26.0), EnvKind::Dungeon);

    // Long hallway with cells on its left.
    c.b.room(
        &Room::new(14.0, -4.0, 17.0, 14.0, 0.0, 4.0)
            .holes(&[
                op(Side::ZMax, 14.0, 17.0, 0.0, 3.6),
                op(Side::ZMin, 14.0, 17.0, 0.0, 4.0),
                op(Side::XMax, 0.7, 2.3, 0.0, 2.6),
                op(Side::XMax, 6.2, 7.8, 0.0, 2.6),
                op(Side::XMax, 10.7, 12.3, 0.0, 2.6),
            ])
            .tint(dim),
    );
    for (z, open) in [(1.5f32, true), (7.0, false), (11.5, true)] {
        c.b.room(&Room::new(17.0, z - 2.0, 20.0, z + 2.0, 0.0, 3.0).holes(&[op(Side::XMin, z - 0.8, z + 0.8, 0.0, 2.6)]).tint(rgb(160, 150, 140)));
        if !open {
            let yaw = FRAC_PI_2;
            let mesh = c.assets.props.cell_bars;
            c.props.push(Prop::new(v(17.0, 0.0, z), yaw, PropKind::Decor { mesh, model: Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), v(17.0, 0.0, z)), radius: 2.0 }));
            c.b.blocker(v(16.9, 0.0, z - 0.8), v(17.1, 2.6, z + 0.8));
        }
    }
    c.item(v(19.0, 0.0, 1.0), ItemKind::Cinders(300), "item_ember2", true);
    spawns.push(SpawnDef { kind: SpawnKind::Husk, pos: v(18.6, 0.0, 11.5), yaw: -FRAC_PI_2 });
    spawns.push(SpawnDef { kind: SpawnKind::Archer, pos: v(15.5, 0.0, -2.8), yaw: 0.0 });
    c.torch(v(14.0, 2.0, 9.0), Vec3::X);
    c.torch(v(14.0, 2.0, 0.0), Vec3::X);
    c.message(v(15.5, 0.0, 12.6), "Etched runes:\nShield up. Arrows ahead.");
    c.b.zone(v(14.0, -1.0, -4.0), v(20.0, 4.0, 14.0), EnvKind::Dungeon);

    // Stairwell up to the second floor.
    c.b.room(
        &Room::new(14.0, -18.0, 17.0, -4.0, 0.0, 10.5)
            .no_floor()
            .holes(&[op(Side::ZMax, 14.0, 17.0, 0.0, 4.0), op(Side::ZMin, 14.0, 17.0, 7.0, 10.0)])
            .tint(dim),
    );
    c.b.stairs(true, -4.0, -18.0, 14.0, 17.0, 0.0, 7.0, Tex::StoneBrick, dim);
    c.torch(v(17.0, 4.5, -11.0), Vec3::NEG_X);
    c.b.zone(v(14.0, -1.0, -18.0), v(17.0, 10.5, -4.0), EnvKind::Dungeon);

    // Landing and mist veil.
    c.b.room(
        &Room::new(13.5, -22.0, 17.5, -18.0, 7.0, 3.5)
            .holes(&[op(Side::ZMax, 14.0, 17.0, 7.0, 10.0), op(Side::XMin, -21.5, -18.5, 7.0, 10.0)])
            .tint(dim),
    );
    c.fog(v(13.5, 7.0, -20.0), FRAC_PI_2, 3.0, 3.0, false, false);
    c.b.zone(v(13.5, 6.0, -22.0), v(17.5, 10.5, -18.0), EnvKind::Dungeon);

    // Upper corridor back towards the courtyard.
    c.b.room(
        &Room::new(10.0, -22.0, 13.5, 4.0, 7.0, 3.5)
            .holes(&[op(Side::XMax, -21.5, -18.5, 7.0, 10.0), op(Side::ZMax, 10.0, 13.0, 7.0, 10.0)])
            .tint(dim),
    );
    c.torch(v(10.0, 9.0, -14.0), Vec3::X);
    c.torch(v(13.5, 9.0, -5.0), Vec3::NEG_X);
    c.decor(c.assets.props.corpses[2], v(12.0, 7.0, -10.0), 0.7, 1.0);
    c.b.zone(v(10.0, 6.0, -22.0), v(13.5, 10.5, 4.0), EnvKind::Dungeon);

    // Shortcut stairs down to the courtyard door.
    c.b.room(
        &Room::new(7.0, -6.0, 10.0, 4.0, 3.0, 7.5)
            .no_floor()
            .holes(&[op(Side::ZMax, 7.0, 10.0, 7.0, 10.0), op(Side::XMin, -4.8, -3.2, 3.0, 5.8)])
            .tint(dim),
    );
    c.b.floor_rect(v2(7.0, -6.0), v2(10.0, -2.0), 3.0, Tex::Flagstone, dim, surface::STONE);
    c.b.stairs(true, 4.0, -2.0, 7.0, 10.0, 7.0, 3.0, Tex::StoneBrick, dim);
    c.b.room(&Room::new(6.0, -4.8, 7.0, -3.2, 3.0, 2.8).holes(&[op(Side::XMin, -4.8, -3.2, 3.0, 5.8), op(Side::XMax, -4.8, -3.2, 3.0, 5.8)]).tint(dim));
    {
        let group = c.b.new_group();
        c.b.col.add_box(v(6.4, 3.0, -4.8), v(6.6, 5.8, -3.2), group, surface::WOOD);
        let leaves = vec![Leaf { mesh: c.assets.props.door, hinge: v(6.5, 3.0, -4.75), closed_yaw: -FRAC_PI_2, swing: FRAC_PI_2 * 0.95 }];
        named.shortcut = c.push(Prop::new(
            v(6.5, 3.0, -4.0),
            0.0,
            PropKind::Door(Door { leaves, open: 0.0, target: 0.0, lock: DoorLock::OneWay, group, side: Vec3::X, flag: "door_shortcut", speed: 1.2 }),
        ));
    }
    c.b.zone(v(6.0, 2.0, -6.0), v(10.0, 10.5, 4.0), EnvKind::Dungeon);

    // Upper hall overlooking the courtyard.
    c.b.room(
        &Room::new(7.0, 4.0, 13.0, 14.0, 7.0, 4.0)
            .holes(&[op(Side::XMin, 6.0, 12.0, 7.9, 9.6), op(Side::ZMin, 7.0, 13.0, 7.0, 10.0), op(Side::ZMax, 7.0, 13.0, 7.0, 10.0)])
            .tint(dim),
    );
    // Window through the courtyard wall, with railing bars.
    c.b.room(&Room::new(6.0, 6.0, 7.0, 12.0, 7.9, 1.7).holes(&[op(Side::XMin, 6.0, 12.0, 7.9, 9.6), op(Side::XMax, 6.0, 12.0, 7.9, 9.6)]).tint(dim));
    c.b.blocker(v(6.4, 7.0, 6.0), v(6.6, 11.0, 12.0));
    for z in [7.5f32, 10.5] {
        let mesh = c.assets.props.portcullis;
        let model = Mat4::from_rotation_translation(Quat::from_rotation_y(FRAC_PI_2), v(6.5, 7.9, z)) * Mat4::from_scale(v(0.85, 0.42, 1.0));
        c.props.push(Prop::new(v(6.5, 7.9, z), 0.0, PropKind::Decor { mesh, model, radius: 3.0 }));
    }
    c.torch(v(13.0, 9.0, 9.0), Vec3::NEG_X);
    c.b.zone(v(7.0, 6.0, 4.0), v(13.0, 11.0, 14.0), EnvKind::Dungeon);

    // Landing in front of the boulder stairs; the weak wall hides Sir Corvin.
    c.b.room(
        &Room::new(7.0, 14.0, 17.0, 16.0, 7.0, 3.5)
            .holes(&[op(Side::ZMin, 7.0, 13.0, 7.0, 10.0), op(Side::ZMin, 13.5, 16.5, 7.0, 10.0), op(Side::ZMax, 13.0, 17.0, 7.0, 11.0)])
            .tint(dim),
    );
    c.message(v(10.0, 7.0, 15.0), "Etched runes:\nSomething lurks above. Watch the stairs.");
    let wall_mesh = c.assets.add_mesh(brick_block(v(13.5, 7.0, 13.7), v(16.5, 10.0, 14.3), Tex::StoneBrick));
    {
        let group = c.b.new_group();
        c.b.col.add_box(v(13.5, 7.0, 13.7), v(16.5, 10.0, 14.3), group, surface::STONE);
        named.weak_wall = c.push(Prop::new(v(15.0, 7.0, 14.0), 0.0, PropKind::Breakable { mesh: wall_mesh, group, broken: false, flag: "wall_broken" }));
    }
    c.b.zone(v(7.0, 6.0, 14.0), v(17.0, 10.5, 16.0), EnvKind::Dungeon);

    // Sir Corvin's sealed room.
    c.b.room(&Room::new(13.5, 6.0, 19.0, 14.0, 7.0, 3.0).holes(&[op(Side::ZMax, 13.5, 16.5, 7.0, 10.0)]).tint(rgb(170, 160, 150)));
    spawns.push(SpawnDef { kind: SpawnKind::Corvin, pos: v(18.3, 7.0, 9.5), yaw: -FRAC_PI_2 });
    c.decor(c.assets.props.rubble, v(17.5, 7.0, 7.0), 0.5, 2.0);
    c.b.light(v(16.0, 9.5, 10.0), v(0.55, 0.6, 0.75), 6.0, 0.8, LightKind::Static);
    c.b.zone(v(13.5, 6.0, 6.0), v(19.0, 10.0, 14.0), EnvKind::Dungeon);

    // Boulder stairs.
    c.b.room(
        &Room::new(13.0, 16.0, 17.0, 30.0, 7.0, 7.5)
            .no_floor()
            .holes(&[op(Side::ZMin, 13.0, 17.0, 7.0, 11.0), op(Side::ZMax, 13.0, 17.0, 11.0, 14.0)])
            .tint(dim),
    );
    c.b.stairs(true, 16.0, 30.0, 13.0, 17.0, 7.0, 11.0, Tex::StoneBrick, dim);
    c.torch(v(13.0, 10.5, 23.0), Vec3::X);
    c.b.zone(v(13.0, 6.0, 16.0), v(17.0, 14.5, 30.0), EnvKind::Dungeon);

    // Top landing with the boulder and the east ward gate.
    c.b.room(&Room::new(13.0, 30.0, 17.0, 34.0, 11.0, 3.5).holes(&[op(Side::ZMin, 13.0, 17.0, 11.0, 14.0), op(Side::ZMax, 14.0, 16.0, 11.0, 13.6)]).tint(dim));
    spawns.push(SpawnDef { kind: SpawnKind::Pusher, pos: v(15.7, 11.0, 33.2), yaw: PI });
    named.east_gate = c.gate(c.assets.props.cell_bars, v(15.0, 11.0, 34.0), 0.0, 1.0, 2.6, 2.4, false, DoorLock::Locked(Key::EastWard), "gate_east");
    c.b.zone(v(13.0, 10.0, 30.0), v(17.0, 14.5, 34.0), EnvKind::Dungeon);

    // East ward balcony (open air).
    c.b.room(
        &Room::new(13.0, 34.0, 22.0, 44.0, 11.0, 4.0)
            .open_sky(16.0)
            .walls([true, true, false, true])
            .holes(&[op(Side::ZMin, 14.0, 16.0, 11.0, 13.6), op(Side::ZMax, 13.0, 16.0, 11.0, 14.0)])
            .tex(Tex::StoneBrick, Tex::Flagstone, Tex::Plaster),
    );
    c.b.block(v(21.6, 11.0, 34.0), v(22.2, 12.0, 44.0), Tex::StoneBrick, W);
    c.b.blocker(v(21.8, 12.0, 34.0), v(22.2, 14.0, 44.0));
    c.b.block(v(22.2, 0.0, 33.8), v(23.0, 11.0, 44.2), Tex::Rock, W);
    spawns.push(SpawnDef { kind: SpawnKind::Husk, pos: v(16.0, 11.0, 38.5), yaw: PI });
    spawns.push(SpawnDef { kind: SpawnKind::Husk, pos: v(19.5, 11.0, 41.0), yaw: PI });
    spawns.push(SpawnDef { kind: SpawnKind::Archer, pos: v(20.5, 11.0, 43.0), yaw: PI });
    c.item(v(21.0, 11.0, 35.0), ItemKind::Cinders(400), "item_ember3", true);
    c.b.zone(v(13.0, 10.0, 34.0), v(23.0, 20.0, 44.0), EnvKind::Outside);

    // North corridor and the boss mist onto the ledge.
    c.b.room(&Room::new(13.0, 44.0, 16.0, 50.0, 11.0, 3.5).holes(&[op(Side::ZMin, 13.0, 16.0, 11.0, 14.0), op(Side::XMin, 46.5, 49.5, 11.0, 14.5)]).tint(dim));
    c.b.room(&Room::new(12.0, 46.5, 13.0, 49.5, 11.0, 3.5).holes(&[op(Side::XMin, 46.5, 49.5, 11.0, 14.5), op(Side::XMax, 46.5, 49.5, 11.0, 14.5)]).tint(dim));
    named.boss_fog = c.fog(v(12.5, 11.0, 48.0), FRAC_PI_2, 3.0, 3.5, false, true);
    c.torch(v(16.0, 13.0, 47.0), Vec3::NEG_X);
    c.message(v(14.5, 11.0, 46.0), "Etched runes:\nStrike while falling from a height to deliver a plunging blow.");
    c.b.zone(v(12.0, 10.0, 44.0), v(16.0, 14.5, 50.0), EnvKind::Dungeon);

    // ---------------------------------------------------------- Cliff path
    c.b.room(&Room::new(-2.5, 50.0, 2.5, 51.5, 3.0, 6.0).holes(&[op(Side::ZMin, -2.5, 2.5, 3.0, 9.0), op(Side::ZMax, -2.5, 2.5, 3.0, 9.0)]).tint(dim));
    named.great_door = c.double_door(-2.5, 2.5, 50.75, 3.0, 6.0, DoorLock::Locked(Key::Great), "door_great");
    let earth = rgb(220, 220, 210);
    c.b.floor_rect(v2(-5.0, 51.5), v2(5.0, 58.0), 3.0, Tex::Dirt, earth, surface::EARTH);
    // Ramp up the cliff.
    {
        let o = v(-4.0, 3.0, 58.0);
        let su = v(0.0, 9.0, 26.0);
        let sv = v(8.0, 0.0, 0.0);
        c.b.surface(o, su, sv, Tex::Grass, earth, 3.0, Some((0, surface::EARTH)));
    }
    c.b.floor_rect(v2(-11.0, 84.0), v2(11.0, 102.0), 12.0, Tex::Grass, earth, surface::EARTH);
    // Cliff walls and rims.
    c.b.block(v(-14.0, -2.0, 51.5), v(-5.0, 22.0, 84.0), Tex::Rock, W);
    c.b.block(v(5.0, -2.0, 51.5), v(14.0, 22.0, 84.0), Tex::Rock, W);
    c.b.block(v(-5.0, -2.0, 58.0), v(-4.0, 6.0, 84.0), Tex::Rock, rgb(200, 200, 200));
    c.b.block(v(4.0, -2.0, 58.0), v(5.0, 6.0, 84.0), Tex::Rock, rgb(200, 200, 200));
    c.b.block(v(-12.0, 4.0, 84.0), v(-11.0, 13.0, 102.0), Tex::Rock, W);
    c.b.block(v(11.0, 4.0, 84.0), v(12.0, 13.0, 102.0), Tex::Rock, W);
    c.b.block(v(-12.0, 4.0, 102.0), v(12.0, 13.0, 103.0), Tex::Rock, W);
    c.b.blocker(v(-12.0, 12.0, 84.0), v(-11.0, 16.0, 103.0));
    c.b.blocker(v(11.0, 12.0, 84.0), v(12.0, 16.0, 103.0));
    c.b.blocker(v(-12.0, 12.0, 102.0), v(12.0, 16.0, 103.0));
    for (i, (x, z)) in [(6.5f32, 88.0f32), (8.0, 90.5), (6.8, 93.0)].iter().enumerate() {
        c.decor(c.assets.props.gravestones[i], v(*x, 12.0, *z), 2.9 + i as f32 * 0.2, 1.5);
    }
    c.decor(c.assets.props.nest, points::NEST, 0.0, 3.0);
    c.item(v(-8.0, 12.0, 92.0), ItemKind::Cinders(250), "item_ember4", true);
    c.b.zone(v(-14.0, 0.0, 51.5), v(14.0, 30.0, 104.0), EnvKind::Outside);

    // The Warden Demon.
    spawns.push(SpawnDef { kind: SpawnKind::Demon, pos: points::DEMON_PERCH, yaw: PI });

    let geo = c.b.finish(10.0);
    MapDef { geo, props: c.props, spawns, player_start: (v(-0.7, 0.0, -44.6), 0.0), named, kill_y: -15.0 }
}
