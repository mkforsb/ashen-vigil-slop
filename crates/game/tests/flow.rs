//! Drives the real game through the asylum's progression with simulated input.

use ashen_engine::math::{Vec2, Vec3};
use ashen_game::actor::Action;
use ashen_game::input::{Button, InputState};
use ashen_game::props::{Key, PropKind};
use ashen_game::world::{Encounter, WorldState};
use ashen_game::{Game, UiCommand};

fn tick(g: &mut Game, secs: f32, inp: &InputState) {
    for _ in 0..(secs * 60.0).round() as usize {
        g.frame(1.0 / 60.0, inp);
    }
}

fn press(g: &mut Game, b: Button) {
    let mut i = InputState::default();
    i.set(b, true);
    tick(g, 0.05, &i);
    tick(g, 0.05, &InputState::default());
}

fn walk(g: &mut Game, dir: Vec2, secs: f32) {
    let i = InputState { move_axis: dir, ..Default::default() };
    tick(g, secs, &i);
}

fn warp(g: &mut Game, x: f32, y: f32, z: f32, yaw: f32) {
    g.command(UiCommand::Debug(format!("warp {x} {y} {z} {yaw}")));
    tick(g, 0.2, &InputState::default());
}

fn new_game() -> Game {
    let mut g = Game::new();
    g.command(UiCommand::NewGame);
    tick(&mut g, 2.5, &InputState::default());
    g
}

#[test]
fn cell_key_opens_cell() {
    let mut g = new_game();
    // Face the corpse with the key and pick it up.
    warp(&mut g, 0.5, 0.0, -44.0, 160.0);
    assert_eq!(g.world.prompt.as_deref(), Some("Pick up"), "prompt near key");
    press(&mut g, Button::Interact);
    tick(&mut g, 1.5, &InputState::default());
    assert!(g.world.progress.keys.contains(&Key::Cell));
    // Open the barred cell door.
    warp(&mut g, 0.0, 0.0, -43.2, 0.0);
    assert_eq!(g.world.prompt.as_deref(), Some("Open"));
    press(&mut g, Button::Interact);
    tick(&mut g, 2.5, &InputState::default());
    walk(&mut g, Vec2::Y, 2.0);
    let p = g.world.player().pos;
    assert!(p.z > -41.0, "walked out of the cell: {p:?}");
}

#[test]
fn corvin_gives_flasks_and_key() {
    let mut g = new_game();
    warp(&mut g, 16.8, 7.0, 9.5, 90.0);
    assert_eq!(g.world.prompt.as_deref(), Some("Talk"));
    press(&mut g, Button::Interact);
    for _ in 0..20 {
        if g.world.state == WorldState::Playing {
            break;
        }
        press(&mut g, Button::Interact);
    }
    assert_eq!(g.world.state, WorldState::Playing);
    assert!(g.world.progress.keys.contains(&Key::EastWard));
    assert_eq!(g.world.progress.flasks, 5);
    // Drinking heals.
    g.world.actors[0].stats.hp = 100.0;
    press(&mut g, Button::UseItem);
    tick(&mut g, 2.0, &InputState::default());
    assert!(g.world.actors[0].stats.hp > 250.0, "healed to {}", g.world.actors[0].stats.hp);
    assert_eq!(g.world.progress.flasks, 4);
}

#[test]
fn boulder_breaks_the_wall() {
    let mut g = new_game();
    g.command(UiCommand::Debug("god".into()));
    warp(&mut g, 14.0, 9.0, 22.5, 0.0);
    walk(&mut g, Vec2::new(0.0, 0.3), 0.3);
    tick(&mut g, 4.0, &InputState::default());
    assert!(g.world.progress.has("wall_broken"), "boulder state {:?}", g.world.boulder.state);
}

