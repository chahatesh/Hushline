// SPDX-License-Identifier: MIT
//! Recording history: per-user DPAPI on disk, bounded memory in private mode.
use crate::{audio::Recording, store::Store};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::Cryptography::*,
    },
};
use zeroize::Zeroize;

pub const MAX_CLIPS: usize = 1000;
pub const MAX_STORED_BYTES: usize = 512 * 1024 * 1024;
const MAX_MEMORY_CLIPS: usize = 3;
const MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;
const MAX_AUDIO: usize = 24_000_000;
fn failed_status() -> String {
    "failed".into()
}
fn one_attempt() -> u32 {
    1
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAudio {
    pub id: String,
    pub created_at: String,
    pub duration_ms: u64,
    pub requires_review: bool,
    pub in_memory: bool,
    #[serde(default = "failed_status")]
    pub status: String,
    #[serde(default)]
    pub transcript_id: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default = "one_attempt")]
    pub attempts: u32,
}
struct MemoryClip(SavedAudio, Recording);
impl Drop for MemoryClip {
    fn drop(&mut self) {
        self.1.wav.zeroize();
    }
}
#[derive(Default)]
pub struct Recovery(Mutex<Vec<MemoryClip>>);
impl Recovery {
    pub fn save(&self, store: &Store, recording: Recording, persist: bool) -> SavedAudio {
        let mut info = SavedAudio {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            duration_ms: recording.duration_ms,
            requires_review: recording.requires_review,
            in_memory: false,
            status: "pending".into(),
            transcript_id: None,
            last_error: None,
            attempts: 1,
        };
        if persist
            && seal(&recording.wav)
                .and_then(|data| store.save_audio(&info, &data))
                .is_ok()
        {
            self.trim(store);
            return info;
        }
        info.in_memory = true;
        let mut clips = self.0.lock().unwrap();
        clips.push(MemoryClip(info.clone(), recording));
        while clips.len() > MAX_MEMORY_CLIPS
            || clips.iter().map(|c| c.1.wav.len()).sum::<usize>() > MAX_MEMORY_BYTES
        {
            clips.remove(0);
        }
        drop(clips);
        self.trim(store);
        info
    }
    fn trim(&self, store: &Store) {
        if let Ok(clips) = self.list(store) {
            for old in clips.into_iter().skip(MAX_CLIPS) {
                let _ = self.delete(store, &old.id);
            }
        }
    }
    pub fn list(&self, store: &Store) -> Result<Vec<SavedAudio>, String> {
        let persisted = store.saved_audios();
        let cutoff = crate::store::audio_cutoff();
        let mut memory = self.0.lock().unwrap();
        memory.retain(|clip| clip.0.created_at >= cutoff);
        let mut result = match persisted {
            Ok(clips) => clips,
            Err(error) if memory.is_empty() => return Err(error),
            Err(_) => Vec::new(), // A storage failure must not hide memory recovery.
        };
        result.extend(memory.iter().map(|clip| clip.0.clone()));
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(result)
    }
    pub fn load(&self, store: &Store, id: &str) -> Result<Recording, String> {
        {
            let cutoff = crate::store::audio_cutoff();
            let mut clips = self.0.lock().unwrap();
            clips.retain(|clip| clip.0.created_at >= cutoff);
            if let Some(clip) = clips.iter().find(|clip| clip.0.id == id) {
                return Ok(clip.1.clone());
            }
        }
        let (info, data) = store.saved_audio(id)?;
        Ok(Recording {
            wav: unseal(&data)?,
            duration_ms: info.duration_ms,
            requires_review: info.requires_review,
            speech_detected: true, // Explicit retry may override conservative VAD.
            limit_reached: false,
            encoding_ms: 0,
        })
    }
    pub fn delete(&self, store: &Store, id: &str) -> Result<(), String> {
        self.0.lock().unwrap().retain(|clip| clip.0.id != id);
        store.delete_audio(id)
    }
    pub fn update(
        &self,
        store: &Store,
        id: &str,
        change: impl FnOnce(&mut SavedAudio),
    ) -> Result<(), String> {
        let mut memory = self.0.lock().map_err(|_| "Audio storage unavailable")?;
        if let Some(clip) = memory.iter_mut().find(|c| c.0.id == id) {
            change(&mut clip.0);
            return Ok(());
        }
        drop(memory);
        // Update metadata only. A deleted/expired clip must never be recreated.
        store.update_audio(id, change)
    }
    pub fn begin_retry(&self, store: &Store, id: &str) -> Result<(), String> {
        self.update(store, id, |info| {
            info.status = "pending".into();
            info.attempts = info.attempts.saturating_add(1);
            info.last_error = None;
        })
    }
    pub fn finish_attempt(
        &self,
        store: &Store,
        id: &str,
        error: Option<&str>,
    ) -> Result<(), String> {
        self.update(store, id, |info| {
            let cancelled_existing = error == Some("Cancelled") && info.transcript_id.is_some();
            info.status = if error.is_none() || cancelled_existing {
                "ready"
            } else {
                "failed"
            }
            .into();
            info.last_error = if cancelled_existing {
                None
            } else {
                error.map(failure_reason)
            };
        })
    }
    pub fn attach_transcript(
        &self,
        store: &Store,
        id: &str,
        transcript_id: &str,
    ) -> Result<(), String> {
        self.update(store, id, |info| {
            info.transcript_id = Some(transcript_id.into())
        })
    }
    pub fn delete_history(&self, store: &Store, id: Option<String>) -> Result<(), String> {
        store.delete(id.clone())?;
        self.0
            .lock()
            .map_err(|_| "Audio storage unavailable")?
            .retain(|clip| {
                id.as_ref()
                    .is_some_and(|id| &clip.0.id != id && clip.0.transcript_id.as_ref() != Some(id))
            });
        Ok(())
    }
}
/// Only fixed messages are persisted. Never retain raw provider or OS payloads.
pub fn failure_reason(error: &str) -> String {
    let lower = error.to_lowercase();
    if error == crate::deadline::AUDIO_TIMEOUT
        || lower.contains("timed out")
        || lower.contains("took too long")
    {
        "Transcription took too long. Retry this recording; it has been kept."
    } else if lower.contains("rate limit") {
        "Groq's rate limit was reached. Wait before retrying."
    } else if lower.contains("key")
        || lower.contains("authentication")
        || lower.contains("permission")
    {
        "Check your API key and account permissions in Settings, then retry."
    } else if lower.contains("network")
        || lower.contains("reach groq")
        || lower.contains("connection")
    {
        "The connection failed. Check your network, then retry."
    } else if lower.contains("no speech")
        || lower.contains("no usable transcript")
        || lower.contains("couldn't hear")
    {
        "No speech was returned. Listen to the recording and check your speech settings."
    } else if lower.contains("offline") || lower.contains("speech pack") {
        "Check the installed offline speech pack, then retry."
    } else if lower.contains("model") {
        "Check the selected speech model in Settings, then retry."
    } else if error == "Cancelled" {
        "Retry cancelled. The original recording is still available."
    } else {
        "The recording could not be processed. Your audio is still available to retry."
    }
    .into()
}
fn crypt(data: &[u8], protect: bool) -> Result<Vec<u8>, String> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let entropy = b"Hush recovery audio v1";
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy.len() as u32,
        pbData: entropy.as_ptr() as *mut u8,
    };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        let result = if protect {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                Some(&entropy),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        } else {
            CryptUnprotectData(
                &input,
                None,
                Some(&entropy),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        result.map_err(|_| {
            "Windows could not unlock or protect this recovery recording.".to_string()
        })?;
        if out.pbData.is_null() {
            return Err("Windows returned empty recovery audio.".into());
        }
        let result = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        std::slice::from_raw_parts_mut(out.pbData, out.cbData as usize).zeroize();
        let _ = LocalFree(Some(HLOCAL(out.pbData.cast())));
        Ok(result)
    }
}
fn seal(wav: &[u8]) -> Result<Vec<u8>, String> {
    if wav.len() > MAX_AUDIO || wav.len() < 44 || &wav[..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err("Invalid recovery recording.".into());
    }
    let mut payload = zeroize::Zeroizing::new(Vec::with_capacity(wav.len() + 32));
    payload.extend_from_slice(&Sha256::digest(wav));
    payload.extend_from_slice(wav);
    crypt(&payload, true)
}
fn unseal(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() > MAX_AUDIO + 4096 {
        return Err("Recovery audio exceeds its size limit.".into());
    }
    let payload = zeroize::Zeroizing::new(crypt(data, false)?);
    if payload.len() < 76
        || payload.len() > MAX_AUDIO + 32
        || Sha256::digest(&payload[32..]).as_slice() != &payload[..32]
    {
        return Err("Recovery audio is damaged. Nothing was sent.".into());
    }
    Ok(payload[32..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_saved_audio_metadata_loads_and_retry_outcomes_keep_audio() {
        let old: SavedAudio = serde_json::from_value(serde_json::json!({"id":"old", "createdAt":"2026", "durationMs":1000, "inMemory":false, "requiresReview":false})).unwrap();
        assert_eq!(old.status, "failed");
        assert_eq!(old.attempts, 1);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("retry.sqlite");
        let id;
        {
            let store = Store::open(&path).unwrap();
            let recovery = Recovery::default();
            let saved = recovery.save(&store, recording(), true);
            id = saved.id;
            recovery.finish_attempt(&store, &id, None).unwrap();
            recovery.attach_transcript(&store, &id, &id).unwrap();
            recovery.begin_retry(&store, &id).unwrap();
            recovery
                .finish_attempt(&store, &id, Some("network error: secret body test-key"))
                .unwrap();
            let metadata = &recovery.list(&store).unwrap()[0];
            assert_eq!(metadata.status, "failed");
            assert_eq!(metadata.attempts, 2);
            assert_eq!(metadata.transcript_id.as_deref(), Some(id.as_str()));
            assert!(!serde_json::to_string(metadata)
                .unwrap()
                .contains("test-key"));
        }
        let store = Store::open(&path).unwrap();
        let recovery = Recovery::default();
        assert_eq!(recovery.load(&store, &id).unwrap().wav, recording().wav);
        recovery.begin_retry(&store, &id).unwrap();
        recovery.finish_attempt(&store, &id, None).unwrap();
        assert_eq!(recovery.list(&store).unwrap()[0].status, "ready");
        assert_eq!(recovery.load(&store, &id).unwrap().wav, recording().wav);
        recovery.delete(&store, &id).unwrap();
        assert!(recovery.finish_attempt(&store, &id, None).is_err());
        assert!(recovery.list(&store).unwrap().is_empty());
    }
    #[test]
    fn private_success_and_retry_stay_in_memory_and_clear_together() {
        let store = Store::open(std::path::Path::new(":memory:")).unwrap();
        let recovery = Recovery::default();
        let info = recovery.save(&store, recording(), false);
        recovery.finish_attempt(&store, &info.id, None).unwrap();
        recovery.begin_retry(&store, &info.id).unwrap();
        recovery
            .finish_attempt(&store, &info.id, Some("Cancelled"))
            .unwrap();
        assert_eq!(
            recovery.load(&store, &info.id).unwrap().wav,
            recording().wav
        );
        assert!(store.saved_audios().unwrap().is_empty());
        recovery.delete_history(&store, None).unwrap();
        assert!(recovery.list(&store).unwrap().is_empty());
    }
    #[test]
    fn recording_history_keeps_more_than_three_transcriptions() {
        let store = Store::open(std::path::Path::new(":memory:")).unwrap();
        let recovery = Recovery::default();
        let first = recovery.save(&store, recording(), true);
        for _ in 0..4 {
            recovery.save(&store, recording(), true);
        }
        assert_eq!(recovery.list(&store).unwrap().len(), 5);
        assert_eq!(
            recovery.load(&store, &first.id).unwrap().wav,
            recording().wav
        );
    }
    fn recording() -> Recording {
        let mut wav = vec![0; 64];
        wav[..4].copy_from_slice(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        wav[44..].fill(123);
        Recording {
            wav,
            duration_ms: 1000,
            limit_reached: false,
            encoding_ms: 0,
            requires_review: false,
            speech_detected: true,
        }
    }
    #[test]
    fn encrypted_audio_survives_restart_and_private_audio_does_not_touch_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.sqlite");
        let id;
        {
            let store = Store::open(&path).unwrap();
            let recovery = Recovery::default();
            let disk = recovery.save(&store, recording(), true);
            assert!(!disk.in_memory);
            id = disk.id.clone();
            assert_eq!(recovery.load(&store, &id).unwrap().wav, recording().wav);
            let (_, encrypted) = store.saved_audio(&id).unwrap();
            assert!(!encrypted.windows(4).any(|v| v == b"RIFF"));
            let memory = recovery.save(&store, recording(), false);
            assert!(memory.in_memory);
            assert_eq!(store.saved_audios().unwrap().len(), 1);
            assert_eq!(recovery.list(&store).unwrap().len(), 2);
        }
        let store = Store::open(&path).unwrap();
        let recovery = Recovery::default();
        assert_eq!(recovery.list(&store).unwrap().len(), 1);
        assert_eq!(recovery.load(&store, &id).unwrap().wav, recording().wav);
        recovery.delete(&store, &id).unwrap();
        assert!(recovery.list(&store).unwrap().is_empty());
    }
    #[test]
    fn corrupted_audio_and_retention_limits_are_enforced() {
        assert!(unseal(b"not encrypted").is_err());
        let store = Store::open(std::path::Path::new(":memory:")).unwrap();
        let recovery = Recovery::default();
        for _ in 0..5 {
            recovery.save(&store, recording(), false);
        }
        assert_eq!(recovery.list(&store).unwrap().len(), MAX_MEMORY_CLIPS);
        for _ in 0..5 {
            recovery.save(&store, recording(), true);
        }
        assert_eq!(store.saved_audios().unwrap().len(), 5);
        assert_eq!(recovery.list(&store).unwrap().len(), 5 + MAX_MEMORY_CLIPS);
        let old = recovery.save(&store, recording(), false);
        recovery
            .0
            .lock()
            .unwrap()
            .iter_mut()
            .find(|c| c.0.id == old.id)
            .unwrap()
            .0
            .created_at = "2000-01-01T00:00:00+00:00".into();
        assert!(recovery.load(&store, &old.id).is_err());
    }
}
