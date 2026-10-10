//! Dioxus UI: menus and the in-game HUD.

use crate::{Panel, SAVE_KEY, SETTINGS_KEY, command, load, store, with_runner};
use ashen_game::hud::{BannerKind, HudState};
use ashen_game::{Settings, UiCommand};
use dioxus::prelude::*;

#[component]
pub fn Loading(progress: f32) -> Element {
    let pct = (progress * 100.0).round();
    rsx! {
        div { class: "fullscreen center title-bg",
            div { class: "loading",
                div { class: "loading-text", "Kindling the flame" }
                div { class: "loading-bar", div { class: "loading-fill", style: "width: {pct}%" } }
            }
        }
    }
}

#[component]
pub fn Title(has_save: bool, panel: Signal<Panel>) -> Element {
    let p = panel();
    rsx! {
        div { class: "fullscreen title-screen",
            div { class: "title-block",
                h1 { class: "game-title", "ASHEN VIGIL" }
                div { class: "subtitle", "~ the Hollow Asylum ~" }
            }
            if p == Panel::None {
                div { class: "menu",
                    if has_save {
                        button {
                            class: "menu-item",
                            onclick: move |_| {
                                if let Some(s) = load(SAVE_KEY) {
                                    command(UiCommand::Continue(s));
                                }
                            },
                            "Continue"
                        }
                    }
                    button { class: "menu-item", onclick: move |_| command(UiCommand::NewGame), "New Game" }
                    button { class: "menu-item", onclick: move |_| panel.set(Panel::Settings), "Settings" }
                    button { class: "menu-item", onclick: move |_| panel.set(Panel::Controls), "Controls" }
                }
            } else {
                Panels { panel }
            }
            div { class: "footer", "A PSX-style demake. All art, sound and music are generated procedurally." }
        }
    }
}

#[component]
pub fn PauseMenu(panel: Signal<Panel>) -> Element {
    let p = panel();
    rsx! {
        div { class: "fullscreen pause-screen",
            if p == Panel::None {
                div { class: "menu",
                    div { class: "menu-title", "Paused" }
                    button { class: "menu-item", onclick: move |_| command(UiCommand::Resume), "Resume" }
                    button { class: "menu-item", onclick: move |_| panel.set(Panel::Settings), "Settings" }
                    button { class: "menu-item", onclick: move |_| panel.set(Panel::Controls), "Controls" }
                    button { class: "menu-item", onclick: move |_| command(UiCommand::QuitToTitle), "Save & Quit to Title" }
                }
            } else {
                Panels { panel }
            }
        }
    }
}

#[component]
fn Panels(panel: Signal<Panel>) -> Element {
    match panel() {
        Panel::Settings => rsx! { SettingsPanel { panel } },
        Panel::Controls => rsx! { ControlsPanel { panel } },
        Panel::None => rsx! {},
    }
}

fn apply(s: Settings) {
    if let Ok(j) = serde_json::to_string(&s) {
        store(SETTINGS_KEY, &j);
    }
    command(UiCommand::Settings(s));
}

