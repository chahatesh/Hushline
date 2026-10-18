// SPDX-License-Identifier: MIT
//! An event-driven WASAPI stream exists only during capture. Raw audio never touches disk.
use crate::model::Settings;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    io::Cursor,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

#[derive(Debug)]
pub enum Event {
    Ready,
    Meter { level: f32, seconds: f32 },
    Finished(Result<Recording, String>),
}
#[derive(Debug)]
pub struct Recording {
    pub wav: Vec<u8>,
    pub duration_ms: u64,
}
pub struct Capture {
    stop: mpsc::Sender<bool>,
}
impl Capture {
    pub fn stop(&self) {
        let _ = self.stop.send(false);
    }
    pub fn cancel(&self) {
        let _ = self.stop.send(true);
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.cancel();
    }
}
struct Buffer {
    samples: Vec<f32>,
    rms: f32,
    voice_frames: usize,
    last_voice: usize,
    error: bool,
}

pub fn devices() -> Result<Vec<String>, String> {
    cpal::default_host()
        .input_devices()
        .map_err(|_| "Could not enumerate microphones.".to_string())?
        .map(|d| {
            d.name()
                .map_err(|_| "A microphone name is unavailable.".to_string())
        })
        .collect()
}
pub fn start(settings: Settings, events: mpsc::Sender<Event>) -> Capture {
    let (stop_tx, stop_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = capture(&settings, &events, stop_rx);
        let _ = events.send(Event::Finished(result));
    });
    Capture { stop: stop_tx }
}
fn capture(
    s: &Settings,
    events: &mpsc::Sender<Event>,
    stop: mpsc::Receiver<bool>,
) -> Result<Recording, String> {
    let host = cpal::default_host();
    let device = if s.microphone.is_empty() {
        host.default_input_device()
    } else {
        host.input_devices()
            .map_err(|_| "Cannot list microphones.")?
            .find(|d| d.name().is_ok_and(|n| n == s.microphone))
    }
    .ok_or("Selected microphone is unavailable. Reconnect it or choose another in Settings.")?;
    let config = device
        .default_input_config()
        .map_err(|_| "Microphone is unavailable. Check Windows microphone privacy settings.")?;
    let rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let max = (rate as usize)
        .checked_mul(s.max_seconds as usize)
        .ok_or("Recording limit is too large.")?;
    let data = Arc::new(Mutex::new(Buffer {
        samples: Vec::with_capacity(max),
        rms: 0.0,
        voice_frames: 0,
        last_voice: 0,
        error: false,
    }));
    let stream =
        match config.sample_format() {
            cpal::SampleFormat::F32 => stream::<f32>(
                &device,
                &config.clone().into(),
                data.clone(),
                channels,
                max,
                s.vad_threshold,
            ),
            cpal::SampleFormat::I16 => stream::<i16>(
                &device,
                &config.clone().into(),
                data.clone(),
                channels,
                max,
                s.vad_threshold,
            ),
            cpal::SampleFormat::U16 => stream::<u16>(
                &device,
                &config.clone().into(),
                data.clone(),
                channels,
                max,
                s.vad_threshold,
            ),
            _ => return Err(
                "Microphone sample format is unsupported. Select a standard Windows input device."
                    .into(),
            ),
        }
        .map_err(|_| {
            "Could not start microphone capture. Check device access and Windows privacy settings."
        })?;
    stream
        .play()
        .map_err(|_| "Could not activate the microphone.")?;
    let _ = events.send(Event::Ready);
    loop {
        match stop.recv_timeout(Duration::from_millis(50)) {
            Ok(true) | Err(mpsc::RecvTimeoutError::Disconnected) => return Err("Cancelled".into()),
            Ok(false) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let d = data.lock().map_err(|_| "Audio buffer unavailable.")?;
        if d.error {
            return Err(
                "Microphone disconnected or failed during capture. Nothing was sent.".into(),
            );
        }
        let frames = d.samples.len();
        let _ = events.send(Event::Meter {
            level: (d.rms * 8.0).min(1.0),
            seconds: frames as f32 / rate as f32,
        });
        if frames >= max
            || frames.saturating_sub(d.last_voice) > rate as usize * s.silence_seconds as usize
        {
            break;
        }
    }
    drop(stream);
    let mut d = data.lock().map_err(|_| "Audio buffer unavailable.")?;
    if d.error {
        return Err("Microphone failed. Nothing was sent.".into());
    }
    if d.voice_frames < (rate as usize * 15 / 100) {
        return Err(
            "No speech detected. Check your microphone or adjust voice sensitivity.".into(),
        );
    }
    let duration_ms = d.samples.len() as u64 * 1000 / rate as u64;
    let samples = std::mem::take(&mut d.samples);
    drop(d);
    let mono = resample(&samples, rate);
    let mut cursor = Cursor::new(Vec::with_capacity(mono.len() * 2 + 44));
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer =
            hound::WavWriter::new(&mut cursor, spec).map_err(|_| "Could not encode audio.")?;
        for v in mono {
            writer
                .write_sample((v.clamp(-1.0, 1.0) * 32767.0) as i16)
                .map_err(|_| "Could not encode audio.")?;
        }
        writer
            .finalize()
            .map_err(|_| "Could not finish audio encoding.")?;
    }
    Ok(Recording {
        wav: cursor.into_inner(),
        duration_ms,
    })
}
fn stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    data: Arc<Mutex<Buffer>>,
    channels: usize,
    max: usize,
    threshold: f32,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let errors = data.clone();
    device.build_input_stream(
        config,
        move |input: &[T], _| {
            if let Ok(mut d) = data.lock() {
                let mut power = 0.0;
                let start = d.samples.len();
                for frame in input.chunks_exact(channels) {
                    if d.samples.len() >= max {
                        break;
                    }
                    let value =
                        frame.iter().map(|v| v.to_sample::<f32>()).sum::<f32>() / channels as f32;
                    let value = if value.is_finite() { value } else { 0.0 };
                    power += value * value;
                    d.samples.push(value);
                }
                let count = d.samples.len() - start;
                d.rms = if count == 0 {
                    0.0
                } else {
                    (power / count as f32).sqrt()
                };
                if d.rms >= threshold {
                    d.voice_frames += count;
                    d.last_voice = d.samples.len();
                }
            }
        },
        move |_| {
            if let Ok(mut d) = errors.lock() {
                d.error = true;
            }
        },
        None,
    )
}
/// A 32-tap Hann-windowed sinc low-pass avoids naive-decimation aliasing.
fn resample(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == 16000 {
        return samples.to_vec();
    }
    let ratio = rate as f64 / 16000.0;
    let cutoff = (16000.0 / rate as f64).min(1.0) * 0.94;
    let len = (samples.len() as f64 / ratio).floor() as usize;
    (0..len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let center = pos.floor() as isize;
            let mut sum = 0.0;
            let mut weights = 0.0;
            for offset in -15isize..=16 {
                let index = center + offset;
                if index < 0 || index >= samples.len() as isize {
                    continue;
                }
                let x = index as f64 - pos;
                let z = std::f64::consts::PI * x * cutoff;
                let sinc = if z.abs() < 1e-9 { 1.0 } else { z.sin() / z };
                let window = 0.5 + 0.5 * (std::f64::consts::PI * x / 16.0).cos();
                let w = sinc * window * cutoff;
                sum += samples[index as usize] as f64 * w;
                weights += w;
            }
            if weights.abs() < 1e-9 {
                0.0
            } else {
                (sum / weights) as f32
            }
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resampler_keeps_duration_and_silence() {
        let v = resample(&vec![0.0; 48000], 48000);
        assert_eq!(v.len(), 16000);
        assert!(v.iter().all(|v| *v == 0.0));
    }
    #[test]
    fn resampler_preserves_dc() {
        let v = resample(&vec![0.4; 44100], 44100);
        assert_eq!(v.len(), 16000);
        assert!((v[8000] - 0.4).abs() < 0.001);
    }
    #[test]
    fn attenuates_above_nyquist() {
        let v: Vec<f32> = (0..48000)
            .map(|i| (i as f64 * 2.0 * std::f64::consts::PI * 12000.0 / 48000.0).sin() as f32)
            .collect();
        let r = resample(&v, 48000);
        let rms = (r[100..15000].iter().map(|v| v * v).sum::<f32>() / 14900.0).sqrt();
        assert!(rms < 0.1, "{rms}");
    }
}
