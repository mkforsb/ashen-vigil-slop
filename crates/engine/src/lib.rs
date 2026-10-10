//! Platform-independent engine pieces for a PS1-style 3D action game.
//!
//! Everything here is content-agnostic: geometry building, procedural texture
//! helpers, rigid skeletal animation, collision, DSP for procedural audio and a
//! WebGL2 renderer (via `glow`) that emulates PSX rendering quirks.
//!
//! Conventions used throughout:
//! - Right-handed, Y up. A character's local forward is +Z, which makes its
//!   right-hand side -X and its left-hand side +X.
//! - A yaw angle `a` corresponds to the forward vector `(sin a, 0, cos a)`,
//!   which matches `Quat::from_rotation_y(a)` applied to +Z.
//! - Distances are metres, time is seconds.

pub mod anim;
pub mod collision;
pub mod math;
pub mod mesh;
pub mod render;
pub mod rng;
pub mod skeleton;
pub mod synth;
pub mod texture;

pub use glam;
