//! Third-person orbit camera with lock-on and wall collision.

use ashen_engine::collision::CollisionWorld;
use ashen_engine::math::*;
use ashen_engine::rng::Rng;

#[derive(Clone, Debug)]
pub struct CameraRig {
    pub yaw: f32,
    /// Positive pitch puts the camera above the player looking down.
    pub pitch: f32,
    pub dist: f32,
    pub pos: Vec3,
    pub look: Vec3,
    pub fov: f32,
    shake: f32,
    cur_dist: f32,
    rng: Rng,
    pub invert_y: bool,
}

impl Default for CameraRig {
    fn default() -> Self {
        CameraRig {
            yaw: 0.0,
            pitch: 0.25,
            dist: 4.2,
            pos: Vec3::ZERO,
            look: Vec3::ZERO,
            fov: deg(58.0),
            shake: 0.0,
            cur_dist: 4.2,
            rng: Rng::new(1),
            invert_y: false,
        }
    }
}

impl CameraRig {
    pub fn add_shake(&mut self, amount: f32) {
        self.shake = (self.shake + amount).min(1.5);
    }

    /// Snaps the camera behind a facing direction.
    pub fn reset_behind(&mut self, yaw: f32, pivot: Vec3) {
        self.yaw = yaw;
        self.pitch = 0.25;
        self.look = pivot;
        self.pos = pivot - yaw_dir(yaw) * self.dist + Vec3::Y * 1.0;
        self.cur_dist = self.dist;
    }

    pub fn update(&mut self, dt: f32, player_pos: Vec3, lock: Option<Vec3>, look_delta: Vec2, col: &CollisionWorld) {
        let pivot = player_pos + Vec3::Y * 1.55;
        let mut look_at = pivot;
        match lock {
            Some(t) => {
                let to = t - pivot;
                let want_yaw = dir_yaw(flat(to));
                self.yaw = wrap_angle(self.yaw + angle_diff(self.yaw, want_yaw) * (1.0 - (-8.0 * dt).exp()));
                let horiz = flat(to).length().max(0.5);
                let elev = (to.y / horiz).atan();
                // Stay above the player even against towering foes.
                let want_pitch = (0.34 - elev * 0.2).clamp(0.12, 0.7);
                self.pitch = damp(self.pitch, want_pitch, 4.0, dt);
                look_at = pivot + (t - pivot).clamp_length_max(5.0) * 0.3;
            }
            None => {
                self.yaw = wrap_angle(self.yaw + look_delta.x);
                let dy = if self.invert_y { -look_delta.y } else { look_delta.y };
                self.pitch = (self.pitch + dy).clamp(-0.65, 1.15);
            }
        }
        let dir = Vec3::new(-self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), -self.yaw.cos() * self.pitch.cos());
        // Pull in when something blocks the view.
        let mut want = self.dist;
        let probes = [Vec3::ZERO, Vec3::new(0.15, 0.15, 0.0), Vec3::new(-0.15, 0.15, 0.0), Vec3::new(0.0, -0.15, 0.15)];
        for o in probes {
            if let Some(h) = col.raycast(pivot + o, dir, self.dist + 0.3) {
                want = want.min((h.t - 0.35).max(0.6));
            }
        }
        // Pull in quickly, ease back out.
        self.cur_dist = if want < self.cur_dist { want } else { damp(self.cur_dist, want, 3.0, dt) };
        self.look = damp_vec3(self.look, look_at, 14.0, dt);
        self.pos = self.look + (pivot - look_at) + dir * self.cur_dist;
        self.shake = (self.shake - dt * 2.5).max(0.0);
    }

    pub fn view(&mut self) -> Mat4 {
        let s = self.shake * self.shake * 0.25;
        let jitter = Vec3::new(self.rng.signed(), self.rng.signed(), self.rng.signed()) * s;
        look_at(self.pos + jitter, self.look + jitter * 0.5, Vec3::Y)
    }

    pub fn forward(&self) -> Vec3 {
        (self.look - self.pos).normalize_or(Vec3::Z)
    }
}
