//! Ashen Vigil: game simulation, content and presentation logic.
//!
//! This crate has no platform dependencies; the web shell feeds it input,
//! uploads the generated assets and draws the render lists it produces.

pub mod actor;
pub mod ai;
pub mod anims;
pub mod assets;
pub mod asylum;
pub mod camera;
pub mod combat;
pub mod fx;
pub mod game;
pub mod hud;
pub mod input;
pub mod level;
pub mod player;
pub mod props;
pub mod rig;
pub mod save;
pub mod script;
pub mod view;
pub mod world;

pub use game::{Game, Mode, Settings, UiCommand};
