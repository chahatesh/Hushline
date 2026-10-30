// SPDX-License-Identifier: MIT
use crate::model::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct Store(Mutex<Connection>, PathBuf, Mutex<()>);

#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySummary {
    pub count: u64,
    pub words: u64,
    pub revision: u64,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub records: Vec<Transcript>,
    pub total: u64,
    pub offset: u64,
    pub revision: u64,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct InsightCache {
    version: u32,
    revision: u64,
    timezone_offset: i32,
    data: crate::insights::Statistics,
}
type Result<T> = std::result::Result<T, String>;
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.busy_timeout(std::time::Duration::from_secs(3))
            .map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA secure_delete=ON; CREATE TABLE IF NOT EXISTS config (key TEXT PRIMARY KEY, value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS history (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, record TEXT NOT NULL); CREATE INDEX IF NOT EXISTS history_recent ON history(created_at DESC); CREATE TABLE IF NOT EXISTS recovery_audio (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, metadata TEXT NOT NULL, audio BLOB NOT NULL); PRAGMA user_version=2;").map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM recovery_audio WHERE created_at < ?1",
            [audio_cutoff()],
        )
        .map_err(|e| e.to_string())?;
        conn.execute_batch("CREATE TABLE IF NOT EXISTS history_totals (singleton INTEGER PRIMARY KEY CHECK(singleton=1), count INTEGER NOT NULL, words INTEGER NOT NULL, revision INTEGER NOT NULL);").map_err(|e|e.to_string())?;
        let initialized: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM history_totals WHERE singleton=1)",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !initialized {
            conn.execute("INSERT INTO history_totals SELECT 1,count(*),coalesce(sum(CASE WHEN json_valid(record) THEN coalesce(json_extract(record,'$.words'),0) ELSE 0 END),0),0 FROM history",[]).map_err(|e|e.to_string())?;
        }
        conn.execute_batch("CREATE TRIGGER IF NOT EXISTS history_added AFTER INSERT ON history BEGIN UPDATE history_totals SET count=count+1,words=words+coalesce(json_extract(new.record,'$.words'),0),revision=revision+1; END;
            CREATE TRIGGER IF NOT EXISTS history_edited AFTER UPDATE ON history BEGIN UPDATE history_totals SET words=words-(CASE WHEN json_valid(old.record) THEN coalesce(json_extract(old.record,'$.words'),0) ELSE 0 END)+coalesce(json_extract(new.record,'$.words'),0),revision=revision+1; END;
            CREATE TRIGGER IF NOT EXISTS history_removed AFTER DELETE ON history BEGIN UPDATE history_totals SET count=count-1,words=words-(CASE WHEN json_valid(old.record) THEN coalesce(json_extract(old.record,'$.words'),0) ELSE 0 END),revision=revision+1; END;
            PRAGMA user_version=3;").map_err(|e|e.to_string())?;
        Ok(Self(Mutex::new(conn), path.to_path_buf(), Mutex::new(())))
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
            .prepare("SELECT record FROM history ORDER BY created_at DESC,id DESC LIMIT 50")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn summary(&self) -> Result<HistorySummary> {
        summary(&*self.0.lock().map_err(|_| "Storage unavailable")?)
    }
    pub fn history_page(&self, query: &str, offset: u64, limit: u32) -> Result<HistoryPage> {
        if query.len() > 512 || !(1..=100).contains(&limit) || offset > i64::MAX as u64 {
            return Err("Invalid history request.".into());
        }
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        let tx = c.unchecked_transaction().map_err(|e| e.to_string())?;
        let filter = "(?1='' OR instr(lower(coalesce(json_extract(record,'$.text'),'') || ' ' || coalesce(json_extract(record,'$.app'),'')),lower(?1))>0)";
        let total = tx
            .query_row(
                &format!("SELECT count(*) FROM history WHERE {filter}"),
                [query],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let mut stmt = tx.prepare(&format!("SELECT record FROM history WHERE {filter} ORDER BY created_at DESC,id DESC LIMIT ?2 OFFSET ?3")).map_err(|e|e.to_string())?;
        let rows = stmt
            .query_map(params![query, limit, offset as i64], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|e| e.to_string())?;
        let records = rows
            .map(|row| {
                serde_json::from_str(&row.map_err(|e| e.to_string())?)
                    .map_err(|_| "Invalid history record.".into())
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(HistoryPage {
            records,
            total,
            offset,
            revision: summary(&tx)?.revision,
        })
    }
    /// Separate read connection/WAL snapshot: scanning old history cannot hold up
    /// the recording actor's settings reads or a newly completed dictation.
    pub fn statistics(&self) -> Result<crate::insights::Statistics> {
        let _job = self.2.lock().map_err(|_| "Statistics unavailable")?;
        let revision = self.summary()?.revision;
        let offset = chrono::Local::now().offset().local_minus_utc();
        if let Some(cache) = self
            .get::<Option<InsightCache>>("insights_cache")
            .unwrap_or(None)
        {
            if cache.version == crate::insights::CACHE_VERSION
                && cache.revision == revision
                && cache.timezone_offset == offset
            {
                return Ok(cache.data);
            }
        }
        let (revision, data) = if self.1 == Path::new(":memory:") {
            collect_statistics(&*self.0.lock().map_err(|_| "Storage unavailable")?)?
        } else {
            let conn = Connection::open_with_flags(
                &self.1,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(|_| "History unavailable")?;
            conn.busy_timeout(std::time::Duration::from_secs(3))
                .map_err(|e| e.to_string())?;
            collect_statistics(&conn)?
        };
        let cache = InsightCache {
            version: crate::insights::CACHE_VERSION,
            revision,
            timezone_offset: offset,
            data,
        };
        let json = serde_json::to_string(&cache).map_err(|e| e.to_string())?;
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        if summary(&c)?.revision == revision {
            c.execute("INSERT INTO config VALUES('insights_cache',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[json]).map_err(|e|e.to_string())?;
            c.execute("INSERT INTO config VALUES('profile_milestone',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[(cache.data.total.words / crate::insights::PROFILE_WORDS).to_string()]).map_err(|e|e.to_string())?;
        }
        Ok(cache.data)
    }
    pub fn profile_due(&self) -> bool {
        self.summary().is_ok_and(|summary| {
            summary.words >= crate::insights::PROFILE_WORDS
                && self.get::<Option<u64>>("profile_milestone").ok().flatten()
                    != Some(summary.words / crate::insights::PROFILE_WORDS)
        })
    }
    pub fn last_text(&self, history_enabled: bool) -> Result<String> {
        if !history_enabled {
            return Ok(String::new());
        }
        let c = self.0.lock().map_err(|_| "Storage unavailable")?;
        let record: Option<String> = c.query_row(
            "SELECT record FROM history WHERE length(record)<=1000000 ORDER BY created_at DESC LIMIT 1", [], |row| row.get(0),
        ).optional().map_err(|_| "History unavailable")?;
        let Some(record) = record else {
            return Ok(String::new());
        };
        let transcript: Transcript =
            serde_json::from_str(&record).map_err(|_| "Invalid history record")?;
        Ok(if transcript.text.len() <= 100_000 {
            transcript.text
        } else {
            String::new()
        })
    }
    pub fn insert(&self, record: &Transcript) -> Result<()> {
        let mut c = self.0.lock().map_err(|_| "Storage unavailable")?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        // Count from the final saved text, including on a retry.
        let mut record = record.clone();
        record.words = record.text.split_whitespace().count() as u32;
        let existing: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM history WHERE id=?1)",
                [&record.id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO history VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET record=excluded.record",
            params![
                record.id,
                record.created_at,
                serde_json::to_string(&record).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM config WHERE key='insights_cache'", [])
            .map_err(|e| e.to_string())?;
        if existing {
            tx.execute("DELETE FROM config WHERE key='profile_milestone'", [])
                .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
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
        tx.execute(
            "DELETE FROM config WHERE key IN ('insights_cache','profile_milestone')",
            [],
        )
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
fn summary(c: &Connection) -> Result<HistorySummary> {
    c.query_row(
        "SELECT count,words,revision FROM history_totals WHERE singleton=1",
        [],
        |r| {
            Ok(HistorySummary {
                count: r.get(0)?,
                words: r.get(1)?,
                revision: r.get(2)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}
fn collect_statistics(c: &Connection) -> Result<(u64, crate::insights::Statistics)> {
    let tx = c.unchecked_transaction().map_err(|e| e.to_string())?;
    let revision = summary(&tx)?.revision;
    let mut builder = crate::insights::Builder::default();
    let mut stmt = tx.prepare("SELECT CASE WHEN length(record)<=1000000 THEN record ELSE '' END FROM history ORDER BY julianday(created_at) ASC,id ASC").map_err(|e|e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    for row in rows {
        match serde_json::from_str::<Transcript>(&row.map_err(|e| e.to_string())?) {
            Ok(record) => builder.push(&record),
            Err(_) => builder.skip(),
        }
    }
    Ok((revision, builder.finish()))
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
    #[test]
    fn all_time_paging_search_profiles_and_retries_survive_restart_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("all-time.sqlite");
        {
            let store = Store::open(&path).unwrap();
            for index in 0..1105 {
                let mut record = transcript(
                    &format!("record-{index:04}"),
                    "",
                    "code prompt notes plan write test task model app api",
                );
                record.created_at = format!("2019-01-02T12:{:02}:{:02}Z", index / 60, index % 60);
                store.insert(&record).unwrap();
            }
            let summary = store.summary().unwrap();
            assert_eq!(summary.count, 1105);
            assert_eq!(summary.words, 11050);
            assert!(store.profile_due());
            let stats = store.statistics().unwrap();
            assert_eq!(stats.total.recordings, 1105);
            assert_eq!(stats.total.words, 11050);
            assert_eq!(stats.profiles.len(), 1);
            assert!(!store.profile_due());
            assert_eq!(store.history().unwrap().len(), 50);
            let old = store.history_page("", 1100, 50).unwrap();
            assert_eq!(old.records.len(), 5);
            assert_eq!(old.total, 1105);
            assert_eq!(old.records.last().unwrap().id, "record-0000");
            assert_eq!(store.history_page("prompt", 0, 50).unwrap().total, 1105);
            assert_eq!(store.history_page("%", 0, 50).unwrap().total, 0);
            assert!(store.history_page("", 0, 101).is_err());
        }
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.statistics().unwrap().profiles.len(), 1);
        let mut retried = reopened.transcript("record-0000").unwrap().unwrap();
        retried.text = "A corrected prompt".into();
        reopened.insert(&retried).unwrap();
        assert_eq!(reopened.summary().unwrap().count, 1105);
        assert_eq!(reopened.summary().unwrap().words, 11043);
        assert_eq!(reopened.statistics().unwrap().total.words, 11043);
        reopened.delete(Some("record-0001".into())).unwrap();
        assert_eq!(reopened.statistics().unwrap().total.words, 11033);
        reopened.delete(None).unwrap();
        let empty = reopened.statistics().unwrap();
        assert_eq!(empty.total.recordings, 0);
        assert!(empty.profiles.is_empty());
        assert!(empty.topics.is_empty());
        assert!(reopened
            .history_page("", 0, 100)
            .unwrap()
            .records
            .is_empty());
    }
    #[test]
    fn legacy_history_migrates_without_expiration_or_exposing_text_in_statistics() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.sqlite");
        {
            let connection = Connection::open(&path).unwrap();
            connection.execute_batch("CREATE TABLE history(id TEXT PRIMARY KEY,created_at TEXT NOT NULL,record TEXT NOT NULL)").unwrap();
            let record = transcript("legacy", "", "Private original sentence");
            connection
                .execute(
                    "INSERT INTO history VALUES(?1,?2,?3)",
                    params![
                        "legacy",
                        "2001-01-01T12:00:00Z",
                        serde_json::to_string(&Transcript {
                            created_at: "2001-01-01T12:00:00Z".into(),
                            words: 3,
                            ..record
                        })
                        .unwrap()
                    ],
                )
                .unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.summary().unwrap().words, 3);
        let stats = store.statistics().unwrap();
        assert_eq!(stats.total.words, 3);
        assert!(stats.days[0].name.starts_with("2001"));
        assert!(!serde_json::to_string(&stats)
            .unwrap()
            .contains("Private original sentence"));
        assert_eq!(store.history_page("Private", 0, 50).unwrap().total, 1);
        let again = Store::open(&path).unwrap();
        assert_eq!(again.summary().unwrap().count, 1);
    }
    #[test]
    fn last_transcript_recovers_after_restart_but_never_in_private_mode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.sqlite");
        {
            let store = Store::open(&path).unwrap();
            let mut old = transcript("old", "", "Older note");
            old.created_at = "2026-10-27T12:00:00Z".into();
            let mut latest = transcript("new", "", "Latest note");
            latest.created_at = "2026-10-28T12:00:00Z".into();
            store.insert(&latest).unwrap();
            store.insert(&old).unwrap();
        }
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.last_text(true).unwrap(), "Latest note");
        assert_eq!(reopened.last_text(false).unwrap(), "");
        reopened.delete(None).unwrap();
        assert_eq!(reopened.last_text(true).unwrap(), "");
    }
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
