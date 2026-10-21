// SPDX-License-Identifier: MIT
//! Failed audio only: per-user DPAPI on disk, memory-only in private mode.
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

pub const MAX_CLIPS: usize = 3;
const MAX_AUDIO: usize = 24_000_000;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAudio {
    pub id: String,
    pub created_at: String,
    pub duration_ms: u64,
    pub requires_review: bool,
    pub in_memory: bool,
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
        while clips.len() > MAX_CLIPS {
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
        let mut result = store.saved_audios()?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        let mut memory = self.0.lock().unwrap();
        memory.retain(|clip| clip.0.created_at >= cutoff);
        result.extend(memory.iter().map(|clip| clip.0.clone()));
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(result)
    }
    pub fn load(&self, store: &Store, id: &str) -> Result<Recording, String> {
        {
            let cutoff = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
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
            limit_reached: false,
            encoding_ms: 0,
        })
    }
    pub fn delete(&self, store: &Store, id: &str) -> Result<(), String> {
        self.0.lock().unwrap().retain(|clip| clip.0.id != id);
        store.delete_audio(id)
    }
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
        assert_eq!(recovery.list(&store).unwrap().len(), MAX_CLIPS);
        for _ in 0..5 {
            recovery.save(&store, recording(), true);
        }
        assert_eq!(store.saved_audios().unwrap().len(), MAX_CLIPS);
        assert_eq!(recovery.list(&store).unwrap().len(), MAX_CLIPS);
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
