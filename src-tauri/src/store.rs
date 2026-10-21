// SPDX-License-Identifier: MIT
use crate::model::*;
use rusqlite::{params, Connection};
use std::{path::Path, sync::Mutex};

pub struct Store(Mutex<Connection>);
type Result<T> = std::result::Result<T, String>;
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.busy_timeout(std::time::Duration::from_secs(3))
            .map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA secure_delete=ON; CREATE TABLE IF NOT EXISTS config (key TEXT PRIMARY KEY, value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS history (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS recovery_audio (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, metadata TEXT NOT NULL, audio BLOB NOT NULL); PRAGMA user_version=2;").map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM recovery_audio WHERE created_at < ?1",
            [audio_cutoff()],
        )
        .map_err(|e| e.to_string())?;
        Ok(Self(Mutex::new(conn)))
    }
    pub fn save_audio(&self, info: &crate::recovery::SavedAudio, data: &[u8]) -> Result<()> {
        let mut c = self.0.lock().map_err(|_| "Storage unavailable")?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO recovery_audio VALUES(?1,?2,?3,?4)",
            params![
                info.id,
                info.created_at,
                serde_json::to_string(info).map_err(|e| e.to_string())?,
                data
            ],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM recovery_audio WHERE created_at < ?1 OR id NOT IN (SELECT id FROM recovery_audio ORDER BY created_at DESC LIMIT ?2)", params![audio_cutoff(), crate::recovery::MAX_CLIPS]).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    pub fn saved_audios(&self) -> Result<Vec<crate::recovery::SavedAudio>> {
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        c.execute(
            "DELETE FROM recovery_audio WHERE created_at < ?1",
            [audio_cutoff()],
        )
        .map_err(|e| e.to_string())?;
        let mut stmt = c.prepare("SELECT metadata FROM recovery_audio WHERE created_at >= ?1 ORDER BY created_at DESC").map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([audio_cutoff()], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn saved_audio(&self, id: &str) -> Result<(crate::recovery::SavedAudio, Vec<u8>)> {
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        let (metadata, data): (String, Vec<u8>) = c.query_row("SELECT metadata,audio FROM recovery_audio WHERE id=?1 AND created_at>=?2 AND length(audio)<=24004096", params![id,audio_cutoff()], |r| Ok((r.get(0)?,r.get(1)?))).map_err(|_| "This saved recording is no longer available.")?;
        Ok((
            serde_json::from_str(&metadata).map_err(|_| "Invalid saved recording.")?,
            data,
        ))
    }
    pub fn delete_audio(&self, id: &str) -> Result<()> {
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        c.execute("DELETE FROM recovery_audio WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| e.to_string())
    }
    pub fn get<T: serde::de::DeserializeOwned + Default>(&self, key: &str) -> Result<T> {
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        let mut stmt = c
            .prepare("SELECT value FROM config WHERE key=?1")
            .map_err(|e| e.to_string())?;
        let value = stmt.query_row([key], |r| r.get::<_, String>(0));
        match value {
            Ok(v) => serde_json::from_str(&v).map_err(|e| e.to_string()),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(T::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn set<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.0.lock().map_err(|_| "Storage unavailable")?.execute("INSERT INTO config(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, json]).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn history(&self) -> Result<Vec<Transcript>> {
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        let mut stmt = c
            .prepare("SELECT record FROM history ORDER BY created_at DESC LIMIT 1000")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn insert(&self, record: &Transcript) -> Result<()> {
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        c.execute(
            "INSERT INTO history VALUES(?1,?2,?3)",
            params![
                record.id,
                record.created_at,
                serde_json::to_string(record).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn delete(&self, id: Option<String>) -> Result<()> {
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        match id {
            Some(id) => c.execute("DELETE FROM history WHERE id=?1", [id]),
            None => c.execute("DELETE FROM history", []),
        }
        .map_err(|e| e.to_string())?;
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn restore(&self, backup: &Backup) -> Result<()> {
        if backup.version != 1 {
            return Err("Unsupported backup version.".into());
        }
        backup.settings.validate()?;
        crate::text::validate_entries(&backup.dictionary, &backup.snippets, &backup.styles)?;
        let mut c = self.0.lock().map_err(|_| "Storage unavailable")?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        for (key, value) in [
            ("settings", serde_json::to_string(&backup.settings)),
            ("dictionary", serde_json::to_string(&backup.dictionary)),
            ("snippets", serde_json::to_string(&backup.snippets)),
            ("styles", serde_json::to_string(&backup.styles)),
        ] {
            tx.execute("INSERT INTO config VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value.map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
}
fn audio_cutoff() -> String {
    (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_audio_cannot_be_read_and_is_removed_on_access() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let info = crate::recovery::SavedAudio {
            id: "expired".into(),
            created_at: "2000-01-01T00:00:00+00:00".into(),
            duration_ms: 1000,
            requires_review: false,
            in_memory: false,
        };
        store
            .0
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO recovery_audio VALUES(?1,?2,?3,?4)",
                params![
                    info.id,
                    info.created_at,
                    serde_json::to_string(&info).unwrap(),
                    b"encrypted test bytes".as_slice()
                ],
            )
            .unwrap();
        assert!(store.saved_audio(&info.id).is_err());
        assert!(store.saved_audios().unwrap().is_empty());
        let count: i64 = store
            .0
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM recovery_audio", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
    #[test]
    fn invalid_restore_leaves_existing_data_untouched() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        s.set(
            "dictionary",
            &vec![Entry {
                id: "one".into(),
                source: "existing".into(),
                replacement: String::new(),
            }],
        )
        .unwrap();
        let mut b = Backup {
            version: 1,
            settings: Settings::default(),
            dictionary: vec![Entry {
                id: "bad".into(),
                source: String::new(),
                replacement: "bad".into(),
            }],
            snippets: vec![],
            styles: vec![],
        };
        assert!(s.restore(&b).is_err());
        assert_eq!(
            s.get::<Vec<Entry>>("dictionary").unwrap()[0].source,
            "existing"
        );
        b.dictionary.clear();
        b.settings.shortcuts.copy = b.settings.shortcuts.push.clone();
        assert!(s.restore(&b).is_err());
        assert_eq!(s.get::<Vec<Entry>>("dictionary").unwrap().len(), 1);
    }
    #[test]
    fn persistence_and_delete() {
        let p = std::env::temp_dir().join(format!("hushline-test-{}.sqlite", uuid::Uuid::new_v4()));
        {
            let s = Store::open(&p).unwrap();
            s.set("scratchpad", &"hello").unwrap();
            assert_eq!(s.get::<String>("scratchpad").unwrap(), "hello");
            let t = Transcript {
                id: "1".into(),
                text: "Private words".into(),
                created_at: "2026".into(),
                app: "Notepad".into(),
                words: 2,
                duration_ms: 100,
                latency_ms: 20,
                inserted: true,
            };
            s.insert(&t).unwrap();
            assert_eq!(s.history().unwrap().len(), 1);
            s.delete(None).unwrap();
            assert!(s.history().unwrap().is_empty());
        }
        {
            let s = Store::open(&p).unwrap();
            assert_eq!(s.get::<String>("scratchpad").unwrap(), "hello");
        }
        let _ = std::fs::remove_file(p);
    }
}
