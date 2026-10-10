//! Procedural sound effects, ambience and music.
//!
//! Every sound is synthesised from scratch at start-up (no samples), at
//! 22 kHz for a PSX SPU flavour.

use ashen_engine::rng::Rng;
use ashen_engine::synth::*;

macro_rules! sfx_enum {
    ($($name:ident),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Sfx { $($name),* }
        impl Sfx {
            pub const ALL: &'static [Sfx] = &[$(Sfx::$name),*];
        }
    };
}

sfx_enum!(
    StepStone,
    StepArmor,
    StepHusk,
    StepDemon,
    StepWater,
    Roll,
    SwingLight,
    SwingHeavy,
    SwingDemon,
    HitFlesh,
    HitArmor,
    HitDemon,
    Block,
    GuardBreak,
    Hurt,
    Death,
    HuskGroan,
    HuskDeath,
    DemonRoar,
    DemonGrunt,
    DemonHurt,
    DemonDeath,
    DemonSlam,
    DemonWing,
    Drink,
    Heal,
    PyreIgnite,
    PyreLoop,
    DoorOpen,
    DoorLocked,
    GateSlam,
    FogPass,
    ItemPickup,
    UrnBreak,
    ArrowShoot,
    ArrowHit,
    BoulderRoll,
    WallCrash,
    Land,
    Cinders,
    MenuMove,
    MenuSelect,
    YouDied,
    Victory,
    AmbientDungeon,
    AmbientWind,
    BossMusic,
    Crow,
    Bell,
);

/// Mixer bus a sound plays on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bus {
    Sfx,
    Music,
    Ambient,
    Ui,
}

impl Sfx {
    pub fn looping(self) -> bool {
        matches!(self, Sfx::PyreLoop | Sfx::BoulderRoll | Sfx::AmbientDungeon | Sfx::AmbientWind | Sfx::BossMusic)
    }

    pub fn bus(self) -> Bus {
        match self {
            Sfx::BossMusic | Sfx::YouDied | Sfx::Victory => Bus::Music,
            Sfx::AmbientDungeon | Sfx::AmbientWind => Bus::Ambient,
            Sfx::MenuMove | Sfx::MenuSelect => Bus::Ui,
            _ => Bus::Sfx,
        }
    }

    /// Random pitch variation applied per play.
    pub fn pitch_jitter(self) -> f32 {
        match self {
            Sfx::StepStone | Sfx::StepArmor | Sfx::StepHusk | Sfx::StepWater => 0.12,
            Sfx::SwingLight | Sfx::SwingHeavy | Sfx::HitFlesh | Sfx::HitArmor | Sfx::Block | Sfx::UrnBreak => 0.08,
            Sfx::HuskGroan | Sfx::Hurt => 0.1,
            _ => 0.03,
        }
    }
}

fn noise_burst(dur: f32, center: f32, q: f32, attack: f32, decay: f32, seed: u64) -> Buf {
    let mut b = noise(dur, perc(attack, decay), seed);
    b.bandpass(|_| center, q);
    b
}

fn thump(dur: f32, f0: f32, f1: f32, decay: f32) -> Buf {
    osc(Wave::Sine, dur, move |t| f0 + (f1 - f0) * (t / dur), perc(0.002, decay))
}

fn whoosh(dur: f32, lo: f32, hi: f32, end: f32, seed: u64) -> Buf {
    let mut b = noise(dur, move |t| (std::f32::consts::PI * t / dur).sin().max(0.0).powf(1.5), seed);
    b.bandpass(
        move |t| {
            let u = t / dur;
            if u < 0.5 { lo + (hi - lo) * u * 2.0 } else { hi + (end - hi) * (u - 0.5) * 2.0 }
        },
        1.2,
    );
    b
}

