//! Offline DSP building blocks for procedural sound effects and music.
//!
//! Sounds are rendered once at start-up into mono buffers at a low sample
//! rate (PSX SPU-ish crunch), then handed to the platform audio backend.

use crate::rng::Rng;
use std::f32::consts::TAU;

/// Default sample rate for generated audio.
pub const SR: f32 = 22050.0;

#[derive(Clone, Debug, Default)]
pub struct Buf {
    pub rate: f32,
    pub data: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wave {
    Sine,
    Saw,
    Square,
    Triangle,
    /// Pulse with 25% duty.
    Pulse,
}

impl Wave {
    #[inline]
    fn eval(self, ph: f32) -> f32 {
        match self {
            Wave::Sine => (ph * TAU).sin(),
            Wave::Saw => 2.0 * ph - 1.0,
            Wave::Square => {
                if ph < 0.5 { 1.0 } else { -1.0 }
            }
            Wave::Triangle => 1.0 - 4.0 * (ph - 0.5).abs(),
            Wave::Pulse => {
                if ph < 0.25 { 1.0 } else { -0.33 }
            }
        }
    }
}

impl Buf {
    pub fn silence(secs: f32) -> Self {
        Buf { rate: SR, data: vec![0.0; (secs * SR).ceil() as usize] }
    }