#[component]
fn SettingsPanel(panel: Signal<Panel>) -> Element {
    let mut s = use_signal(|| with_runner(|r| r.game.settings.clone()).unwrap_or_default());
    let cur = s();
    let mut set = move |f: &dyn Fn(&mut Settings)| {
        let mut n = s();
        f(&mut n);
        s.set(n.clone());
        apply(n);
    };
    let toggle = |label: &'static str, value: bool, f: fn(&mut Settings, bool)| {
        rsx! {
            label { class: "setting",
                span { "{label}" }
                input {
                    r#type: "checkbox",
                    checked: value,
                    onchange: move |e| set(&|st: &mut Settings| f(st, e.checked())),
                }
            }
        }
    };
    let slider = |label: &'static str, value: f32, min: f32, max: f32, f: fn(&mut Settings, f32)| {
        rsx! {
            label { class: "setting",
                span { "{label}" }
                input {
                    r#type: "range",
                    min: "{min}",
                    max: "{max}",
                    step: "0.05",
                    value: "{value}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<f32>() {
                            set(&|st: &mut Settings| f(st, v));
                        }
                    },
                }
            }
        }
    };
    rsx! {
        div { class: "panel",
            div { class: "menu-title", "Settings" }
            label { class: "setting",
                span { "Resolution" }
                select {
                    value: "{cur.vertical_res}",
                    onchange: move |e| {
                        if let Ok(v) = e.value().parse::<u32>() {
                            set(&|st: &mut Settings| st.vertical_res = v);
                        }
                    },
                    option { value: "224", "224p (PAL-ish)" }
                    option { value: "240", "240p (authentic)" }
                    option { value: "360", "360p" }
                    option { value: "480", "480p (interlaced hi-res)" }
                    option { value: "720", "720p (modern)" }
                }
            }
            {toggle("Vertex wobble", cur.vertex_snap, |s, v| s.vertex_snap = v)}
            {toggle("Affine texture warping", cur.affine, |s, v| s.affine = v)}
            {toggle("15-bit colour dithering", cur.dither, |s, v| s.dither = v)}
            {toggle("4:3 aspect", cur.force_4_3, |s, v| s.force_4_3 = v)}
            {slider("Mouse sensitivity", cur.mouse_sens, 0.2, 3.0, |s, v| s.mouse_sens = v)}
            {slider("Stick sensitivity", cur.stick_sens, 0.2, 3.0, |s, v| s.stick_sens = v)}
            {toggle("Invert camera Y", cur.invert_y, |s, v| s.invert_y = v)}
            {slider("Master volume", cur.master, 0.0, 1.0, |s, v| s.master = v)}
            {slider("Music volume", cur.music, 0.0, 1.0, |s, v| s.music = v)}
            {slider("Effects volume", cur.sfx, 0.0, 1.0, |s, v| s.sfx = v)}
            {toggle("Show FPS", cur.show_fps, |s, v| s.show_fps = v)}
            button { class: "menu-item", onclick: move |_| panel.set(Panel::None), "Back" }
        }
    }
}

#[component]
fn ControlsPanel(panel: Signal<Panel>) -> Element {
    let rows: &[(&str, &str, &str)] = &[
        ("Move", "W A S D", "Left stick"),
        ("Camera", "Mouse", "Right stick"),
        ("Light attack", "Left click", "RB"),
        ("Heavy attack", "Shift + Left click / F", "RT"),
        ("Block (hold)", "Right click", "LB"),
        ("Roll / backstep (tap)", "Space", "B"),
        ("Sprint (hold)", "Space while moving", "B while moving"),
        ("Jump (while sprinting)", "C", "L3"),
        ("Lock on", "Q / Tab / Middle click", "R3"),
        ("Drink Ember Flask", "R", "X"),
        ("Interact / talk", "E", "A"),
        ("Walk", "Ctrl", "Tilt stick"),
        ("Pause", "Esc / P", "Start"),
    ];
    rsx! {
        div { class: "panel",
            div { class: "menu-title", "Controls" }
            table { class: "controls",
                tr {
                    th { "" }
                    th { "Keyboard & mouse" }
                    th { "Gamepad" }
                }
                for (a, k, g) in rows.iter() {
                    tr {
                        td { "{a}" }
                        td { class: "key", "{k}" }
                        td { class: "key", "{g}" }
                    }
                }
            }
            p { class: "hint", "Attack again during a swing to chain. Attacks right after a roll, backstep or while sprinting have their own moves. Attack while falling to plunge." }
            button { class: "menu-item", onclick: move |_| panel.set(Panel::None), "Back" }
        }
    }
}

fn key(gamepad: bool, kb: &str, pad: &str) -> String {
    if gamepad { format!("({pad})") } else { format!("[{kb}]") }
}