/// Formant-filtered buzz, for grunts and roars.
fn voice(dur: f32, f0: impl Fn(f32) -> f32 + Copy, amp: impl Fn(f32) -> f32 + Clone, formants: &[(f32, f32, f32)], breath: f32, seed: u64) -> Buf {
    let mut rng = Rng::new(seed);
    let jitter: Vec<f32> = (0..64).map(|_| rng.signed()).collect();
    let src = osc(
        Wave::Saw,
        dur,
        move |t| {
            let j = jitter[((t * 40.0) as usize) % 64];
            f0(t) * (1.0 + j * 0.04)
        },
        amp.clone(),
    );
    let mut out = Buf::silence(dur);
    for &(f, q, g) in formants {
        let mut c = src.clone();
        c.bandpass(move |_| f, q);
        out.mix(&c, 0.0, g);
    }
    let mut br = noise(dur, amp, seed ^ 0x55);
    br.bandpass(|_| 1800.0, 0.8);
    out.mix(&br, 0.0, breath);
    out
}

fn metal(dur: f32, base: f32, decay: f32, seed: u64) -> Buf {
    let mut r = Rng::new(seed);
    let ratios = [1.0, 1.59, 2.14, 2.65, 3.42, 4.1];
    let partials: Vec<(f32, f32, f32)> = ratios
        .iter()
        .enumerate()
        .map(|(i, k)| (base * k * (1.0 + r.signed() * 0.02), 1.0 / (1.0 + i as f32 * 0.6), decay / (1.0 + i as f32 * 0.4)))
        .collect();
    let mut b = modal(dur, &partials);
    b.mix(&noise_burst(0.03, base * 4.0, 0.7, 0.0005, 0.01, seed ^ 1), 0.0, 0.8);
    b
}

fn crackle(dur: f32, density: f32, seed: u64) -> Buf {
    let mut r = Rng::new(seed);
    let mut b = Buf::silence(dur);
    let n = (dur * density) as usize;
    for _ in 0..n {
        let t = r.range(0.0, dur);
        let len = r.range(0.002, 0.012);
        let mut c = noise(len, perc(0.0005, len * 0.4), r.next_u64());
        c.highpass(|_| 1200.0, 0.7);
        b.mix(&c, t, r.range(0.2, 1.0));
    }
    b
}

fn debris(dur: f32, n: usize, seed: u64) -> Buf {
    let mut r = Rng::new(seed);
    let mut b = Buf::silence(dur);
    for _ in 0..n {
        let t = r.range(0.0, dur * 0.7);
        let f = r.range(500.0, 2500.0);
        b.mix(&noise_burst(0.06, f, 2.0, 0.001, 0.02, r.next_u64()), t, r.range(0.2, 0.7) * (1.0 - t / dur));
    }
    b
}

fn chord_pad(dur: f32, notes: &[f32], seed: u64) -> Buf {
    let mut b = Buf::silence(dur);
    for (i, &n) in notes.iter().enumerate() {
        let f = midi(n);
        let v = unison(Wave::Saw, dur, move |_| f, adsr(0.4, 0.3, 0.8, 0.6, dur - 0.6), 3, 12.0, seed + i as u64);
        b.mix(&v, 0.0, 0.25);
    }
    b
}

/// Choir-like "aah": formant-filtered supersaw.
fn choir(dur: f32, notes: &[f32], seed: u64) -> Buf {
    let mut b = Buf::silence(dur);
    for (i, &n) in notes.iter().enumerate() {
        let f = midi(n);
        let src = unison(Wave::Saw, dur, move |t| f * (1.0 + (t * 5.0).sin() * 0.004), adsr(0.5, 0.2, 0.85, 0.5, dur - 0.5), 4, 14.0, seed + i as u64 * 7);
        for (ff, q, g) in [(700.0, 3.0, 1.0), (1150.0, 4.0, 0.6), (2600.0, 6.0, 0.25)] {
            let mut c = src.clone();
            c.bandpass(move |_| ff, q);
            b.mix(&c, 0.0, g * 0.5);
        }
    }
    b
}