    pub fn from_fn(secs: f32, mut f: impl FnMut(f32, usize) -> f32) -> Self {
        let n = (secs * SR).ceil() as usize;
        let data = (0..n).map(|i| f(i as f32 / SR, i)).collect();
        Buf { rate: SR, data }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn secs(&self) -> f32 {
        self.data.len() as f32 / self.rate
    }

    /// Mixes `other` in starting at `at` seconds, growing as needed.
    pub fn mix(&mut self, other: &Buf, at: f32, gain: f32) -> &mut Self {
        let start = (at * self.rate).round().max(0.0) as usize;
        if self.data.len() < start + other.data.len() {
            self.data.resize(start + other.data.len(), 0.0);
        }
        for (i, s) in other.data.iter().enumerate() {
            self.data[start + i] += s * gain;
        }
        self
    }

    pub fn gain(&mut self, g: f32) -> &mut Self {
        for s in self.data.iter_mut() {
            *s *= g;
        }
        self
    }

    pub fn peak(&self) -> f32 {
        self.data.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    pub fn normalize(&mut self, peak: f32) -> &mut Self {
        let p = self.peak();
        if p > 1e-6 {
            self.gain(peak / p);
        }
        self
    }

    /// Multiplies by an envelope `f(t)`.
    pub fn env(&mut self, f: impl Fn(f32) -> f32) -> &mut Self {
        let r = self.rate;
        for (i, s) in self.data.iter_mut().enumerate() {
            *s *= f(i as f32 / r);
        }
        self
    }

    pub fn fade(&mut self, fade_in: f32, fade_out: f32) -> &mut Self {
        let len = self.secs();
        self.env(|t| {
            let a = if fade_in > 0.0 { (t / fade_in).min(1.0) } else { 1.0 };
            let b = if fade_out > 0.0 { ((len - t) / fade_out).clamp(0.0, 1.0) } else { 1.0 };
            a * b
        })
    }

    fn biquad(&mut self, kind: FilterKind, cutoff: impl Fn(f32) -> f32, q: f32) -> &mut Self {
        let mut f = Biquad::default();
        let r = self.rate;
        for (i, s) in self.data.iter_mut().enumerate() {
            if i % 16 == 0 {
                let fc = cutoff(i as f32 / r).clamp(10.0, r * 0.49);
                f.set(kind, fc, q, r);
            }
            *s = f.process(*s);
        }
        self
    }

    pub fn lowpass(&mut self, cutoff: impl Fn(f32) -> f32, q: f32) -> &mut Self {
        self.biquad(FilterKind::Low, cutoff, q)
    }

    pub fn highpass(&mut self, cutoff: impl Fn(f32) -> f32, q: f32) -> &mut Self {
        self.biquad(FilterKind::High, cutoff, q)
    }

    pub fn bandpass(&mut self, center: impl Fn(f32) -> f32, q: f32) -> &mut Self {
        self.biquad(FilterKind::Band, center, q)
    }

    /// Soft clipping.
    pub fn drive(&mut self, amount: f32) -> &mut Self {
        let norm = amount.tanh().max(1e-4);
        for s in self.data.iter_mut() {
            *s = (*s * amount).tanh() / norm;
        }
        self
    }

    /// Quantises amplitude to `bits`.
    pub fn crush(&mut self, bits: u32) -> &mut Self {
        let levels = (1u32 << bits) as f32 * 0.5;
        for s in self.data.iter_mut() {
            *s = (*s * levels).round() / levels;
        }
        self
    }

    /// Feedback delay.
    pub fn echo(&mut self, delay: f32, feedback: f32, wet: f32, tail: f32) -> &mut Self {
        let d = (delay * self.rate) as usize;
        if d == 0 {
            return self;
        }
        let extra = (tail * self.rate) as usize;
        self.data.resize(self.data.len() + extra, 0.0);
        let dry = self.data.clone();
        let mut line = vec![0.0f32; self.data.len()];
        for i in 0..self.data.len() {
            let fb = if i >= d { line[i - d] } else { 0.0 };
            line[i] = dry[i] + fb * feedback;
            self.data[i] = dry[i] + if i >= d { line[i - d] * wet } else { 0.0 };
        }
        self
    }

    /// Simple Schroeder reverb baked into the buffer.
    pub fn reverb(&mut self, room: f32, wet: f32, tail: f32) -> &mut Self {
        let extra = (tail * self.rate) as usize;
        self.data.resize(self.data.len() + extra, 0.0);
        let dry = self.data.clone();
        let combs = [1557, 1617, 1491, 1422].map(|n| ((n as f32) * self.rate / 44100.0) as usize);
        let mut out = vec![0.0f32; dry.len()];
        for &cl in &combs {
            let mut buf = vec![0.0f32; cl];
            let mut idx = 0;
            let mut lp = 0.0f32;
            for i in 0..dry.len() {
                let y = buf[idx];
                lp = y * 0.6 + lp * 0.4;
                buf[idx] = dry[i] + lp * room;
                idx = (idx + 1) % cl;
                out[i] += y * 0.25;
            }
        }
        for &al in &[556usize, 441, 341] {
            let al = ((al as f32) * self.rate / 44100.0) as usize;
            let mut buf = vec![0.0f32; al];
            let mut idx = 0;
            for s in out.iter_mut() {
                let b = buf[idx];
                let y = -*s + b;
                buf[idx] = *s + b * 0.5;
                idx = (idx + 1) % al;
                *s = y;
            }
        }
        for i in 0..dry.len() {
            self.data[i] = dry[i] + out[i] * wet;
        }
        self
    }

    /// Plays the buffer at a different speed (cheap linear resampling).
    pub fn pitched(&self, factor: f32) -> Buf {
        let n = (self.data.len() as f32 / factor) as usize;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let x = i as f32 * factor;
            let j = x as usize;
            let f = x - j as f32;
            let a = self.data.get(j).copied().unwrap_or(0.0);
            let b = self.data.get(j + 1).copied().unwrap_or(0.0);
            out.push(a + (b - a) * f);
        }
        Buf { rate: self.rate, data: out }
    }

    pub fn reversed(&self) -> Buf {
        let mut d = self.data.clone();
        d.reverse();
        Buf { rate: self.rate, data: d }
    }

    /// Trims trailing near-silence.
    pub fn trim(&mut self, threshold: f32) -> &mut Self {
        while self.data.last().is_some_and(|s| s.abs() < threshold) {
            self.data.pop();
        }
        self
    }

    /// Makes a loop seamless by crossfading the tail into the head.
    pub fn loopify(&mut self, xfade: f32) -> &mut Self {
        let n = ((xfade * self.rate) as usize).min(self.data.len() / 2);
        let len = self.data.len() - n;
        for i in 0..n {
            let t = i as f32 / n as f32;
            self.data[i] = self.data[i] * t + self.data[len + i] * (1.0 - t);
        }
        self.data.truncate(len);
        self
    }

    /// Encodes as 16-bit PCM WAV (handy for inspecting sounds offline).
    pub fn to_wav(&self) -> Vec<u8> {
        let n = self.data.len() as u32;
        let rate = self.rate as u32;
        let mut v = Vec::with_capacity(44 + n as usize * 2);
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + n * 2).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * 2).to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(n * 2).to_le_bytes());
        for s in &self.data {
            v.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        v
    }
}

#[derive(Clone, Copy, PartialEq)]
enum FilterKind {
    Low,
    High,
    Band,
}

#[derive(Default, Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn set(&mut self, kind: FilterKind, fc: f32, q: f32, rate: f32) {
        let w0 = TAU * fc / rate;
        let (s, c) = w0.sin_cos();
        let alpha = s / (2.0 * q.max(0.05));
        let (b0, b1, b2) = match kind {
            FilterKind::Low => ((1.0 - c) * 0.5, 1.0 - c, (1.0 - c) * 0.5),
            FilterKind::High => ((1.0 + c) * 0.5, -(1.0 + c), (1.0 + c) * 0.5),
            FilterKind::Band => (alpha, 0.0, -alpha),
        };
        let a0 = 1.0 + alpha;
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = -2.0 * c / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Oscillator with time-varying frequency and amplitude.
pub fn osc(wave: Wave, secs: f32, freq: impl Fn(f32) -> f32, amp: impl Fn(f32) -> f32) -> Buf {
    let mut ph = 0.0f32;
    Buf::from_fn(secs, |t, _| {
        let v = wave.eval(ph) * amp(t);
        ph = (ph + freq(t) / SR).fract();
        v
    })
}