#[test]
fn escape_gate_slams_and_boss_waits() {
    let mut g = new_game();
    g.command(UiCommand::Debug("god".into()));
    warp(&mut g, 2.0, 3.0, 18.5, 0.0);
    tick(&mut g, 1.0, &InputState::default());
    assert!(matches!(g.world.encounter, Encounter::Intro { .. }), "{:?}", g.world.encounter);
    tick(&mut g, 4.0, &InputState::default());
    assert_eq!(g.world.encounter, Encounter::Active);
    // Run through the left gate and down.
    warp(&mut g, 11.0, 3.0, 20.0, 90.0);
    walk(&mut g, Vec2::Y, 1.5);
    tick(&mut g, 1.0, &InputState::default());
    assert!(g.world.progress.has("asylum_escaped"));
    assert_eq!(g.world.encounter, Encounter::None);
    let gate = g.world.named.escape_gate;
    let PropKind::Gate(gt) = &g.world.props[gate].kind else { panic!() };
    assert_eq!(gt.lift, 0.0, "gate closed");
}

#[test]
fn boss_death_unlocks_great_door_and_ending() {
    let mut g = new_game();
    g.command(UiCommand::Debug("god".into()));
    warp(&mut g, 0.0, 3.0, 18.5, 0.0);
    tick(&mut g, 5.0, &InputState::default());
    let d = g.world.demon.unwrap() as usize;
    g.world.actors[d].stats.hp = 1.0;
    g.world.actors[d].stats.absorb = 0.0;
    // Lock on and swing until it dies.
    press(&mut g, Button::LockOn);
    for _ in 0..40 {
        let pos = g.world.actors[d].pos;
        let p = g.world.player().pos;
        if g.world.actors[d].dead {
            break;
        }
        // Stand next to it and attack.
        let to = (pos - p).normalize();
        warp(&mut g, pos.x - to.x * 2.8, 3.0, pos.z - to.z * 2.8, to.x.atan2(to.z).to_degrees());
        press(&mut g, Button::Light);
        tick(&mut g, 0.6, &InputState::default());
    }
    assert!(g.world.actors[d].dead, "demon died");
    tick(&mut g, 6.0, &InputState::default());
    assert!(g.world.progress.keys.contains(&Key::Great));
    assert!(g.world.progress.cinders >= 2000);
    // Open the great door.
    warp(&mut g, 0.0, 3.0, 48.6, 0.0);
    assert_eq!(g.world.prompt.as_deref(), Some("Open"));
    press(&mut g, Button::Interact);
    tick(&mut g, 4.0, &InputState::default());
    walk(&mut g, Vec2::Y, 3.0);
    assert!(g.world.player().pos.z > 52.0, "through the door: {:?}", g.world.player().pos);
    // Climb to the nest.
    warp(&mut g, 0.0, 12.0, 88.0, 0.0);
    walk(&mut g, Vec2::Y, 2.0);
    assert!(matches!(g.world.state, WorldState::Ending { .. }), "{:?}", g.world.state);
}

#[test]
fn death_respawns_at_pyre_with_bloodstain() {
    let mut g = new_game();
    // Kindle the courtyard pyre.
    warp(&mut g, 0.0, 3.0, 2.6, 0.0);
    assert_eq!(g.world.prompt.as_deref(), Some("Kindle the pyre"));
    press(&mut g, Button::Interact);
    tick(&mut g, 2.0, &InputState::default());
    assert_eq!(g.world.state, WorldState::Resting);
    press(&mut g, Button::Back);
    tick(&mut g, 1.5, &InputState::default());
    g.world.progress.cinders = 500;
    warp(&mut g, 0.0, 3.0, 10.0, 0.0);
    let p = &mut g.world.actors[0];
    p.stats.hp = 1.0;
    // Fall to death by being struck: simply kill.
    g.world.kill_actor(0);
    tick(&mut g, 6.0, &InputState::default());
    assert_eq!(g.world.state, WorldState::Playing);
    let pos = g.world.player().pos;
    assert!((pos - Vec3::new(0.0, 3.0, 2.6)).length() < 1.0, "respawned at pyre: {pos:?}");
    assert_eq!(g.world.progress.cinders, 0);
    assert!(g.world.progress.bloodstain.is_some());
    assert!(matches!(g.world.player().action, Action::Move));
}