fn timpani(note: f32, dur: f32, seed: u64) -> Buf {
    let f = midi(note);
    let mut b = osc(Wave::Sine, dur, move |t| f * (1.0 + 0.3 * (-t * 30.0).exp()), perc(0.002, 0.5));
    b.mix(&osc(Wave::Sine, dur, move |_| f * 1.5, perc(0.002, 0.25)), 0.0, 0.3);
    let mut n = noise(0.08, perc(0.001, 0.03), seed);
    n.lowpass(|_| 900.0, 0.7);
    b.mix(&n, 0.0, 0.6);
    b
}

fn brass(note: f32, dur: f32, seed: u64) -> Buf {
    let f = midi(note);
    let mut b = unison(Wave::Saw, dur, move |_| f, adsr(0.03, 0.15, 0.7, 0.2, dur - 0.2), 3, 8.0, seed);
    b.lowpass(move |t| 400.0 + 1800.0 * (-t * 4.0).exp(), 1.5);
    b
}

/// Warden Demon battle theme: D minor, ~100 bpm, low strings ostinato, brass,
/// choir and timpani.
fn boss_music() -> Buf {
    let beat = 0.6;
    let bars = 16;
    let total = beat * 4.0 * bars as f32;
    let mut out = Buf::silence(total + 3.0);
    // Chord roots per bar (MIDI): Dm Bb Gm A | Dm F Gm A | Bb Gm Dm A | Dm Bb E A
    let prog: [(f32, [f32; 3]); 16] = [
        (38.0, [62.0, 65.0, 69.0]),
        (34.0, [62.0, 65.0, 70.0]),
        (31.0, [62.0, 67.0, 70.0]),
        (33.0, [61.0, 64.0, 69.0]),
        (38.0, [62.0, 65.0, 69.0]),
        (41.0, [60.0, 65.0, 69.0]),
        (31.0, [62.0, 67.0, 70.0]),
        (33.0, [61.0, 64.0, 69.0]),
        (34.0, [62.0, 65.0, 70.0]),
        (31.0, [62.0, 67.0, 70.0]),
        (38.0, [62.0, 65.0, 69.0]),
        (33.0, [61.0, 64.0, 69.0]),
        (38.0, [62.0, 65.0, 69.0]),
        (34.0, [62.0, 65.0, 70.0]),
        (40.0, [62.0, 64.0, 68.0]),
        (33.0, [61.0, 64.0, 67.0]),
    ];
    let ostinato = [0.0, 0.0, 3.0, 0.0, 1.0, 0.0, -1.0, 0.0];
    for (bar, (root, chord)) in prog.iter().enumerate() {
        let t0 = bar as f32 * beat * 4.0;
        // Strings ostinato in eighths.
        for (i, off) in ostinato.iter().enumerate() {
            let f = midi(root + 12.0 + off);
            let mut n = unison(Wave::Saw, beat * 0.5, move |_| f, adsr(0.01, 0.1, 0.6, 0.05, beat * 0.42), 3, 10.0, (bar * 8 + i) as u64);
            n.lowpass(|_| 900.0, 1.0);
            out.mix(&n, t0 + i as f32 * beat * 0.5, 0.55);
        }
        // Sustained choir from bar 4, low pad throughout.
        let pad = chord_pad(beat * 4.0 + 0.4, &[root + 12.0, root + 19.0], bar as u64 * 13);
        out.mix(&pad, t0, 0.35);
        if bar >= 4 {
            out.mix(&choir(beat * 4.0 + 0.4, chord, bar as u64 * 31), t0, 0.32);
        }
        // Brass stabs on beat one and the "and" of three.
        if bar % 2 == 0 || bar >= 8 {
            for &(bt, len) in &[(0.0, 0.9), (2.5, 0.5)] {
                for &n in chord.iter() {
                    out.mix(&brass(n - 12.0, beat * len, (bar * 3) as u64), t0 + bt * beat, 0.16);
                }
            }
        }
        // Timpani.
        for b in 0..4 {
            let accent = if b == 0 { 1.0 } else if b == 2 { 0.7 } else { 0.35 };
            let note = if b % 2 == 0 { *root } else { root + 7.0 };
            out.mix(&timpani(note.max(31.0), 1.2, (bar * 4 + b) as u64), t0 + b as f32 * beat, 0.6 * accent);
        }
        if bar % 4 == 3 {
            for k in 0..4 {
                out.mix(&timpani(root.max(31.0), 0.6, (bar * 9 + k) as u64), t0 + (3.0 + k as f32 * 0.25) * beat, 0.4);
            }
        }
    }
    out.reverb(0.82, 0.35, 0.0);
    // Wrap the reverb tail into the beginning so the loop is seamless.
    let len = (total * out.rate) as usize;
    let tail: Vec<f32> = out.data[len..].to_vec();
    for (i, s) in tail.iter().enumerate() {
        if i < len {
            out.data[i] += s;
        }
    }
    out.data.truncate(len);
    out.normalize(0.85);
    out
}

