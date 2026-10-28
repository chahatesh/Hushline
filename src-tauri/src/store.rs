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
        prune_audio(
            &tx,
            crate::recovery::MAX_CLIPS,
            crate::recovery::MAX_STORED_BYTES,
        )?;
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
    pub fn update_audio(
        &self,
        id: &str,
        change: impl FnOnce(&mut crate::recovery::SavedAudio),
    ) -> Result<()> {
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        let metadata: String = c
            .query_row(
                "SELECT metadata FROM recovery_audio WHERE id=?1 AND created_at>=?2",
                params![id, audio_cutoff()],
                |r| r.get(0),
            )
            .map_err(|_| "This saved recording is no longer available.")?;
        let mut info = serde_json::from_str(&metadata).map_err(|_| "Invalid saved recording.")?;
        change(&mut info);
        c.execute(
            "UPDATE recovery_audio SET metadata=?2 WHERE id=?1",
            params![id, serde_json::to_string(&info).map_err(|e| e.to_string())?],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
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
            "INSERT INTO history VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET record=excluded.record",
            params![
                record.id,
                record.created_at,
                serde_json::to_string(record).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn transcript(&self, id: &str) -> Result<Option<Transcript>> {
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        match c.query_row("SELECT record FROM history WHERE id=?1", [id], |row| {
            row.get::<_, String>(0)
        }) {
            Ok(record) => serde_json::from_str(&record)
                .map(Some)
                .map_err(|_| "Invalid history record.".into()),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
    pub fn delete(&self, id: Option<String>) -> Result<()> {
        let mut c = self.0.lock().map_err(|_| "Storage unavailable")?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        match &id {
            Some(id) => {
                tx.execute("DELETE FROM recovery_audio WHERE id IN (SELECT json_extract(record,'$.audioId') FROM history WHERE id=?1)", [id]).map_err(|e| e.to_string())?;
            }
            None => {
                tx.execute("DELETE FROM recovery_audio", [])
                    .map_err(|e| e.to_string())?;
            }
        }
        match id {
            Some(id) => tx.execute("DELETE FROM history WHERE id=?1", [id]),
            None => tx.execute("DELETE FROM history", []),
        }
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
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
pub(crate) fn audio_cutoff() -> String {
    (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339()
}
fn prune_audio(connection: &Connection, count: usize, bytes: usize) -> Result<()> {
    connection.execute("DELETE FROM recovery_audio WHERE created_at < ?1 OR id NOT IN (SELECT id FROM (SELECT id, ROW_NUMBER() OVER (ORDER BY created_at DESC, id DESC) AS position, SUM(length(audio)) OVER (ORDER BY created_at DESC, id DESC) AS bytes FROM recovery_audio) WHERE position <= ?2 AND bytes <= ?3)", params![audio_cutoff(), count, bytes]).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn transcript(id: &str, audio_id: &str, text: &str) -> Transcript {
        Transcript {
            id: id.into(),
            audio_id: Some(audio_id.into()),
            text: text.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            app: "fixture.exe".into(),
            words: 2,
            duration_ms: 1000,
            latency_ms: 30,
            inserted: false,
            delivery_failure: None,
        }
    }
    fn audio(id: &str, created: String) -> crate::recovery::SavedAudio {
        crate::recovery::SavedAudio {
            id: id.into(),
            created_at: created,
            duration_ms: 1000,
            requires_review: false,
            in_memory: false,
            status: "pending".into(),
            transcript_id: None,
            last_error: None,
            attempts: 1,
        }
    }
    #[test]
    fn retries_replace_one_history_record_and_deletion_removes_linked_audio() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        store
            .save_audio(&audio("audio", chrono::Utc::now().to_rfc3339()), &[0; 32])
            .unwrap();
        store
            .insert(&transcript("result", "audio", "First result"))
            .unwrap();
        store
            .insert(&transcript("result", "audio", "Retried result"))
            .unwrap();
        assert_eq!(store.history().unwrap().len(), 1);
        assert_eq!(
            store.transcript("result").unwrap().unwrap().text,
            "Retried result"
        );
        store.delete(Some("result".into())).unwrap();
        assert!(store.history().unwrap().is_empty());
        assert!(store.saved_audios().unwrap().is_empty());
        store
            .save_audio(
                &audio("unlinked failure", chrono::Utc::now().to_rfc3339()),
                &[0; 32],
            )
            .unwrap();
        store.delete(None).unwrap();
        assert!(store.saved_audios().unwrap().is_empty());
    }
    #[test]
    fn audio_limits_prune_oldest_by_count_bytes_and_age() {
        for (count, bytes, expected) in [(3, 1000, 3), (100, 70, 2)] {
            let store = Store::open(Path::new(":memory:")).unwrap();
            for index in 0..5 {
                let time = (chrono::Utc::now() - chrono::Duration::seconds(5 - index)).to_rfc3339();
                store
                    .save_audio(&audio(&index.to_string(), time), &[0; 32])
                    .unwrap();
            }
            prune_audio(&store.0.lock().unwrap(), count, bytes).unwrap();
            let kept = store.saved_audios().unwrap();
            assert_eq!(kept.len(), expected);
            assert_eq!(kept[0].id, "4");
            assert!(kept.iter().all(|clip| clip.id != "0"));
        }
        let store = Store::open(Path::new(":memory:")).unwrap();
        store
            .save_audio(
                &audio(
                    "ten days",
                    (chrono::Utc::now() - chrono::Duration::days(10)).to_rfc3339(),
                ),
                &[0; 32],
            )
            .unwrap();
        assert_eq!(store.saved_audios().unwrap().len(), 1);
        store
            .save_audio(
                &audio(
                    "expired",
                    (chrono::Utc::now() - chrono::Duration::days(31)).to_rfc3339(),
                ),
                &[0; 32],
            )
            .unwrap();
        assert_eq!(store.saved_audios().unwrap().len(), 1);
    }
    #[test]
    fn expired_audio_cannot_be_read_and_is_removed_on_access() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let info = crate::recovery::SavedAudio {
            id: "expired".into(),
            created_at: "2000-01-01T00:00:00+00:00".into(),
            duration_ms: 1000,
            requires_review: false,
            in_memory: false,
            status: "failed".into(),
            transcript_id: None,
            last_error: None,
            attempts: 1,
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
                delivery_failure: None,
                audio_id: None,
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
