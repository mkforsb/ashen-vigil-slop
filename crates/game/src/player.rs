//! Player controller: turns input into an [`Intent`].

use crate::actor::{Action, Actor, Intent, Request};
use crate::input::{Button, InputTracker};
use ashen_engine::math::*;

/// Holding dodge longer than this (while moving) sprints instead of rolling.
pub const SPRINT_HOLD: f32 = 0.28;

#[derive(Clone, Debug, Default)]
pub struct PlayerCtl {
    dodge_t: f32,
    dodge_held: bool,
}

/// Camera-relative movement direction from the stick.
pub fn camera_move(axis: Vec2, cam_yaw: f32) -> Vec3 {
    let fwd = yaw_dir(cam_yaw);
    let right = Vec3::new(-cam_yaw.cos(), 0.0, cam_yaw.sin());
    (fwd * axis.y + right * axis.x).clamp_length_max(1.0)
}

impl PlayerCtl {
    pub fn intent(&mut self, inp: &InputTracker, me: &Actor, cam_yaw: f32, lock_pos: Option<Vec3>, dt: f32) -> Intent {
        let mv = camera_move(inp.state.move_axis, cam_yaw);
        let mut it = Intent { mv, face: lock_pos, block: inp.held(Button::Block), walk: inp.held(Button::Walk), speed_scale: 1.0, ..Default::default() };

        if inp.pressed(Button::Dodge) {
            self.dodge_t = 0.0;
            self.dodge_held = true;
        }
        if self.dodge_held && inp.held(Button::Dodge) {
            self.dodge_t += dt;
        }
        let moving = mv.length() > 0.3;
        if self.dodge_held && inp.held(Button::Dodge) && self.dodge_t > SPRINT_HOLD && moving {
            it.sprint = true;
        }
        if inp.released(Button::Dodge) && self.dodge_held {
            if self.dodge_t <= SPRINT_HOLD {
                it.request = Some(Request::Roll(if moving { mv } else { Vec3::ZERO }));
            }
            self.dodge_held = false;
        }
        if !inp.held(Button::Dodge) {
            self.dodge_held = false;
        }
        if inp.pressed(Button::Light) {
            it.request = Some(Request::Light);
        }
        if inp.pressed(Button::Heavy) {
            it.request = Some(Request::Heavy);
        }
        if inp.pressed(Button::Jump) {
            it.request = Some(if me.sprinting { Request::Jump } else { Request::Roll(if moving { mv } else { Vec3::ZERO }) });
        }
        if inp.pressed(Button::UseItem) {
            it.request = Some(Request::Drink);
        }
        // Sitting: any input stands up (handled by the world).
        if matches!(me.action, Action::Sitting) {
            it = Intent { speed_scale: 1.0, ..Default::default() };
        }
        it
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_relative() {
        // Camera looking along +Z: pushing right moves towards -X.
        let d = camera_move(Vec2::new(1.0, 0.0), 0.0);
        assert!((d - Vec3::NEG_X).length() < 1e-5);
        let d = camera_move(Vec2::new(0.0, 1.0), std::f32::consts::FRAC_PI_2);
        assert!((d - Vec3::X).length() < 1e-5);
    }
}