fn ambient_dungeon() -> Buf {
    let dur = 12.0;
    let mut b = brown(dur, |_| 0.5, 3);
    b.lowpass(|t| 120.0 + (t * 0.5).sin() * 40.0, 0.8);
    let mut wind = noise(dur, |t| 0.25 + 0.15 * (t * 0.7).sin(), 4);
    wind.bandpass(|t| 350.0 + 120.0 * (t * 0.3).sin(), 2.0);
    b.mix(&wind, 0.0, 0.25);
    let mut r = Rng::new(99);
    for _ in 0..9 {
        let t = r.range(0.0, dur - 0.5);
        let f = r.range(900.0, 1500.0);
        let drip = osc(Wave::Sine, 0.12, move |tt| f * (1.0 - tt * 3.0), perc(0.001, 0.03));
        b.mix(&drip, t, r.range(0.08, 0.2));
    }
    b.reverb(0.85, 0.5, 0.0);
    b.loopify(1.5);
    b.normalize(0.5);
    b
}

fn ambient_wind() -> Buf {
    let dur = 12.0;
    let mut b = noise(dur, |t| 0.4 + 0.3 * (t * 0.45).sin() * (t * 0.17).cos(), 5);
    b.bandpass(|t| 500.0 + 300.0 * (t * 0.35).sin() + 150.0 * (t * 1.1).sin(), 1.5);
    let mut low = brown(dur, |_| 0.4, 6);
    low.lowpass(|_| 150.0, 0.7);
    b.mix(&low, 0.0, 0.6);
    b.loopify(2.0);
    b.normalize(0.5);
    b
}