/// One-frame taps on a 144 Hz display must all register.
#[test]
fn taps_register_at_high_refresh_rates() {
    let mut g = new_game();
    let dt = 1.0 / 144.0;
    let idle = InputState::default();
    let tap = |g: &mut Game, b: Button| {
        let mut i = InputState::default();
        i.set(b, true);
        g.frame(dt, &i);
        for _ in 0..40 {
            g.frame(dt, &idle);
        }
    };
    // Toggle the rune message in the corridor ten times.
    warp(&mut g, 0.0, 0.0, -38.6, 0.0);
    let mut toggles = 0;
    for _ in 0..10 {
        let before = matches!(g.world.state, WorldState::Reading(_));
        tap(&mut g, Button::Interact);
        let after = matches!(g.world.state, WorldState::Reading(_));
        toggles += (before != after) as u32;
    }
    assert_eq!(toggles, 10, "every E tap toggles the message");
    // Every Space tap rolls (backsteps, standing still).
    for _ in 0..8 {
        tap(&mut g, Button::Dodge);
        assert!(matches!(g.world.player().action, Action::Backstep), "{:?}", g.world.player().action);
        for _ in 0..120 {
            g.frame(dt, &idle);
        }
    }
    // Every click swings.
    for _ in 0..8 {
        let mut i = InputState::default();
        i.set(Button::Light, true);
        g.frame(dt, &i);
        for _ in 0..5 {
            g.frame(dt, &idle);
        }
        assert!(matches!(g.world.player().action, Action::Attack { .. }), "{:?}", g.world.player().action);
        for _ in 0..200 {
            g.frame(dt, &idle);
        }
    }
}

#[test]
fn mist_veil_can_be_traversed_both_ways() {
    let mut g = new_game();
    // Stairwell landing, facing the veil into the upper corridor (-X).
    warp(&mut g, 14.6, 7.0, -20.0, -90.0);
    assert_eq!(g.world.prompt.as_deref(), Some("Traverse the mist"));
    press(&mut g, Button::Interact);
    tick(&mut g, 2.0, &InputState::default());
    let p = g.world.player().pos;
    assert!(p.x < 13.0, "walked through the veil: {p:?}");
    // And back again from the other side.
    g.world.actors[0].yaw = std::f32::consts::FRAC_PI_2;
    tick(&mut g, 0.2, &InputState::default());
    assert_eq!(g.world.prompt.as_deref(), Some("Traverse the mist"));
    press(&mut g, Button::Interact);
    tick(&mut g, 2.0, &InputState::default());
    let p = g.world.player().pos;
    assert!(p.x > 14.0, "walked back: {p:?}");
    // The veil still blocks plain walking afterwards.
    g.world.actors[0].yaw = -std::f32::consts::FRAC_PI_2;
    walk(&mut g, Vec2::Y, 1.5);
    assert!(g.world.player().pos.x > 13.6, "veil blocks walking: {:?}", g.world.player().pos);
}

#[test]
fn boss_veil_starts_second_encounter() {
    let mut g = new_game();
    g.world.progress.set("asylum_escaped");
    g.command(UiCommand::Debug("reset".into()));
    warp(&mut g, 13.8, 11.0, 48.0, -90.0);
    assert_eq!(g.world.prompt.as_deref(), Some("Traverse the mist"));
    press(&mut g, Button::Interact);
    tick(&mut g, 2.0, &InputState::default());
    assert!(g.world.player().pos.x < 12.0, "on the ledge: {:?}", g.world.player().pos);
    assert_eq!(g.world.encounter, Encounter::Active);
}

#[test]
fn interrupted_veil_walk_closes_the_veil() {
    let mut g = new_game();
    warp(&mut g, 14.6, 7.0, -20.0, -90.0);
    press(&mut g, Button::Interact);
    tick(&mut g, 0.2, &InputState::default());
    g.world.actors[0].set_action(Action::Move);
    tick(&mut g, 0.1, &InputState::default());
    assert!(g.world.fog_pass.is_none());
    walk(&mut g, Vec2::Y, 2.0);
    assert!(g.world.player().pos.x > 13.6, "veil blocks again: {:?}", g.world.player().pos);
}
