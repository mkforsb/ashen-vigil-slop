//! Ashen Vigil web shell: Dioxus UI + WebGL2 canvas + WebAudio.

mod audio;
mod input;
mod ui;

use ashen_engine::render::{RenderList, Renderer};
use ashen_game::assets::sounds::{self, Sfx};
use ashen_game::hud::HudState;
use ashen_game::{Game, Mode, Settings, UiCommand};
use audio::WebAudio;
use dioxus::prelude::*;
use input::WebInput;
use std::cell::RefCell;
use wasm_bindgen::JsCast;

const STYLE: &str = include_str!("../assets/main.css");

pub const SAVE_KEY: &str = "ashen_vigil_save";
pub const SETTINGS_KEY: &str = "ashen_vigil_settings";

fn main() {
    dioxus::launch(App);
}

pub struct Runner {
    pub game: Game,
    renderer: Renderer,
    pub input: WebInput,
    audio: Option<WebAudio>,
    list: RenderList,
    canvas: web_sys::HtmlCanvasElement,
    sounds_loaded: usize,
    pub test_mode: bool,
}

thread_local! {
    static RUNNER: RefCell<Option<Runner>> = const { RefCell::new(None) };
}

/// Runs `f` with the game runner if it exists.
pub fn with_runner<R>(f: impl FnOnce(&mut Runner) -> R) -> Option<R> {
    RUNNER.with(|r| r.borrow_mut().as_mut().map(f))
}

/// Sends a UI command to the game.
pub fn command(c: UiCommand) {
    with_runner(|r| {
        let starting = matches!(c, UiCommand::NewGame | UiCommand::Continue(_) | UiCommand::Resume | UiCommand::LeavePyre);
        let leaving = matches!(c, UiCommand::Pause | UiCommand::QuitToTitle);
        r.game.command(c);
        if let Some(a) = &r.audio {
            a.resume();
        }
        if starting {
            r.input.request_lock();
        }
        if leaving {
            r.input.release_lock();
        }
    });
}

pub fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

pub fn load(key: &str) -> Option<String> {
    storage()?.get_item(key).ok()?
}

pub fn store(key: &str, value: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(key, value);
    }
}

pub fn url_param(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    search.trim_start_matches('?').split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        (k == name).then(|| v.to_string())
    })
}

fn now_ms() -> f64 {
    web_sys::window().and_then(|w| w.performance()).map(|p| p.now()).unwrap_or(0.0)
}

