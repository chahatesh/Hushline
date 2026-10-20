// SPDX-License-Identifier: MIT
//! One thread owns the stopped, reusable WASAPI client. Start/Stop/Reset are
//! synchronous, so a new recording cannot inherit samples from the previous one.
use windows::{
    core::GUID,
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Media::Audio::*,
        System::Com::{
            StructuredStorage::{PropVariantClear, PropVariantToStringAlloc},
            *,
        },
    },
};

pub struct Apartment;
impl Apartment {
    pub fn enter() -> Result<Self, String> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }
            .map_err(|_| "Windows audio could not be initialized.")?;
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}

pub struct Stream {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    pub rate: u32,
    channels: usize,
    bytes: usize,
    float: bool,
    running: bool,
    seen_packet: bool,
    next_position: Option<u64>,
    pub gaps: u32,
    selection: String,
    device_id: String,
    enumerator: IMMDeviceEnumerator,
}
impl Stream {
    pub fn open(name: &str) -> Result<Self, String> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|_| "Windows could not list your microphones.")?;
            let device = if name.is_empty() {
                enumerator.GetDefaultAudioEndpoint(eCapture, eConsole).ok()
            } else {
                let devices = enumerator
                    .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
                    .map_err(|_| "Windows could not list your microphones.")?;
                let mut found = None;
                for i in 0..devices.GetCount().unwrap_or(0) {
                    let device = devices.Item(i).map_err(|_| "Microphone is unavailable.")?;
                    if friendly_name(&device).as_deref() == Some(name) {
                        found = Some(device);
                        break;
                    }
                }
                found
            }
            .ok_or("Your microphone is unavailable. Reconnect it or choose another in Settings.")?;
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(|_| {
                "Cannot access your microphone. Check Windows microphone permissions."
            })?;
            let format = client
                .GetMixFormat()
                .map_err(|_| "Cannot read your microphone's audio format.")?;
            let value = format.read_unaligned();
            let subtype = if value.wFormatTag == 0xfffe && value.cbSize >= 22 {
                (*(format as *const WAVEFORMATEXTENSIBLE)).SubFormat
            } else {
                GUID::from_u128(if value.wFormatTag == 3 {
                    0x00000003_0000_0010_8000_00aa00389b71
                } else if value.wFormatTag == 1 {
                    0x00000001_0000_0010_8000_00aa00389b71
                } else {
                    0
                })
            };
            let float = subtype == GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);
            let pcm = subtype == GUID::from_u128(0x00000001_0000_0010_8000_00aa00389b71);
            let bytes = value.wBitsPerSample as usize / 8;
            let valid = (float && bytes == 4 || pcm && matches!(bytes, 2..=4))
                && value.nChannels > 0
                && value.nChannels <= 32
                && value.nSamplesPerSec >= 8000
                && value.nSamplesPerSec <= 384000
                && value.nBlockAlign as usize == bytes * value.nChannels as usize;
            // Endpoint capacity; available packets are drained every 20 ms.
            let initialized = if valid {
                client.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, 2_000_000, 0, format, None)
            } else {
                Err(windows::core::Error::from_hresult(
                    windows::Win32::Foundation::E_INVALIDARG,
                ))
            };
            CoTaskMemFree(Some(format as *const _));
            initialized.map_err(|_| {
                "Could not prepare this microphone. Try another Windows input device."
            })?;
            let capture = client
                .GetService::<IAudioCaptureClient>()
                .map_err(|_| "Cannot read microphone audio.")?;
            let device_id = device_id(&device).ok_or("Cannot identify the selected microphone.")?;
            Ok(Self {
                client,
                capture,
                rate: value.nSamplesPerSec,
                channels: value.nChannels as usize,
                bytes,
                float,
                running: false,
                seen_packet: false,
                next_position: None,
                gaps: 0,
                selection: name.to_string(),
                device_id,
                enumerator,
            })
        }
    }
    pub fn start(&mut self) -> Result<(), String> {
        unsafe { self.client.Reset().and_then(|_| self.client.Start()) }.map_err(|_| {
            "Your microphone could not start. Check the device and Windows permissions."
        })?;
        self.running = true;
        self.seen_packet = false;
        self.next_position = None;
        self.gaps = 0;
        Ok(())
    }
    pub fn matches_selection(&self, name: &str) -> bool {
        if self.selection != name {
            return false;
        }
        if !name.is_empty() {
            return true;
        }
        unsafe {
            self.enumerator
                .GetDefaultAudioEndpoint(eCapture, eConsole)
                .ok()
                .and_then(|d| device_id(&d))
        }
        .as_ref()
            == Some(&self.device_id)
    }
    pub fn stop(&mut self) -> Result<(), String> {
        if self.running {
            unsafe { self.client.Stop() }
                .map_err(|_| "Your microphone disconnected during recording.")?;
            self.running = false;
        }
        Ok(())
    }
    pub fn read(&mut self, mut append: impl FnMut(f32)) -> Result<(), String> {
        unsafe {
            while self
                .capture
                .GetNextPacketSize()
                .map_err(|_| "Your microphone disconnected. Nothing was sent.")?
                > 0
            {
                let (mut pointer, mut frames, mut flags) = (std::ptr::null_mut(), 0, 0);
                let mut position = 0;
                self.capture
                    .GetBuffer(
                        &mut pointer,
                        &mut frames,
                        &mut flags,
                        Some(&mut position),
                        None,
                    )
                    .map_err(|_| "Could not read microphone audio. Nothing was sent.")?;
                // Preserve the remaining audio after a glitch, but mark the
                // recording for mandatory review. Never present it as complete.
                let gap = flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0;
                let valid_time = flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 == 0;
                let missing = if valid_time {
                    self.next_position
                        .map(|p| position.saturating_sub(p))
                        .unwrap_or(0)
                } else {
                    0
                };
                if self.seen_packet && (gap || missing > 0) {
                    self.gaps = self.gaps.saturating_add(1);
                    // Silence preserves the time boundary rather than joining
                    // words across missing audio. Large gaps remain bounded.
                    for _ in 0..missing.min(self.rate as u64 * 2) {
                        append(0.0);
                    }
                }
                self.next_position = if valid_time {
                    Some(position + frames as u64)
                } else {
                    None
                };
                for frame in 0..frames as usize {
                    let value =
                        if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || pointer.is_null() {
                            0.0
                        } else {
                            let data = std::slice::from_raw_parts(
                                pointer.add(frame * self.channels * self.bytes),
                                self.channels * self.bytes,
                            );
                            data.chunks_exact(self.bytes)
                                .map(|v| decode(v, self.float))
                                .sum::<f32>()
                                / self.channels as f32
                        };
                    append(if value.is_finite() {
                        value.clamp(-1.0, 1.0)
                    } else {
                        0.0
                    });
                }
                self.capture
                    .ReleaseBuffer(frames)
                    .map_err(|_| "Could not finish reading microphone audio.")?;
                self.seen_packet = true;
            }
        }
        Ok(())
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
unsafe fn friendly_name(device: &IMMDevice) -> Option<String> {
    let properties = device.OpenPropertyStore(STGM_READ).ok()?;
    let mut value = properties.GetValue(&PKEY_Device_FriendlyName).ok()?;
    let string = PropVariantToStringAlloc(&value).ok();
    let result = string.and_then(|s| {
        let result = s.to_string().ok();
        CoTaskMemFree(Some(s.0 as *const _));
        result
    });
    let _ = PropVariantClear(&mut value);
    result
}
unsafe fn device_id(device: &IMMDevice) -> Option<String> {
    let id = device.GetId().ok()?;
    let result = id.to_string().ok();
    CoTaskMemFree(Some(id.0 as *const _));
    result
}
fn decode(bytes: &[u8], float: bool) -> f32 {
    if float {
        return f32::from_le_bytes(bytes.try_into().unwrap());
    }
    match bytes.len() {
        2 => i16::from_le_bytes(bytes.try_into().unwrap()) as f32 / 32768.0,
        3 => {
            ((bytes[0] as i32 | (bytes[1] as i32) << 8 | (bytes[2] as i32) << 16) << 8 >> 8) as f32
                / 8_388_608.0
        }
        4 => i32::from_le_bytes(bytes.try_into().unwrap()) as f32 / 2_147_483_648.0,
        _ => 0.0,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Pauses the owned real-microphone reader briefly to check scheduling headroom; no upload or audio file."]
    fn endpoint_reports_a_scheduler_overrun_instead_of_silent_loss() {
        let _com = Apartment::enter().unwrap();
        let mut stream = Stream::open("").unwrap();
        let buffer = unsafe { stream.client.GetBufferSize().unwrap() };
        println!(
            "Endpoint buffer: {} ms",
            buffer as u64 * 1000 / stream.rate as u64
        );
        stream.start().unwrap();
        let mut frames = 0;
        let started = std::time::Instant::now();
        while frames == 0 && started.elapsed() < std::time::Duration::from_secs(3) {
            std::thread::sleep(std::time::Duration::from_millis(10));
            stream.read(|_| frames += 1).unwrap();
        }
        assert!(frames > 0);
        std::thread::sleep(std::time::Duration::from_millis(300));
        let mut retained = 0;
        let result = stream.read(|_| retained += 1);
        stream.stop().unwrap();
        assert!(result.is_ok(), "An audio glitch ended capture: {result:?}");
        assert!(stream.gaps > 0, "Forced overrun was not marked for review");
        assert!(
            retained >= stream.rate as usize / 4,
            "Remaining audio/time boundary was lost"
        );
        println!("Forced overrun marked for review: {}; retained frames including marked silence: {retained}",stream.gaps);
    }
    #[test]
    fn decodes_native_formats() {
        assert_eq!(decode(&[0, 128], false), -1.0);
        assert_eq!(decode(&[0, 0, 128], false), -1.0);
        assert_eq!(decode(&[0, 0, 0, 128], false), -1.0);
        assert_eq!(decode(&0.5f32.to_le_bytes(), true), 0.5);
    }
    #[test]
    #[ignore = "Uses the real microphone briefly, with no upload or audio file."]
    fn prepared_wasapi_starts_and_resets() {
        let _com = Apartment::enter().unwrap();
        let cold = std::time::Instant::now();
        let mut stream = Stream::open("").unwrap();
        println!(
            "Stopped stream prepared in {} ms",
            cold.elapsed().as_millis()
        );
        for _ in 0..3 {
            let started = std::time::Instant::now();
            stream.start().unwrap();
            println!(
                "Prepared stream Start: {} us",
                started.elapsed().as_micros()
            );
            let mut frames = 0;
            while frames == 0 && started.elapsed() < std::time::Duration::from_secs(3) {
                std::thread::sleep(std::time::Duration::from_millis(5));
                stream.read(|_| frames += 1).unwrap();
            }
            println!(
                "First audio frames: {} ms; {} samples",
                started.elapsed().as_millis(),
                frames
            );
            assert!(frames > 0);
            stream.stop().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(40));
            unsafe {
                stream.client.Reset().unwrap();
            }
            assert_eq!(
                unsafe { stream.capture.GetNextPacketSize().unwrap() },
                0,
                "Stopped stream retained unread samples"
            );
        }
    }
}
