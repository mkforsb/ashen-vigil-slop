use ashen_engine::math::{Vec2, Vec3};
use ashen_engine::render::RenderList;
use ashen_game::input::{Button, InputState};
use ashen_game::{Game, UiCommand};
use std::time::Instant;

fn run(g: &mut Game, secs: f32, input: &InputState) {
    let mut list = RenderList::default();
    let n = (secs * 60.0) as usize;
    for _ in 0..n {
        g.frame(1.0 / 60.0, input);
        g.render(16.0 / 9.0, &mut list);
        let _ = g.audio();
        let _ = g.hud();
    }
}

#[test]
fn new_game_runs() {
    let t = Instant::now();
    let mut g = Game::new();
    eprintln!("assets+map: {:?}, meshes {}", t.elapsed(), g.assets.meshes.len());
    let t = Instant::now();
    for &s in ashen_game::assets::sounds::Sfx::ALL {
        let _ = ashen_game::assets::sounds::generate(s);
    }
    eprintln!("sounds: {:?}", t.elapsed());
    g.command(UiCommand::NewGame);
    let idle = InputState::default();
    run(&mut g, 3.0, &idle);
    // Walk forward, attack, roll.
    let mut inp = InputState { move_axis: Vec2::new(0.0, 1.0), ..Default::default() };
    run(&mut g, 1.0, &inp);
    inp.set(Button::Light, true);
    run(&mut g, 0.1, &inp);
    inp.set(Button::Light, false);
    run(&mut g, 1.0, &inp);
    inp.set(Button::Dodge, true);
    run(&mut g, 0.1, &inp);
    inp.set(Button::Dodge, false);
    run(&mut g, 1.0, &inp);
    let p = g.world.player().pos;
    eprintln!("player at {p:?}");
    assert!(p.y > -1.0 && p.y < 1.0, "player fell through the floor: {p:?}");
}

#[test]
fn boss_fight_runs() {
    let mut g = Game::new();
    g.command(UiCommand::NewGame);
    // Teleport into the arena to trigger the encounter.
    g.world.actors[0].pos = Vec3::new(0.0, 3.0, 19.0);
    g.world.actors[0].set_action(ashen_game::actor::Action::Move);
    let mut inp = InputState::default();
    run(&mut g, 6.0, &inp);
    eprintln!("encounter {:?}", g.world.encounter);
    let d = g.world.demon.unwrap() as usize;
    eprintln!("demon at {:?} action {:?}", g.world.actors[d].pos, g.world.actors[d].action);
    // Fight: hold block and attack occasionally.
    inp.set(Button::Block, true);
    for i in 0..40 {
        inp.set(Button::Light, i % 3 == 0);
        run(&mut g, 0.25, &inp);
    }
    eprintln!("player hp {} demon hp {} state {:?}", g.world.player().stats.hp, g.world.actors[d].stats.hp, g.world.state);
    run(&mut g, 8.0, &InputState::default());
}