async fn next_frame() -> f64 {
    let p = js_sys::Promise::new(&mut |resolve, _| {
        web_sys::window().unwrap().request_animation_frame(&resolve).unwrap();
    });
    wasm_bindgen_futures::JsFuture::from(p).await.ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn init(test_mode: bool) -> Result<Runner, String> {
    let doc = web_sys::window().unwrap().document().unwrap();
    let canvas: web_sys::HtmlCanvasElement = doc.get_element_by_id("game").ok_or("no canvas")?.dyn_into().map_err(|_| "not a canvas")?;
    let opts = web_sys::WebGlContextAttributes::new();
    opts.set_antialias(false);
    opts.set_alpha(false);
    opts.set_depth(false);
    let ctx = canvas
        .get_context_with_context_options("webgl2", &opts)
        .map_err(|_| "webgl2 unavailable")?
        .ok_or("WebGL2 is not supported by this browser")?
        .dyn_into::<web_sys::WebGl2RenderingContext>()
        .map_err(|_| "bad context")?;
    let mut renderer = Renderer::new(glow::Context::from_webgl2_context(ctx))?;
    let mut game = Game::new();
    if let Some(s) = load(SETTINGS_KEY).and_then(|s| serde_json::from_str::<Settings>(&s).ok()) {
        game.command(UiCommand::Settings(s));
    }
    renderer.upload_textures(&game.assets.textures);
    for m in &game.assets.meshes {
        renderer.upload_mesh(m);
    }
    let input = WebInput::install();
    if test_mode {
        input.raw.borrow_mut().locked = true;
    }
    let audio = WebAudio::new().ok();
    Ok(Runner { game, renderer, input, audio, list: RenderList::default(), canvas, sounds_loaded: 0, test_mode })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Panel {
    None,
    Settings,
    Controls,
}

#[component]
fn App() -> Element {
    let mut hud = use_signal(HudState::default);
    let mut mode = use_signal(|| Mode::Title);
    let mut loading = use_signal(|| 0.0f32);
    let mut error = use_signal(|| None::<String>);
    let mut locked = use_signal(|| false);
    let mut has_save = use_signal(|| load(SAVE_KEY).is_some());
    let panel = use_signal(|| Panel::None);

    use_future(move || async move {
        // Let the loading screen paint before the heavy generation.
        next_frame().await;
        next_frame().await;
        let test_mode = url_param("test").is_some();
        match init(test_mode) {
            Ok(r) => RUNNER.with(|c| *c.borrow_mut() = Some(r)),
            Err(e) => {
                error.set(Some(e));
                return;
            }
        }
        if let Some(cmd) = url_param("start") {
            match cmd.as_str() {
                "new" => command(UiCommand::NewGame),
                "continue" => {
                    if let Some(s) = load(SAVE_KEY) {
                        command(UiCommand::Continue(s));
                    }
                }
                _ => {}
            }
        }
        // `?debug=warp 0 3 4 0;key Great` runs developer commands after start.
        if let Some(d) = url_param("debug") {
            let d = js_sys::decode_uri_component(&d).ok().and_then(|s| s.as_string()).unwrap_or(d);
            for c in d.split(';') {
                command(UiCommand::Debug(c.replace('+', " ")));
            }
        }
        let mut last = now_ms();
        loop {
            let t = next_frame().await;
            let dt = ((t - last) / 1000.0).clamp(0.0, 0.1) as f32;
            last = t;
            let out = with_runner(|r| {
                // Progressive sound synthesis within a frame budget.
                let total = Sfx::ALL.len();
                if r.sounds_loaded < total {
                    let start = now_ms();
                    while r.sounds_loaded < total && now_ms() - start < 12.0 {
                        let s = Sfx::ALL[r.sounds_loaded];
                        if let Some(a) = r.audio.as_mut() {
                            let _ = a.add(s, &sounds::generate(s));
                        }
                        r.sounds_loaded += 1;
                    }
                }
                let progress = r.sounds_loaded as f32 / total as f32;

                let inp = r.input.poll(dt);
                {
                    let mut raw = r.input.raw.borrow_mut();
                    if raw.gesture {
                        if let Some(a) = &r.audio {
                            a.resume();
                        }
                    }
                    if raw.lock_lost {
                        raw.lock_lost = false;
                        if r.game.mode == Mode::Playing && !r.test_mode {
                            r.game.command(UiCommand::Pause);
                        }
                    }
                }
                r.game.frame(dt, &inp);
                if r.game.mode == Mode::Paused {
                    r.input.release_lock();
                }

                // Canvas resolution follows the window, never the canvas's own
                // layout size (that would feed back and grow without bound).
                let win = web_sys::window().unwrap();
                let dpr = win.device_pixel_ratio().clamp(1.0, 2.0);
                let css_w = win.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(960.0);
                let css_h = win.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(540.0);
                let w = ((css_w * dpr) as u32).min(4096);
                let h = ((css_h * dpr) as u32).min(4096);
                if w > 0 && h > 0 && (r.canvas.width() != w || r.canvas.height() != h) {
                    r.canvas.set_width(w);
                    r.canvas.set_height(h);
                }
                let (cw, ch) = (r.canvas.width().max(1), r.canvas.height().max(1));
                r.renderer.settings = r.game.settings.render();
                let aspect = if r.game.settings.force_4_3 { 4.0 / 3.0 } else { cw as f32 / ch as f32 };
                r.game.render(aspect, &mut r.list);
                r.renderer.render(&r.list, cw, ch);

                let af = r.game.audio();
                if let Some(a) = r.audio.as_mut() {
                    a.update_loops(&af.loops);
                    for s in &af.oneshots {
                        a.play(s);
                    }
                }
                if let Some(save) = r.game.take_save() {
                    store(SAVE_KEY, &save);
                }
                let mut hs = r.game.hud().quantized();
                if hs.ending.is_some_and(|t| t > 4.0) || hs.resting.is_some() {
                    r.input.release_lock();
                }
                if r.game.settings.show_fps {
                    hs.debug = Some(format!("{}  draws {} tris {}", hs.debug.unwrap_or_default(), r.renderer.stats_draws, r.renderer.stats_tris));
                }
                (hs, r.game.mode, progress, r.input.raw.borrow().locked)
            });
            if let Some((hs, m, progress, lk)) = out {
                if *hud.peek() != hs {
                    hud.set(hs);
                }
                if *mode.peek() != m {
                    if m == Mode::Title {
                        has_save.set(load(SAVE_KEY).is_some());
                    }
                    mode.set(m);
                }
                if (*loading.peek() - progress).abs() > 0.01 || (progress >= 1.0 && *loading.peek() < 1.0) {
                    loading.set(progress);
                }
                if *locked.peek() != lk {
                    locked.set(lk);
                }
            }
        }
    });

    let m = mode();
    rsx! {
        // The stylesheet is compiled into the wasm so the bundle works from any
        // URL path (Dioxus resolves `asset!` paths from the site root).
        document::Style { {STYLE} }
        document::Title { "Ashen Vigil" }
        // Critical layout is inline so the game works even before (or without)
        // the stylesheet.
        div { id: "root", style: "position: fixed; left: 0; top: 0; width: 100vw; height: 100vh; overflow: hidden; background: #000;",
            canvas {
                id: "game",
                style: "position: absolute; left: 0; top: 0; width: 100vw; height: 100vh; display: block;",
                onmousedown: move |_| {
                    if m == Mode::Playing {
                        with_runner(|r| r.input.request_lock());
                    }
                },
            }
            if let Some(e) = error() {
                div { class: "fullscreen center error",
                    h1 { "The flame will not kindle" }
                    p { "{e}" }
                }
            } else if loading() < 1.0 && m == Mode::Title {
                ui::Loading { progress: loading() }
            } else {
                match m {
                    Mode::Title => rsx! { ui::Title { has_save: has_save(), panel } },
                    Mode::Paused => rsx! {
                        ui::HudView { hud: hud() }
                        ui::PauseMenu { panel }
                    },
                    Mode::Playing => rsx! {
                        ui::HudView { hud: hud() }
                        if !locked() && hud().resting.is_none() && hud().ending.is_none() {
                            div { class: "click-hint", "Click to take control" }
                        }
                    },
                }
            }
        }
    }
}
