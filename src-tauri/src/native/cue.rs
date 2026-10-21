// SPDX-License-Identifier: MIT
//! Original, quiet recording-ready cue, generated once and held for async playback.
use std::sync::OnceLock;
use windows::{core::PCWSTR, Win32::Media::Audio::*};

fn recording_wav() -> &'static [u8] {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    WAV.get_or_init(|| {
        const RATE: u32 = 22_050;
        const SAMPLES: usize = 2_646; // 120 ms, with soft edges to avoid clicks.
        let mut wav = Vec::with_capacity(44 + SAMPLES * 2);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36u32 + SAMPLES as u32 * 2).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
        wav.extend_from_slice(&RATE.to_le_bytes());
        wav.extend_from_slice(&(RATE * 2).to_le_bytes());
        wav.extend_from_slice(b"\x02\0\x10\0data");
        wav.extend_from_slice(&(SAMPLES as u32 * 2).to_le_bytes());
        for i in 0..SAMPLES {
            let t = i as f64 / RATE as f64;
            let attack = (t / 0.008).min(1.0);
            let release = ((0.12 - t) / 0.025).clamp(0.0, 1.0);
            let envelope = attack * release * (-t * 20.0).exp();
            // A low, softly falling fundamental and a restrained second harmonic.
            let phase = std::f64::consts::TAU * (220.0 * t - 150.0 * t * t);
            let sample = ((phase.sin() + (phase * 2.0).sin() * 0.14) * envelope * 3_500.0) as i16;
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    })
}

pub fn play_recording_cue() {
    // SND_MEMORY's asynchronous buffer must outlive playback; OnceLock keeps it
    // for the process lifetime. No file, service, or idle rendering is involved.
    unsafe {
        let _ = PlaySoundW(
            PCWSTR(recording_wav().as_ptr().cast()),
            None,
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT | SND_NOSTOP,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cue_is_short_quiet_pcm_with_soft_edges() {
        let wav = recording_wav();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), 44 + 2_646 * 2);
        let samples: Vec<i16> = wav[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|s| i16::from_le_bytes([s[0], s[1]]))
            .collect();
        assert_eq!(samples[0], 0);
        assert!(samples.iter().all(|s| s.unsigned_abs() < 4_000));
        assert!(samples.last().unwrap().unsigned_abs() < 20);
    }
}
