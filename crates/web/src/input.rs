//! Browser input: keyboard, mouse (pointer lock) and gamepads, mapped onto
//! the game's abstract buttons.

use ashen_game::input::{Button, InputState};
use glam::Vec2;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

#[derive(Default)]
pub struct Raw {
    keys: HashSet<String>,
    mouse: [bool; 5],
    /// Keys / buttons pressed since the last poll, so a tap shorter than a
    /// frame still registers as held for one frame.
    tapped: HashSet<String>,
    mouse_tapped: [bool; 5],
    dx: f32,
    dy: f32,
    pub locked: bool,
    /// Pointer lock was lost by the player (Escape): the game should pause.
    pub lock_lost: bool,
    /// The game itself released the pointer (menus); not a reason to pause.
    expect_unlock: bool,
    /// Any user gesture happened (needed to start audio).
    pub gesture: bool,
    last_gamepad: bool,
}

#[derive(Clone)]
pub struct WebInput {
    pub raw: Rc<RefCell<Raw>>,
}

const GAME_KEYS: &[&str] = &["Space", "Tab", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "KeyQ", "KeyE", "KeyR", "KeyF", "KeyC"];

fn listen<E: JsCast + wasm_bindgen::convert::FromWasmAbi + 'static>(target: &web_sys::EventTarget, name: &str, f: impl FnMut(E) + 'static) {
    let cb = Closure::<dyn FnMut(E)>::new(f);
    target.add_event_listener_with_callback(name, cb.as_ref().unchecked_ref()).ok();
    cb.forget();
}

impl WebInput {
    pub fn install() -> Self {
        let raw = Rc::new(RefCell::new(Raw::default()));
        let window = web_sys::window().unwrap();
        let doc = window.document().unwrap();
        let wt: &web_sys::EventTarget = window.as_ref();
        {
            let r = raw.clone();
            listen(wt, "keydown", move |e: web_sys::KeyboardEvent| {
                let code = e.code();
                let mut r = r.borrow_mut();
                r.gesture = true;
                r.last_gamepad = false;
                if r.locked && GAME_KEYS.contains(&code.as_str()) {
                    e.prevent_default();
                }
                if !e.repeat() {
                    r.tapped.insert(code.clone());
                }
                r.keys.insert(code);
            });
        }
        {
            let r = raw.clone();
            listen(wt, "keyup", move |e: web_sys::KeyboardEvent| {
                r.borrow_mut().keys.remove(&e.code());
            });
        }
        {
            let r = raw.clone();
            listen(wt, "mousedown", move |e: web_sys::MouseEvent| {
                let mut r = r.borrow_mut();
                r.gesture = true;
                r.last_gamepad = false;
                let b = e.button() as usize;
                if b < 5 && r.locked {
                    r.mouse[b] = true;
                    r.mouse_tapped[b] = true;
                }
            });
        }
        {
            let r = raw.clone();
            listen(wt, "mouseup", move |e: web_sys::MouseEvent| {
                let b = e.button() as usize;
                if b < 5 {
                    r.borrow_mut().mouse[b] = false;
                }
            });
        }
        {
            let r = raw.clone();
            listen(wt, "mousemove", move |e: web_sys::MouseEvent| {
                let mut r = r.borrow_mut();
                // Headless test harnesses dispatch synthetic events without lock.
                r.dx += e.movement_x() as f32;
                r.dy += e.movement_y() as f32;
            });
        }
        listen(wt, "contextmenu", move |e: web_sys::MouseEvent| e.prevent_default());
        {
            let r = raw.clone();
            listen(wt, "blur", move |_: web_sys::FocusEvent| {
                let mut r = r.borrow_mut();
                r.keys.clear();
                r.mouse = [false; 5];
            });
        }
        {
            let r = raw.clone();
            let d = doc.clone();
            listen(doc.as_ref(), "pointerlockchange", move |_: web_sys::Event| {
                let mut r = r.borrow_mut();
                let locked = d.pointer_lock_element().is_some();
                if r.locked && !locked {
                    if r.expect_unlock {
                        r.expect_unlock = false;
                    } else {
                        r.lock_lost = true;
                    }
                    r.mouse = [false; 5];
                }
                r.locked = locked;
            });
        }
        WebInput { raw }
    }