/// Several detuned oscillators summed (a thicker "supersaw").
pub fn unison(wave: Wave, secs: f32, freq: impl Fn(f32) -> f32, amp: impl Fn(f32) -> f32, voices: u32, detune_cents: f32, seed: u64) -> Buf {
    let mut rng = Rng::new(seed);
    let mut phases: Vec<f32> = (0..voices).map(|_| rng.f32()).collect();
    let ratios: Vec<f32> = (0..voices)
        .map(|i| {
            let k = if voices > 1 { i as f32 / (voices - 1) as f32 * 2.0 - 1.0 } else { 0.0 };
            2f32.powf(k * detune_cents / 1200.0)
        })
        .collect();
    let norm = 1.0 / (voices as f32).sqrt();
    Buf::from_fn(secs, |t, _| {
        let f = freq(t);
        let mut v = 0.0;
        for (p, r) in phases.iter_mut().zip(&ratios) {
            v += wave.eval(*p);
            *p = (*p + f * r / SR).fract();
        }
        v * norm * amp(t)
    })
}

/// Two-operator FM.
pub fn fm(secs: f32, carrier: impl Fn(f32) -> f32, ratio: f32, index: impl Fn(f32) -> f32, amp: impl Fn(f32) -> f32) -> Buf {
    let mut pc = 0.0f32;
    let mut pm = 0.0f32;
    Buf::from_fn(secs, |t, _| {
        let f = carrier(t);
        let m = (pm * TAU).sin() * index(t);
        let v = ((pc + m / TAU) * TAU).sin() * amp(t);
        pc = (pc + f / SR).fract();
        pm = (pm + f * ratio / SR).fract();
        v
    })
}

/// White noise.
pub fn noise(secs: f32, amp: impl Fn(f32) -> f32, seed: u64) -> Buf {
    let mut r = Rng::new(seed);
    Buf::from_fn(secs, |t, _| r.signed() * amp(t))
}

/// Brown (integrated) noise, darker rumble.
pub fn brown(secs: f32, amp: impl Fn(f32) -> f32, seed: u64) -> Buf {
    let mut r = Rng::new(seed);
    let mut x = 0.0f32;
    Buf::from_fn(secs, |t, _| {
        x = (x + r.signed() * 0.08) * 0.985;
        x * 3.0 * amp(t)
    })
}

/// Bank of exponentially decaying sine partials: `(freq, amp, decay_secs)`.
/// Good for bells, metal clanks and struck objects.
pub fn modal(secs: f32, partials: &[(f32, f32, f32)]) -> Buf {
    let mut ph: Vec<f32> = vec![0.0; partials.len()];
    Buf::from_fn(secs, |t, _| {
        let mut v = 0.0;
        for (k, &(f, a, d)) in partials.iter().enumerate() {
            v += (ph[k] * TAU).sin() * a * (-t / d.max(1e-4)).exp();
            ph[k] = (ph[k] + f / SR).fract();
        }
        v
    })
}

/// Classic attack/decay envelope.
pub fn perc(attack: f32, decay: f32) -> impl Fn(f32) -> f32 + Copy {
    move |t| if t < attack { t / attack.max(1e-5) } else { (-(t - attack) / decay.max(1e-5)).exp() }
}

/// Linear ADSR given the note length `hold` (time until release).
pub fn adsr(a: f32, d: f32, s: f32, r: f32, hold: f32) -> impl Fn(f32) -> f32 + Copy {
    move |t| {
        let v = if t < a {
            t / a.max(1e-5)
        } else if t < a + d {
            1.0 - (1.0 - s) * (t - a) / d.max(1e-5)
        } else {
            s
        };
        if t < hold { v } else { (v * (1.0 - (t - hold) / r.max(1e-5))).max(0.0) }
    }
}

/// MIDI note number to frequency.
pub fn midi(n: f32) -> f32 {
    440.0 * 2f32.powf((n - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_are_stable() {
        let mut b = noise(0.5, |_| 1.0, 1);
        b.lowpass(|t| 200.0 + t * 4000.0, 4.0).highpass(|_| 80.0, 0.7).bandpass(|_| 1000.0, 2.0);
        assert!(b.data.iter().all(|s| s.is_finite()));
        assert!(b.peak() < 10.0);
    }

    #[test]
    fn reverb_and_echo_finite() {
        let mut b = modal(0.3, &[(440.0, 1.0, 0.1), (1170.0, 0.5, 0.05)]);
        b.reverb(0.8, 0.4, 1.0).echo(0.1, 0.4, 0.5, 0.5).normalize(0.9);
        assert!(b.data.iter().all(|s| s.is_finite()));
        assert!((b.peak() - 0.9).abs() < 1e-4);
    }

    #[test]
    fn loopify_shortens() {
        let mut b = osc(Wave::Sine, 1.0, |_| 100.0, |_| 1.0);
        let n = b.len();
        b.loopify(0.1);
        assert!(b.len() < n);
    }
}
