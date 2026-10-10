//! Platform-agnostic input state. The web shell maps keyboard, mouse and
//! gamepads onto these abstract buttons every frame.

use ashen_engine::math::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Button {
    Light,
    Heavy,
    Block,
    Dodge,
    Interact,
    UseItem,
    LockOn,
    Jump,
    Walk,
    Pause,
    /// Menu navigation (confirm / back) for in-game prompts.
    Confirm,
    Back,
}

pub const BUTTONS: usize = 12;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputState {
    /// Movement: x = right, y = forward, length <= 1.
    pub move_axis: Vec2,
    /// Camera rotation this frame in radians (x = yaw right, y = pitch down).
    pub look: Vec2,
    pub held: [bool; BUTTONS],
    pub gamepad: bool,
}

impl InputState {
    pub fn set(&mut self, b: Button, down: bool) {
        self.held[b as usize] = down;
    }
}

/// Tracks edges and hold durations across frames.
///
/// Edges and look movement accumulate until a simulation tick consumes them:
/// on high refresh-rate displays many rendered frames run no tick at all, and
/// their input must carry over rather than be dropped.
#[derive(Clone, Debug, Default)]
pub struct InputTracker {
    prev: [bool; BUTTONS],
    pressed: [bool; BUTTONS],
    released: [bool; BUTTONS],
    held_for: [f32; BUTTONS],
    pub state: InputState,
}

impl InputTracker {
    pub fn begin_frame(&mut self, s: &InputState) {
        for i in 0..BUTTONS {
            self.pressed[i] |= s.held[i] && !self.prev[i];
            self.released[i] |= !s.held[i] && self.prev[i];
            self.prev[i] = s.held[i];
        }
        let look = self.state.look + s.look;
        self.state = s.clone();
        self.state.look = look;
    }

    /// Call after a simulation tick so edges fire only once.
    pub fn consume_edges(&mut self, dt: f32) {
        for i in 0..BUTTONS {
            self.pressed[i] = false;
            self.released[i] = false;
            if self.state.held[i] {
                self.held_for[i] += dt;
            } else {
                self.held_for[i] = 0.0;
            }
        }
        self.state.look = Vec2::ZERO;
    }

    pub fn pressed(&self, b: Button) -> bool {
        self.pressed[b as usize]
    }

    pub fn released(&self, b: Button) -> bool {
        self.released[b as usize]
    }

    pub fn held(&self, b: Button) -> bool {
        self.state.held[b as usize]
    }

    pub fn held_for(&self, b: Button) -> f32 {
        self.held_for[b as usize]
    }

    /// Clears all state (e.g. when a menu opens).
    pub fn reset(&mut self) {
        *self = InputTracker { prev: self.state.held, ..Default::default() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_fire_once() {
        let mut t = InputTracker::default();
        let mut s = InputState::default();
        s.set(Button::Light, true);
        t.begin_frame(&s);
        assert!(t.pressed(Button::Light));
        t.consume_edges(0.016);
        assert!(!t.pressed(Button::Light));
        t.begin_frame(&s);
        assert!(!t.pressed(Button::Light));
        assert!(t.held(Button::Light));
        s.set(Button::Light, false);
        t.begin_frame(&s);
        assert!(t.released(Button::Light));
    }

    #[test]
    fn edges_survive_frames_without_ticks() {
        // A 144 Hz display: a press and release each land on frames where no
        // 60 Hz tick runs; both must still reach the next tick.
        let mut t = InputTracker::default();
        let mut s = InputState::default();
        s.set(Button::Dodge, true);
        t.begin_frame(&s);
        s.set(Button::Dodge, false);
        t.begin_frame(&s);
        assert!(t.pressed(Button::Dodge) && t.released(Button::Dodge));
        t.consume_edges(1.0 / 60.0);
        assert!(!t.pressed(Button::Dodge) && !t.released(Button::Dodge));
    }

    #[test]
    fn look_accumulates_until_consumed() {
        let mut t = InputTracker::default();
        let s = InputState { look: Vec2::new(0.1, 0.0), ..Default::default() };
        t.begin_frame(&s);
        t.begin_frame(&s);
        assert!((t.state.look.x - 0.2).abs() < 1e-6);
        t.consume_edges(1.0 / 60.0);
        assert_eq!(t.state.look, Vec2::ZERO);
    }
}