    pub fn request_lock(&self) {
        if let Some(c) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id("game")) {
            c.request_pointer_lock();
        }
    }

    /// Releases the pointer on the game's behalf (menus), without pausing.
    pub fn release_lock(&self) {
        if let Some(d) = web_sys::window().and_then(|w| w.document()) {
            if d.pointer_lock_element().is_some() {
                self.raw.borrow_mut().expect_unlock = true;
                d.exit_pointer_lock();
            }
        }
    }

    /// Builds this frame's input state.
    pub fn poll(&self, dt: f32) -> InputState {
        let mut r = self.raw.borrow_mut();
        let mut s = InputState::default();
        let k = |c: &str| r.keys.contains(c) || r.tapped.contains(c);
        let mouse: [bool; 5] = std::array::from_fn(|i| r.mouse[i] || r.mouse_tapped[i]);
        let mut mv = Vec2::ZERO;
        if k("KeyW") || k("ArrowUp") {
            mv.y += 1.0;
        }
        if k("KeyS") || k("ArrowDown") {
            mv.y -= 1.0;
        }
        if k("KeyD") || k("ArrowRight") {
            mv.x += 1.0;
        }
        if k("KeyA") || k("ArrowLeft") {
            mv.x -= 1.0;
        }
        s.move_axis = mv.normalize_or_zero();
        let shift = k("ShiftLeft") || k("ShiftRight");
        let lmb = mouse[0];
        s.set(Button::Light, lmb && !shift);
        s.set(Button::Heavy, (lmb && shift) || k("KeyF"));
        s.set(Button::Block, mouse[2]);
        s.set(Button::LockOn, mouse[1] || k("KeyQ") || k("Tab"));
        s.set(Button::Dodge, k("Space"));
        s.set(Button::Interact, k("KeyE"));
        s.set(Button::UseItem, k("KeyR"));
        s.set(Button::Jump, k("KeyC"));
        s.set(Button::Walk, k("ControlLeft") || k("AltLeft"));
        s.set(Button::Pause, k("Escape") || k("KeyP"));
        s.set(Button::Confirm, k("Enter"));
        s.set(Button::Back, k("Backspace"));
        s.look = Vec2::new(-r.dx, r.dy) * 0.0022;
        r.dx = 0.0;
        r.dy = 0.0;
        r.tapped.clear();
        r.mouse_tapped = [false; 5];

        // Gamepads (standard mapping).
        let mut pad_used = false;
        if let Some(nav) = web_sys::window().map(|w| w.navigator()) {
            if let Ok(pads) = nav.get_gamepads() {
                for p in pads.iter() {
                    let Ok(pad) = p.dyn_into::<web_sys::Gamepad>() else { continue };
                    let axes: Vec<f32> = pad.axes().iter().map(|a| a.as_f64().unwrap_or(0.0) as f32).collect();
                    let btn = |i: u32| pad.buttons().get(i).dyn_into::<web_sys::GamepadButton>().map(|b| b.pressed()).unwrap_or(false);
                    let dz = |v: Vec2, d: f32| if v.length() < d { Vec2::ZERO } else { v * ((v.length() - d) / (1.0 - d)).min(1.0) / v.length() };
                    if axes.len() >= 4 {
                        let l = dz(Vec2::new(axes[0], -axes[1]), 0.2);
                        let rs = dz(Vec2::new(axes[2], axes[3]), 0.15);
                        if l.length() > 0.0 {
                            s.move_axis = l;
                            pad_used = true;
                        }
                        if rs.length() > 0.0 {
                            s.look += Vec2::new(-rs.x * 2.8, rs.y * 1.8) * dt;
                            pad_used = true;
                        }
                    }
                    let map = [(0, Button::Interact), (1, Button::Dodge), (2, Button::UseItem), (4, Button::Block), (5, Button::Light), (7, Button::Heavy), (9, Button::Pause), (10, Button::Jump), (11, Button::LockOn), (0, Button::Confirm), (1, Button::Back)];
                    for (i, b) in map {
                        if btn(i) {
                            s.held[b as usize] = true;
                            pad_used = true;
                        }
                    }
                }
            }
        }
        if pad_used {
            r.last_gamepad = true;
            r.gesture = true;
        }
        s.gamepad = r.last_gamepad;
        s
    }
}
