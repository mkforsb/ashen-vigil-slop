//! WebAudio backend: plays the game's procedurally generated buffers.

use ashen_engine::synth::Buf;
use ashen_game::assets::sounds::Sfx;
use ashen_game::game::LoopOut;
use ashen_game::world::SoundOut;
use std::collections::HashMap;
use wasm_bindgen::JsValue;
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, GainNode, StereoPannerNode};

struct Voice {
    sfx: Sfx,
    src: AudioBufferSourceNode,
    gain: GainNode,
    pan: StereoPannerNode,
}

pub struct WebAudio {
    ctx: AudioContext,
    master: GainNode,
    buffers: HashMap<Sfx, AudioBuffer>,
    loops: HashMap<u32, Voice>,
    voices_this_frame: usize,
}

impl WebAudio {
    pub fn new() -> Result<Self, JsValue> {
        let ctx = AudioContext::new()?;
        let master = ctx.create_gain()?;
        master.gain().set_value(1.0);
        master.connect_with_audio_node(&ctx.destination())?;
        // A shared room reverb send.
        let conv = ctx.create_convolver()?;
        let rate = ctx.sample_rate();
        let len = (rate * 1.6) as u32;
        let ir = ctx.create_buffer(2, len, rate)?;
        let mut seed = 12345u32;
        for ch in 0..2 {
            let mut data = vec![0f32; len as usize];
            for (i, d) in data.iter_mut().enumerate() {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let n = (seed >> 9) as f32 / (1u32 << 23) as f32 * 2.0 - 1.0;
                *d = n * (-(i as f32) / (rate * 0.35)).exp();
            }
            ir.copy_to_channel(&mut data, ch)?;
        }
        conv.set_buffer(Some(&ir));
        let wet = ctx.create_gain()?;
        wet.gain().set_value(0.16);
        master.connect_with_audio_node(&conv)?;
        conv.connect_with_audio_node(&wet)?;
        wet.connect_with_audio_node(&ctx.destination())?;
        Ok(WebAudio { ctx, master, buffers: HashMap::new(), loops: HashMap::new(), voices_this_frame: 0 })
    }

    pub fn add(&mut self, sfx: Sfx, b: &Buf) -> Result<(), JsValue> {
        if b.data.is_empty() {
            return Ok(());
        }
        let buf = self.ctx.create_buffer(1, b.data.len() as u32, b.rate)?;
        let mut data = b.data.clone();
        buf.copy_to_channel(&mut data, 0)?;
        self.buffers.insert(sfx, buf);
        Ok(())
    }

    pub fn resume(&self) {
        if self.ctx.state() != AudioContextState::Running {
            let _ = self.ctx.resume();
        }
    }

    pub fn running(&self) -> bool {
        self.ctx.state() == AudioContextState::Running
    }

    fn voice(&self, sfx: Sfx, gain: f32, pan: f32, rate: f32, looping: bool) -> Option<Voice> {
        let buf = self.buffers.get(&sfx)?;
        let src = self.ctx.create_buffer_source().ok()?;
        src.set_buffer(Some(buf));
        src.set_loop(looping);
        src.playback_rate().set_value(rate);
        let g = self.ctx.create_gain().ok()?;
        g.gain().set_value(gain);
        let p = self.ctx.create_stereo_panner().ok()?;
        p.pan().set_value(pan.clamp(-1.0, 1.0));
        src.connect_with_audio_node(&g).ok()?;
        g.connect_with_audio_node(&p).ok()?;
        p.connect_with_audio_node(&self.master).ok()?;
        src.start().ok()?;
        Some(Voice { sfx, src, gain: g, pan: p })
    }

    pub fn play(&mut self, s: &SoundOut) {
        if !self.running() || s.gain < 0.005 || self.voices_this_frame > 24 {
            return;
        }
        self.voices_this_frame += 1;
        let _ = self.voice(s.sfx, s.gain.min(1.5), s.pan, s.rate, false);
    }

    pub fn update_loops(&mut self, loops: &[LoopOut]) {
        self.voices_this_frame = 0;
        if !self.running() {
            return;
        }
        let now = self.ctx.current_time();
        let keep: Vec<u32> = loops.iter().map(|l| l.key).collect();
        let stale: Vec<u32> = self.loops.keys().copied().filter(|k| !keep.contains(k)).collect();
        for k in stale {
            if let Some(v) = self.loops.remove(&k) {
                let _ = v.gain.gain().set_target_at_time(0.0, now, 0.15);
                let _ = AsRef::<web_sys::AudioScheduledSourceNode>::as_ref(&v.src).stop_with_when(now + 0.8);
            }
        }
        for l in loops {
            let replace = self.loops.get(&l.key).is_some_and(|v| v.sfx != l.sfx);
            if replace {
                if let Some(v) = self.loops.remove(&l.key) {
                    let _ = v.gain.gain().set_target_at_time(0.0, now, 0.4);
                    let _ = AsRef::<web_sys::AudioScheduledSourceNode>::as_ref(&v.src).stop_with_when(now + 2.0);
                }
            }
            match self.loops.get(&l.key) {
                Some(v) => {
                    let _ = v.gain.gain().set_target_at_time(l.gain, now, 0.08);
                    let _ = v.pan.pan().set_target_at_time(l.pan.clamp(-1.0, 1.0), now, 0.08);
                }
                None => {
                    if let Some(v) = self.voice(l.sfx, 0.0, l.pan, 1.0, true) {
                        let _ = v.gain.gain().set_target_at_time(l.gain, now, 0.4);
                        self.loops.insert(l.key, v);
                    }
                }
            }
        }
    }
}
