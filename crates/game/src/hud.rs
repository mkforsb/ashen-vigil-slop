//! HUD/UI model shared with the web shell. The shell renders it with Dioxus.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerKind {
    /// "YOU PERISHED"
    Death,
    /// "FOE VANQUISHED"
    Victory,
    /// "PYRE KINDLED"
    Kindled,
    /// Area name.
    Area,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Banner {
    pub text: String,
    pub kind: BannerKind,
    /// 0..1 opacity.
    pub alpha: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BossBar {
    pub name: String,
    pub hp: f32,
    /// Delayed "damage taken" trail.
    pub lag: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dialog {
    pub speaker: String,
    pub line: String,
    pub more: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudState {
    pub visible: bool,
    pub hp: f32,
    pub hp_max: f32,
    pub hp_lag: f32,
    pub stamina: f32,
    pub stamina_max: f32,
    pub flasks: u8,
    pub flasks_max: u8,
    pub cinders: u32,
    pub boss: Option<BossBar>,
    pub prompt: Option<String>,
    pub banner: Option<Banner>,
    /// Recent pickups, newest last.
    pub popups: Vec<String>,
    /// Etched rune message being read.
    pub message: Option<String>,
    pub dialog: Option<Dialog>,
    pub resting: Option<String>,
    pub ending: Option<f32>,
    pub debug: Option<String>,
    pub gamepad: bool,
}

impl HudState {
    /// Quantises continuously changing values so the UI only re-renders when
    /// something visible changes.
    pub fn quantized(mut self) -> Self {
        let q = |v: f32| (v * 2.0).round() / 2.0;
        self.hp = q(self.hp);
        self.hp_lag = q(self.hp_lag);
        self.stamina = q(self.stamina);
        if let Some(b) = self.boss.as_mut() {
            b.hp = (b.hp * 400.0).round() / 400.0;
            b.lag = (b.lag * 400.0).round() / 400.0;
        }
        if let Some(b) = self.banner.as_mut() {
            b.alpha = (b.alpha * 30.0).round() / 30.0;
        }
        self
    }
}