pub fn generate(s: Sfx) -> Buf {
    let mut b = match s {
        Sfx::StepStone => {
            let mut b = noise_burst(0.08, 700.0, 1.0, 0.002, 0.02, 1);
            b.mix(&thump(0.06, 110.0, 70.0, 0.02), 0.0, 0.6);
            b
        }
        Sfx::StepArmor => {
            let mut b = generate(Sfx::StepStone);
            let mut m = metal(0.15, 1800.0, 0.05, 2);
            m.gain(0.25);
            b.mix(&m, 0.01, 1.0);
            b
        }
        Sfx::StepHusk => {
            let mut b = noise(0.07, perc(0.003, 0.02), 3);
            b.lowpass(|_| 600.0, 0.7);
            b
        }
        Sfx::StepDemon => {
            let mut b = thump(0.6, 60.0, 30.0, 0.2);
            let mut n = brown(0.5, perc(0.005, 0.15), 4);
            n.lowpass(|_| 250.0, 0.7);
            b.mix(&n, 0.0, 0.8);
            b.mix(&debris(0.5, 4, 5), 0.02, 0.3);
            b
        }
        Sfx::StepWater => {
            let mut b = noise(0.18, perc(0.005, 0.06), 6);
            b.bandpass(|t| 1500.0 - t * 4000.0, 1.0);
            b
        }
        Sfx::Roll => {
            let mut b = noise(0.45, |t| (std::f32::consts::PI * t / 0.45).sin(), 7);
            b.bandpass(|t| 900.0 - t * 1000.0, 0.8);
            let mut m = metal(0.2, 1400.0, 0.06, 8);
            m.gain(0.3);
            b.mix(&m, 0.05, 1.0);
            b.mix(&m, 0.3, 0.8);
            b.mix(&thump(0.1, 90.0, 60.0, 0.04), 0.32, 0.5);
            b
        }
        Sfx::SwingLight => whoosh(0.28, 600.0, 2600.0, 900.0, 9),
        Sfx::SwingHeavy => whoosh(0.42, 400.0, 1700.0, 500.0, 10),
        Sfx::SwingDemon => {
            let mut b = whoosh(0.7, 150.0, 800.0, 200.0, 11);
            let mut l = brown(0.7, |t| (std::f32::consts::PI * t / 0.7).sin(), 12);
            l.lowpass(|_| 200.0, 0.7);
            b.mix(&l, 0.0, 0.7);
            b
        }
        Sfx::HitFlesh => {
            let mut b = noise_burst(0.05, 1600.0, 0.8, 0.001, 0.015, 13);
            b.mix(&thump(0.15, 140.0, 70.0, 0.05), 0.0, 0.9);
            let mut sq = noise(0.12, perc(0.005, 0.04), 14);
            sq.bandpass(|t| 500.0 - t * 1500.0, 2.0);
            b.mix(&sq, 0.01, 0.7);
            b
        }
        Sfx::HitArmor => {
            let mut b = metal(0.3, 900.0, 0.12, 15);
            b.gain(0.6);
            b.mix(&thump(0.15, 120.0, 60.0, 0.05), 0.0, 0.9);
            b.mix(&generate(Sfx::HitFlesh), 0.0, 0.4);
            b
        }
        Sfx::HitDemon => {
            let mut b = thump(0.25, 80.0, 45.0, 0.08);
            let mut n = noise(0.2, perc(0.002, 0.05), 16);
            n.lowpass(|_| 700.0, 0.8);
            b.mix(&n, 0.0, 0.8);
            b
        }
        Sfx::Block => {
            let mut b = metal(0.6, 620.0, 0.3, 17);
            b.mix(&thump(0.12, 150.0, 80.0, 0.04), 0.0, 0.5);
            b
        }
        Sfx::GuardBreak => {
            let mut b = metal(0.8, 480.0, 0.35, 18);
            b.mix(&osc(Wave::Square, 0.5, |t| 300.0 - t * 300.0, perc(0.01, 0.2)), 0.05, 0.15);
            b.mix(&thump(0.2, 120.0, 50.0, 0.08), 0.0, 0.8);
            b
        }
        Sfx::Hurt => voice(0.3, |t| 150.0 - t * 120.0, perc(0.01, 0.12), &[(700.0, 4.0, 1.0), (1200.0, 5.0, 0.6)], 0.25, 19),
        Sfx::Death => voice(1.0, |t| 130.0 - t * 50.0, |t| (t * 20.0).min(1.0) * (1.0 - t).max(0.0), &[(650.0, 4.0, 1.0), (1000.0, 5.0, 0.6)], 0.35, 20),
        Sfx::HuskGroan => voice(0.8, |t| 85.0 + (t * 6.0).sin() * 8.0, |t| (t * 8.0).min(1.0) * (1.0 - t / 0.8).max(0.0), &[(450.0, 3.0, 1.0), (950.0, 4.0, 0.5)], 0.6, 21),
        Sfx::HuskDeath => voice(1.0, |t| 100.0 - t * 50.0, |t| (t * 15.0).min(1.0) * (1.0 - t).max(0.0), &[(500.0, 3.0, 1.0), (900.0, 4.0, 0.5)], 0.6, 22),
        Sfx::DemonRoar => {
            let mut b = voice(
                2.2,
                |t| 58.0 + (t * 7.0).sin() * 4.0 + if t < 0.3 { t * 40.0 } else { 12.0 - (t - 0.3) * 6.0 },
                |t| (t * 4.0).min(1.0) * (1.0 - (t / 2.2).powi(2)),
                &[(380.0, 3.0, 1.0), (750.0, 4.0, 0.7), (1400.0, 5.0, 0.3)],
                0.5,
                23,
            );
            b.drive(2.5).reverb(0.75, 0.35, 0.6);
            b
        }
        Sfx::DemonGrunt => {
            let mut b = voice(0.8, |t| 62.0 - t * 15.0, perc(0.05, 0.3), &[(420.0, 3.0, 1.0), (800.0, 4.0, 0.6)], 0.5, 24);
            b.drive(2.0);
            b
        }
        Sfx::DemonHurt => {
            let mut b = voice(0.9, |t| 90.0 + t * 30.0, perc(0.02, 0.35), &[(500.0, 3.0, 1.0), (1000.0, 4.0, 0.7)], 0.5, 25);
            b.drive(2.2);
            b
        }
        Sfx::DemonDeath => {
            let mut b = voice(
                3.8,
                |t| 70.0 - t * 12.0 + (t * 5.0).sin() * 3.0,
                |t| (t * 3.0).min(1.0) * (1.0 - t / 3.8).max(0.0).powf(0.7),
                &[(360.0, 3.0, 1.0), (720.0, 4.0, 0.6), (1300.0, 5.0, 0.3)],
                0.6,
                26,
            );
            b.drive(2.0).reverb(0.85, 0.45, 1.0);
            b
        }
        Sfx::DemonSlam => {
            let mut b = thump(1.2, 50.0, 22.0, 0.35);
            let mut n = brown(1.0, perc(0.003, 0.3), 27);
            n.lowpass(|_| 300.0, 0.7);
            b.mix(&n, 0.0, 1.0);
            b.mix(&debris(1.2, 14, 28), 0.03, 0.5);
            b.drive(1.5).reverb(0.7, 0.3, 0.6);
            b
        }
        Sfx::DemonWing => {
            let mut b = noise(0.35, |t| (std::f32::consts::PI * t / 0.35).sin().powi(2), 29);
            b.lowpass(|_| 350.0, 1.0);
            b.gain(2.0);
            b
        }
        Sfx::Drink => {
            let mut b = Buf::silence(0.9);
            for k in 0..3 {
                let g = osc(Wave::Sine, 0.12, |t| 180.0 + t * 1800.0, perc(0.01, 0.05));
                b.mix(&g, 0.15 + k as f32 * 0.22, 0.7);
            }
            b
        }
        Sfx::Heal => {
            let mut b = modal(1.6, &[(880.0, 0.5, 0.6), (1318.5, 0.4, 0.5), (1760.0, 0.3, 0.4), (2637.0, 0.15, 0.3)]);
            b.env(|t| (t * 10.0).min(1.0));
            b.reverb(0.8, 0.5, 0.5);
            b
        }
        Sfx::PyreIgnite => {
            let mut b = whoosh(1.2, 200.0, 1200.0, 400.0, 30);
            b.mix(&crackle(1.5, 40.0, 31), 0.2, 0.5);
            b.mix(&thump(0.4, 90.0, 50.0, 0.15), 0.0, 0.5);
            b
        }
        Sfx::PyreLoop => {
            let mut b = crackle(4.0, 25.0, 32);
            let mut h = noise(4.0, |_| 0.15, 33);
            h.lowpass(|_| 500.0, 0.7);
            b.mix(&h, 0.0, 1.0);
            b.loopify(0.3);
            b
        }
        Sfx::DoorOpen => {
            let mut b = osc(Wave::Saw, 1.1, |t| 130.0 + 40.0 * (t * 9.0).sin() + 20.0 * (t * 23.0).sin(), |t| (t * 8.0).min(1.0) * (1.0 - t / 1.1));
            b.bandpass(|_| 800.0, 3.0);
            b.mix(&thump(0.3, 100.0, 60.0, 0.1), 1.0, 0.8);
            b
        }
        Sfx::DoorLocked => {
            let mut b = Buf::silence(0.4);
            for k in 0..3 {
                b.mix(&metal(0.12, 2000.0 + k as f32 * 300.0, 0.03, 34 + k), k as f32 * 0.09, 0.4);
            }
            b.mix(&thump(0.1, 120.0, 80.0, 0.03), 0.0, 0.6);
            b
        }
        Sfx::GateSlam => {
            let mut b = metal(1.5, 180.0, 0.6, 37);
            b.mix(&thump(0.6, 70.0, 35.0, 0.2), 0.0, 1.0);
            b.mix(&debris(0.8, 10, 38), 0.0, 0.3);
            for k in 0..6 {
                b.mix(&metal(0.08, 2500.0, 0.02, 40 + k), 0.05 + k as f32 * 0.05, 0.2);
            }
            b.reverb(0.7, 0.3, 0.5);
            b
        }
        Sfx::FogPass => {
            let mut b = noise(1.4, |t| (std::f32::consts::PI * t / 1.4).sin(), 41);
            b.highpass(|t| 1500.0 + 2500.0 * t, 0.7);
            b.mix(&osc(Wave::Sine, 1.4, |t| 220.0 + t * 110.0, |t| (std::f32::consts::PI * t / 1.4).sin() * 0.2), 0.0, 1.0);
            b.reverb(0.8, 0.5, 0.5);
            b
        }
        Sfx::ItemPickup => {
            let mut b = modal(0.8, &[(660.0, 0.5, 0.3), (990.0, 0.3, 0.25)]);
            b.mix(&modal(0.8, &[(880.0, 0.5, 0.3), (1320.0, 0.3, 0.25)]), 0.12, 1.0);
            b.reverb(0.7, 0.4, 0.3);
            b
        }
        Sfx::UrnBreak => {
            let mut r = Rng::new(42);
            let mut b = Buf::silence(0.6);
            for _ in 0..18 {
                let t = r.range(0.0, 0.35);
                b.mix(&noise_burst(0.05, r.range(2000.0, 6000.0), 3.0, 0.0005, 0.015, r.next_u64()), t, r.range(0.3, 1.0) * (1.0 - t * 2.0));
            }
            b.mix(&thump(0.1, 200.0, 120.0, 0.03), 0.0, 0.5);
            b
        }
        Sfx::ArrowShoot => {
            let mut b = osc(Wave::Triangle, 0.3, |_| 196.0, perc(0.001, 0.08));
            b.mix(&noise_burst(0.02, 3000.0, 1.0, 0.0005, 0.005, 43), 0.0, 0.5);
            b.mix(&whoosh(0.25, 1500.0, 3000.0, 2000.0, 44), 0.02, 0.4);
            b
        }
        Sfx::ArrowHit => {
            let mut b = thump(0.12, 300.0, 150.0, 0.03);
            b.mix(&noise_burst(0.05, 1200.0, 1.5, 0.001, 0.015, 45), 0.0, 0.7);
            b
        }
        Sfx::BoulderRoll => {
            let mut b = brown(3.0, |t| 0.6 + 0.4 * (t * 9.0).sin().abs(), 46);
            b.lowpass(|_| 180.0, 0.9);
            b.mix(&debris(3.0, 20, 47), 0.0, 0.2);
            b.loopify(0.3);
            b
        }
        Sfx::WallCrash => {
            let mut b = noise(1.5, perc(0.002, 0.4), 48);
            b.lowpass(|t| 3000.0 * (-t * 2.0).exp() + 200.0, 0.8);
            b.mix(&thump(0.8, 60.0, 30.0, 0.3), 0.0, 1.0);
            b.mix(&debris(1.5, 30, 49), 0.05, 0.6);
            b.reverb(0.7, 0.3, 0.6);
            b
        }
        Sfx::Land => {
            let mut b = thump(0.2, 100.0, 50.0, 0.06);
            b.mix(&generate(Sfx::StepArmor), 0.0, 0.8);
            b
        }
        Sfx::Cinders => {
            let mut b = noise(1.0, |t| (std::f32::consts::PI * t).sin(), 50);
            b.bandpass(|t| 400.0 + t * 3000.0, 2.0);
            b.mix(&osc(Wave::Sine, 1.0, |t| 300.0 + t * 600.0, |t| (std::f32::consts::PI * t).sin() * 0.3), 0.0, 1.0);
            b.reverb(0.7, 0.4, 0.3);
            b
        }
        Sfx::MenuMove => osc(Wave::Square, 0.04, |_| 880.0, perc(0.001, 0.015)),
        Sfx::MenuSelect => {
            let mut b = modal(0.5, &[(523.0, 0.5, 0.2), (784.0, 0.3, 0.15)]);
            b.reverb(0.6, 0.3, 0.2);
            b
        }
        Sfx::YouDied => {
            let mut b = Buf::silence(4.5);
            for (n, g) in [(38.0, 1.0), (45.0, 0.7), (50.0, 0.5), (51.0, 0.35)] {
                let f = midi(n);
                let mut v = unison(Wave::Saw, 4.2, move |_| f, adsr(0.05, 0.5, 0.6, 2.5, 1.5), 4, 18.0, n as u64);
                v.lowpass(|t| 2000.0 * (-t * 1.2).exp() + 200.0, 1.0);
                b.mix(&v, 0.0, 0.3 * g);
            }
            b.mix(&thump(1.5, 55.0, 30.0, 0.5), 0.0, 0.8);
            b.reverb(0.85, 0.5, 0.5);
            b
        }
        Sfx::Victory => {
            let mut b = Buf::silence(5.0);
            let notes = [62.0, 66.0, 69.0, 74.0, 78.0];
            b.mix(&choir(4.5, &notes, 51), 0.0, 0.9);
            for (k, &n) in notes.iter().enumerate() {
                b.mix(&brass(n - 12.0, 1.6, 52 + k as u64), 0.2 + k as f32 * 0.08, 0.25);
            }
            b.mix(&timpani(38.0, 2.0, 53), 0.0, 0.8);
            b.reverb(0.85, 0.5, 0.6);
            b
        }
        Sfx::AmbientDungeon => ambient_dungeon(),
        Sfx::AmbientWind => ambient_wind(),
        Sfx::BossMusic => boss_music(),
        Sfx::Crow => {
            let mut b = Buf::silence(0.9);
            for k in 0..2 {
                let mut c = voice(0.3, |t| 600.0 - t * 500.0, perc(0.01, 0.12), &[(1200.0, 3.0, 1.0), (2400.0, 4.0, 0.5)], 0.6, 54 + k);
                c.drive(3.0);
                b.mix(&c, k as f32 * 0.42, 0.8);
            }
            b.reverb(0.8, 0.5, 0.8);
            b
        }
        Sfx::Bell => {
            let f = 110.0;
            let mut b = modal(6.0, &[(f * 0.5, 0.6, 4.0), (f, 0.8, 3.0), (f * 1.2, 0.5, 2.0), (f * 1.5, 0.4, 1.6), (f * 2.0, 0.5, 1.2), (f * 2.51, 0.3, 0.8), (f * 3.0, 0.2, 0.6)]);
            b.mix(&noise_burst(0.03, 800.0, 1.0, 0.0005, 0.01, 55), 0.0, 0.5);
            b.reverb(0.85, 0.5, 1.0);
            b
        }
    };
    if !s.looping() {
        b.trim(0.0005);
    }
    let peak = b.peak();
    if peak > 0.95 {
        b.gain(0.95 / peak);
    }
    if s.bus() == Bus::Sfx {
        // A touch of 12-bit-ish crunch for the PSX feel.
        b.crush(11);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_sounds_are_finite_and_bounded() {
        for &s in Sfx::ALL {
            if s == Sfx::BossMusic {
                continue;
            }
            let b = generate(s);
            assert!(!b.is_empty(), "{s:?}");
            assert!(b.data.iter().all(|x| x.is_finite() && x.abs() <= 1.0), "{s:?}");
        }
    }
}