#[component]
pub fn HudView(hud: HudState) -> Element {
    let h = hud;
    let gp = h.gamepad;
    let hp_w = h.hp_max * 0.5;
    let st_w = h.stamina_max * 2.4;
    let hp_pct = if h.hp_max > 0.0 { h.hp / h.hp_max * 100.0 } else { 0.0 };
    let lag_pct = if h.hp_max > 0.0 { h.hp_lag / h.hp_max * 100.0 } else { 0.0 };
    let st_pct = if h.stamina_max > 0.0 { h.stamina / h.stamina_max * 100.0 } else { 0.0 };
    let interact = key(gp, "E", "A");
    rsx! {
        div { class: "hud",
            if h.visible {
                div { class: "bars",
                    div { class: "bar hp", style: "width: {hp_w}px",
                        div { class: "lag", style: "width: {lag_pct}%" }
                        div { class: "fill", style: "width: {hp_pct}%" }
                    }
                    div { class: "bar st", style: "width: {st_w}px",
                        div { class: "fill", style: "width: {st_pct}%" }
                    }
                }
                div { class: "equip",
                    div { class: "slot left", "🛡" }
                    div { class: "slot right", "⚔" }
                    div { class: "slot item",
                        span { class: if h.flasks > 0 { "flask lit" } else { "flask" }, "⚱" }
                        span { class: "count", "{h.flasks}" }
                    }
                }
                div { class: "cinders",
                    span { class: "cinders-num", "{h.cinders}" }
                }
            }
            if let Some(b) = &h.boss {
                div { class: "boss",
                    div { class: "boss-name", "{b.name}" }
                    div { class: "boss-bar",
                        div { class: "lag", style: "width: {b.lag * 100.0}%" }
                        div { class: "fill", style: "width: {b.hp * 100.0}%" }
                    }
                }
            }
            if let Some(p) = &h.prompt {
                if h.message.is_none() && h.dialog.is_none() {
                    div { class: "prompt", "{interact} {p}" }
                }
            }
            if let Some(b) = &h.banner {
                div {
                    class: match b.kind {
                        BannerKind::Death => "banner death",
                        BannerKind::Victory => "banner victory",
                        BannerKind::Kindled => "banner kindled",
                        BannerKind::Area => "banner area",
                    },
                    style: "opacity: {b.alpha}",
                    div { class: "banner-strip" }
                    div { class: "banner-text", "{b.text}" }
                }
            }
            div { class: "popups",
                for (i, p) in h.popups.iter().enumerate() {
                    div { key: "{i}{p}", class: "popup", "{p}" }
                }
            }
            if let Some(m) = &h.message {
                div { class: "message-box",
                    for line in m.lines() {
                        p { "{line}" }
                    }
                    div { class: "hint", "{interact} close" }
                }
            }
            if let Some(d) = &h.dialog {
                div { class: "dialog-box",
                    div { class: "speaker", "{d.speaker}" }
                    p { "{d.line}" }
                    div { class: "hint",
                        if d.more { "{interact} continue" } else { "{interact} farewell" }
                    }
                }
            }
            if let Some(name) = &h.resting {
                div { class: "pyre-menu",
                    div { class: "pyre-name", "{name}" }
                    div { class: "pyre-note", "Your wounds close. Your flasks are rekindled. The dead stir again." }
                    button { class: "menu-item", onclick: move |_| command(UiCommand::LeavePyre), "Leave" }
                    div { class: "hint", "{interact} / move to leave" }
                }
            }
            if let Some(t) = h.ending {
                div { class: "ending", style: "opacity: {((t - 2.5) / 2.0).clamp(0.0, 1.0)}",
                    p { class: "ending-line", "A great raven descends upon the cliff, seizes you in its talons, and bears you away into the grey." }
                    h2 { "THE HOLLOW ASYLUM" }
                    div { class: "ending-sub", "~ complete ~" }
                    p { class: "ending-line", "To be continued..." }
                    if t > 5.0 {
                        button { class: "menu-item", onclick: move |_| command(UiCommand::QuitToTitle), "Return to Title" }
                    }
                }
            }
            if let Some(d) = &h.debug {
                div { class: "debug", "{d}" }
            }
        }
    }
}
