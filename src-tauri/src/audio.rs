// SPDX-License-Identifier: MIT
//! A stopped WASAPI client is prepared asynchronously. No idle audio is read.
//! The worker blocks on commands while idle; Start/Stop/Reset happen on its thread.
use crate::model::Settings;
use cpal::traits::{DeviceTrait, HostTrait};
use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
mod wasapi;
const READ_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Debug)]
pub enum Event {
    Ready,
    Warning,
    Meter { level: f32, seconds: f32 },
    Finished(Result<Recording, String>),
}
#[derive(Debug, Clone)]
pub struct Recording {
    pub wav: Vec<u8>,
    pub duration_ms: u64,
    pub limit_reached: bool,
    pub encoding_ms: u64,
    pub requires_review: bool,
}
pub struct Engine {
    tx: mpsc::Sender<Request>,
    next: AtomicU64,
}
pub struct Capture {
    tx: mpsc::Sender<Request>,
    id: u64,
    cancelled: Arc<AtomicBool>,
    silence: Arc<AtomicBool>,
}
impl Capture {
    pub fn stop_on_silence(&self, enabled: bool) {
        self.silence.store(enabled, Ordering::Relaxed);
    }
    pub fn stop(&self) {
        let _ = self.tx.send(Request::Stop(self.id, false));
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.tx.send(Request::Stop(self.id, true));
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.cancel();
    }
}
struct Start {
    id: u64,
    settings: Settings,
    events: mpsc::Sender<Event>,
    cancelled: Arc<AtomicBool>,
    silence: Arc<AtomicBool>,
}
enum Request {
    Configure(Settings),
    Start(Start),
    Stop(u64, bool),
    #[cfg(test)]
    Barrier(mpsc::Sender<()>),
}
struct Active {
    request: Start,
    samples: Vec<f32>,
    rate: u32,
    max: usize,
    rms: f32,
    voice_frames: usize,
    last_voice: usize,
    started: Instant,
    last_packet: Instant,
    limit_reached: bool,
    requires_review: bool,
}
impl Active {
    fn read(&mut self, stream: &mut wasapi::Stream) -> Result<(), String> {
        let start = self.samples.len();
        let mut power = 0.0;
        stream.read(|value| {
            if self.samples.len() < self.max {
                self.samples.push(value);
                power += value * value;
            }
        })?;
        if !self.requires_review && stream.requires_review() {
            self.requires_review = true;
            let _ = self.request.events.send(Event::Warning);
        }
        let count = self.samples.len() - start;
        if count > 0 {
            self.last_packet = Instant::now();
            self.rms = (power / count as f32).sqrt();
            if self.rms >= self.request.settings.vad_threshold {
                self.voice_frames += count;
                self.last_voice = self.samples.len();
            }
        }
        Ok(())
    }
}
impl Engine {
    pub fn new(settings: Settings) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("hushline-audio".into())
            .spawn(move || worker(rx))
            .expect("audio worker thread");
        let engine = Self {
            tx,
            next: AtomicU64::new(1),
        };
        engine.configure(settings);
        engine
    }
    pub fn configure(&self, settings: Settings) {
        let _ = self.tx.send(Request::Configure(settings));
    }
    pub fn start(&self, settings: Settings, events: mpsc::Sender<Event>) -> Capture {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let cancelled = Arc::new(AtomicBool::new(false));
        let silence = Arc::new(AtomicBool::new(false));
        let request = Start {
            id,
            settings,
            events: events.clone(),
            cancelled: cancelled.clone(),
            silence: silence.clone(),
        };
        if self.tx.send(Request::Start(request)).is_err() {
            let _ = events.send(Event::Finished(Err(
                "Microphone worker is unavailable. Restart Hush.".into(),
            )));
        }
        Capture {
            tx: self.tx.clone(),
            id,
            cancelled,
            silence,
        }
    }
}
#[cfg(test)]
fn start(settings: Settings, events: mpsc::Sender<Event>) -> Capture {
    let engine = Engine::new(Settings {
        prepare_microphone: false,
        ..settings.clone()
    });
    engine.start(settings, events)
}
pub fn devices() -> Result<Vec<String>, String> {
    cpal::default_host()
        .input_devices()
        .map_err(|_| "Could not list microphones.".to_string())?
        .map(|d| {
            d.name()
                .map_err(|_| "A microphone name is unavailable.".to_string())
        })
        .collect()
}
fn begin(request: &Start, stream: &mut Option<wasapi::Stream>) -> Result<Active, String> {
    if request.cancelled.load(Ordering::Acquire) {
        return Err("Cancelled".into());
    }
    if stream
        .as_ref()
        .is_some_and(|s| !s.matches_selection(&request.settings.microphone))
    {
        *stream = None;
    }
    if stream.is_none() {
        *stream = Some(wasapi::Stream::open(&request.settings.microphone)?);
    }
    let device = stream.as_mut().unwrap();
    let rate = device.rate;
    let max = (rate as usize)
        .checked_mul(request.settings.max_seconds as usize)
        .ok_or("Recording limit is too large.")?;
    let mut samples = Vec::new();
    samples.try_reserve_exact(max).map_err(|_| {
        "There isn't enough memory for this recording limit. Choose a shorter limit in Settings."
    })?;
    if request.cancelled.load(Ordering::Acquire) {
        return Err("Cancelled".into());
    }
    device.start()?;
    Ok(Active {
        request: Start {
            id: request.id,
            settings: request.settings.clone(),
            events: request.events.clone(),
            cancelled: request.cancelled.clone(),
            silence: request.silence.clone(),
        },
        samples,
        rate,
        max,
        rms: 0.0,
        voice_frames: 0,
        last_voice: 0,
        started: Instant::now(),
        last_packet: Instant::now(),
        limit_reached: false,
        requires_review: false,
    })
}
fn finish(
    mut active: Active,
    stream: &mut Option<wasapi::Stream>,
    cancel: bool,
    error: Option<String>,
) {
    let stopped = stream.as_mut().map(|s| s.stop());
    let tail = if !cancel && error.is_none() {
        stream.as_mut().map(|s| active.read(s))
    } else {
        None
    };
    let failure = error
        .or_else(|| stopped.and_then(Result::err))
        .or_else(|| tail.and_then(Result::err));
    if failure.is_some() {
        *stream = None;
    }
    if cancel || active.request.cancelled.load(Ordering::Acquire) {
        let _ = active
            .request
            .events
            .send(Event::Finished(Err("Cancelled".into())));
        return;
    }
    if let Some(error) = failure {
        let _ = active.request.events.send(Event::Finished(Err(error)));
        return;
    }
    // Encoding is separate from ownership of the prepared endpoint: another
    // shortcut can start capture immediately without waiting for this work.
    std::thread::spawn(move || {
        let result = encode(&active);
        let _ = active.request.events.send(Event::Finished(result));
    });
}
fn worker(rx: mpsc::Receiver<Request>) {
    let apartment = wasapi::Apartment::enter();
    let mut configured = Settings::default();
    let mut stream = None;
    let mut active: Option<Active> = None;
    loop {
        let command = if active.is_some() {
            rx.recv_timeout(READ_INTERVAL)
        } else {
            rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        match command {
            Ok(Request::Configure(settings)) => {
                let changed = settings.microphone != configured.microphone;
                configured = settings;
                if active.is_none() {
                    if changed || !configured.prepare_microphone {
                        stream = None;
                    }
                    if configured.prepare_microphone && stream.is_none() && apartment.is_ok() {
                        stream = wasapi::Stream::open(&configured.microphone).ok();
                    }
                }
            }
            Ok(Request::Start(request)) => {
                if let Some(previous) = active.take() {
                    finish(previous, &mut stream, true, None);
                }
                if request.settings.microphone != configured.microphone {
                    stream = None;
                }
                configured = request.settings.clone();
                let result = if let Err(error) = &apartment {
                    Err(error.clone())
                } else {
                    begin(&request, &mut stream)
                };
                match result {
                    Ok(recording) => {
                        let _ = request.events.send(Event::Ready);
                        active = Some(recording);
                    }
                    Err(error) => {
                        stream = None;
                        let _ = request.events.send(Event::Finished(Err(error)));
                    }
                }
            }
            Ok(Request::Stop(id, cancel)) => {
                if active.as_ref().is_some_and(|v| v.request.id == id) {
                    finish(active.take().unwrap(), &mut stream, cancel, None);
                    if !configured.prepare_microphone {
                        stream = None;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(recording) = active.take() {
                    finish(recording, &mut stream, true, None);
                }
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            #[cfg(test)]
            Ok(Request::Barrier(tx)) => {
                let _ = tx.send(());
            }
        }
        if let Some(recording) = active.as_mut() {
            let mut error = stream
                .as_mut()
                .ok_or("Microphone is unavailable.".to_string())
                .and_then(|s| recording.read(s))
                .err();
            let cancel = recording.request.cancelled.load(Ordering::Acquire);
            let frames = recording.samples.len();
            if recording.last_packet.elapsed() > Duration::from_secs(2) {
                error=Some("Your microphone stopped sending audio. Reconnect it or choose another device. Nothing was sent.".into());
            }
            recording.limit_reached = frames >= recording.max
                || recording.started.elapsed()
                    >= Duration::from_secs(recording.request.settings.max_seconds as u64);
            let stopped = recording.limit_reached
                || silence_expired(
                    frames,
                    recording.last_voice,
                    recording.rate,
                    recording.request.settings.silence_seconds,
                    recording.request.silence.load(Ordering::Relaxed),
                );
            let _ = recording.request.events.send(Event::Meter {
                level: (recording.rms * 8.0).min(1.0),
                seconds: frames as f32 / recording.rate as f32,
            });
            if cancel || stopped || error.is_some() {
                let failed = error.is_some();
                finish(active.take().unwrap(), &mut stream, cancel, error);
                if failed || !configured.prepare_microphone {
                    stream = None;
                }
            }
        }
    }
}
fn encode(active: &Active) -> Result<Recording, String> {
    if active.voice_frames < active.rate as usize * 15 / 100 {
        return Err(
            "I couldn't hear any speech. Check your microphone or adjust voice sensitivity.".into(),
        );
    }
    let started = Instant::now();
    let mono = resample(&active.samples, active.rate);
    let mut cursor = Cursor::new(Vec::with_capacity(mono.len() * 2 + 44));
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|_| "Could not prepare your recording.")?;
        for value in mono {
            writer
                .write_sample((value.clamp(-1.0, 1.0) * 32767.0) as i16)
                .map_err(|_| "Could not prepare your recording.")?;
        }
        writer
            .finalize()
            .map_err(|_| "Could not finish your recording.")?;
    }
    Ok(Recording {
        wav: cursor.into_inner(),
        duration_ms: active.samples.len() as u64 * 1000 / active.rate as u64,
        limit_reached: active.limit_reached || active.samples.len() >= active.max,
        encoding_ms: started.elapsed().as_millis() as u64,
        requires_review: active.requires_review,
    })
}
fn silence_expired(
    frames: usize,
    last_voice: usize,
    rate: u32,
    seconds: u32,
    hands_free: bool,
) -> bool {
    hands_free
        && seconds > 0
        && frames.saturating_sub(last_voice) > rate as usize * seconds as usize
}
/// A 32-tap Hann-windowed sinc low-pass avoids naive-decimation aliasing.
fn resample(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == 16000 {
        return samples.to_vec();
    }
    let ratio = rate as f64 / 16000.0;
    let cutoff = (16000.0 / rate as f64).min(1.0) * 0.94;
    let len = (samples.len() as f64 / ratio).floor() as usize;
    // Precompute phases once per recording instead of doing 32 sine/cosine calls
    // for every output sample. This bounds filter work in the release hot path.
    let phases: Vec<[f32; 32]> = (0..256)
        .map(|phase| {
            let mut row = [0.0f32; 32];
            let fraction = phase as f64 / 256.0;
            for (tap, w) in row.iter_mut().enumerate() {
                let x = tap as f64 - 15.0 - fraction;
                let z = std::f64::consts::PI * x * cutoff;
                let sinc = if z.abs() < 1e-9 { 1.0 } else { z.sin() / z };
                let window = 0.5 + 0.5 * (std::f64::consts::PI * x / 16.0).cos();
                *w = (sinc * window * cutoff) as f32;
            }
            let total = row.iter().sum::<f32>();
            for w in &mut row {
                *w /= total;
            }
            row
        })
        .collect();
    (0..len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let center = pos.floor() as isize;
            let phase = ((pos - center as f64) * 256.0).floor().clamp(0.0, 255.0) as usize;
            let row = &phases[phase];
            if center >= 15 && center + 16 < samples.len() as isize {
                return samples[(center - 15) as usize..(center + 17) as usize]
                    .iter()
                    .zip(row)
                    .map(|(sample, w)| sample * w)
                    .sum();
            }
            let mut sum = 0.0;
            let mut weights = 0.0;
            for offset in -15isize..=16 {
                let index = center + offset;
                if index < 0 || index >= samples.len() as isize {
                    continue;
                }
                let w = row[(offset + 15) as usize];
                sum += samples[index as usize] * w;
                weights += w;
            }
            if weights.abs() < 1e-9 {
                0.0
            } else {
                sum / weights
            }
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Tests cold first use and normal microphone Stop locally; no upload or saved audio."]
    fn cold_first_use_and_normal_stop() {
        let settings = Settings::default();
        let engine = Engine::new(settings.clone());
        for attempt in 0..4 {
            let (tx, rx) = mpsc::channel();
            let capture = engine.start(settings.clone(), tx);
            let started = Instant::now();
            let mut stopped = false;
            loop {
                match rx.recv_timeout(Duration::from_secs(12)).unwrap() {
                    Event::Ready => println!(
                        "Attempt {attempt} Ready {} ms",
                        started.elapsed().as_millis()
                    ),
                    Event::Meter { seconds, .. } if seconds >= 1.0 && !stopped => {
                        capture.stop();
                        stopped = true;
                    }
                    Event::Finished(result) => {
                        assert!(stopped, "Ended before Stop: {:?}", result.as_ref().err());
                        match result {
                            Ok(audio) => assert!(audio.duration_ms >= 900),
                            Err(e) => assert!(e.contains("couldn't hear any speech"), "{e}"),
                        }
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
    #[test]
    fn cancellation_before_capture_never_opens_a_stream() {
        let (events, event_rx) = mpsc::channel();
        let request = Start {
            id: 1,
            settings: Settings::default(),
            events,
            cancelled: Arc::new(AtomicBool::new(true)),
            silence: Arc::new(AtomicBool::new(false)),
        };
        let mut stream = None;
        assert_eq!(begin(&request, &mut stream).err().unwrap(), "Cancelled");
        assert!(stream.is_none());
        assert!(event_rx.try_recv().is_err());
    }
    #[test]
    #[ignore = "Uses the actual default microphone briefly; no audio is uploaded or saved."]
    fn microphone_start_and_cancel_without_upload() {
        let inputs = devices().expect("microphone enumeration");
        assert!(!inputs.is_empty(), "No input devices available");
        let settings = Settings {
            max_seconds: 5,
            silence_seconds: 2,
            ..Settings::default()
        };
        let (tx, rx) = mpsc::channel();
        let started = std::time::Instant::now();
        let capture = start(settings, tx);
        match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::Ready => println!(
                "Microphone activation to stream-ready: {} ms",
                started.elapsed().as_millis()
            ),
            other => panic!("Microphone failed before activation: {other:?}"),
        }
        let mut metered = false;
        for _ in 0..5 {
            if let Event::Meter { seconds, .. } = rx.recv_timeout(Duration::from_secs(2)).unwrap() {
                if seconds > 0.0 {
                    metered = true;
                    break;
                }
            }
        }
        assert!(metered, "No audio frames arrived");
        capture.cancel();
        loop {
            match rx.recv_timeout(Duration::from_secs(2)).unwrap() {
                Event::Finished(Err(e)) => {
                    assert_eq!(e, "Cancelled");
                    break;
                }
                Event::Finished(Ok(_)) => panic!("Cancellation unexpectedly completed a recording"),
                _ => {}
            }
        }
    }
    #[test]
    fn push_to_talk_does_not_stop_during_a_pause() {
        assert!(!silence_expired(160000, 0, 16000, 8, false));
        assert!(silence_expired(160000, 0, 16000, 8, true));
        assert!(!silence_expired(160000, 0, 16000, 0, true));
    }
    #[test]
    #[ignore = "Prepares the actual microphone and tests three captures, without upload or audio files."]
    fn prepared_worker_first_frames_and_cancellation() {
        let settings = Settings::default();
        let engine = Engine::new(settings.clone());
        let (tx, rx) = mpsc::channel();
        engine.tx.send(Request::Barrier(tx)).unwrap();
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        for _ in 0..3 {
            let (tx, rx) = mpsc::channel();
            let started = Instant::now();
            let capture = engine.start(settings.clone(), tx);
            loop {
                match rx.recv_timeout(Duration::from_secs(3)).unwrap() {
                    Event::Ready => println!(
                        "Prepared worker Ready: {} ms",
                        started.elapsed().as_millis()
                    ),
                    Event::Meter { seconds, .. } if seconds > 0.0 => {
                        println!(
                            "Prepared worker first frames: {} ms",
                            started.elapsed().as_millis()
                        );
                        break;
                    }
                    Event::Finished(result) => panic!("Capture ended before frames: {result:?}"),
                    _ => {}
                }
            }
            capture.cancel();
            loop {
                if let Event::Finished(result) = rx.recv_timeout(Duration::from_secs(3)).unwrap() {
                    assert_eq!(result.unwrap_err(), "Cancelled");
                    break;
                }
            }
        }
    }
    #[test]
    fn recordings_longer_than_two_minutes_keep_their_tail() {
        let rate = 16000;
        let mut samples = vec![0.2; rate * 125];
        *samples.last_mut().unwrap() = 0.7;
        let (events, _) = mpsc::channel();
        let active = Active {
            request: Start {
                id: 1,
                settings: Settings::default(),
                events,
                cancelled: Arc::new(AtomicBool::new(false)),
                silence: Arc::new(AtomicBool::new(false)),
            },
            samples,
            rate: rate as u32,
            max: rate * 600,
            rms: 0.2,
            voice_frames: rate * 125,
            last_voice: rate * 125,
            started: Instant::now(),
            last_packet: Instant::now(),
            limit_reached: false,
            requires_review: false,
        };
        let result = encode(&active).unwrap();
        assert_eq!(result.duration_ms, 125000);
        assert!(!result.limit_reached);
        let reader = hound::WavReader::new(Cursor::new(result.wav)).unwrap();
        assert_eq!(reader.duration(), (rate * 125) as u32);
        assert_eq!(
            reader.into_samples::<i16>().last().unwrap().unwrap(),
            (0.7 * 32767.0) as i16
        );
    }
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
