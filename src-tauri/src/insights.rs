// SPDX-License-Identifier: MIT
//! Local, deterministic usage insights. No inference, audio, excerpts or personal traits.
use crate::model::Transcript;
use chrono::{DateTime, Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub const PROFILE_WORDS: u64 = 10_000;
pub const CACHE_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub words: u64,
    pub recordings: u64,
    pub duration_ms: u64,
    pub latency_ms: u64,
    pub inserted: u64,
    pub longest: u64,
    pub with_lists: u64,
    pub with_paragraphs: u64,
}
impl Usage {
    fn add(&mut self, record: &Transcript, words: u64) {
        self.words += words;
        self.recordings += 1;
        self.duration_ms += record.duration_ms;
        self.latency_ms += record.latency_ms;
        self.inserted += u64::from(record.inserted);
        self.longest = self.longest.max(words);
        self.with_lists += u64::from(record.text.lines().any(is_list_item));
        self.with_paragraphs +=
            u64::from(record.text.contains("\n\n") || record.text.contains("\r\n\r\n"));
    }
}
fn is_list_item(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("• ")
        || line.starts_with("- ")
        || line.starts_with("* ")
        || line
            .split_once(". ")
            .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamedUsage {
    pub name: String,
    #[serde(flatten)]
    pub usage: Usage,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signal {
    pub name: String,
    pub count: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProfile {
    pub from_word: u64,
    pub through_word: u64,
    pub completed_at: String,
    pub topics: Vec<Signal>,
    pub terms: Vec<Signal>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub total: Usage,
    pub days: Vec<NamedUsage>,
    pub apps: Vec<NamedUsage>,
    pub hours: Vec<Usage>,
    pub weekdays: Vec<Usage>,
    pub profiles: Vec<VoiceProfile>,
    pub topics: Vec<Signal>,
    pub terms: Vec<Signal>,
    pub unclassified_words: u64,
    pub skipped_records: u64,
}

// Fixed vocabulary keeps memory bounded and prevents names, credentials, URLs,
// or arbitrary transcript phrases from being copied into a derived profile.
const TOPICS: &[(&str, &str)] = &[
    ("Software & development", "code coding software developer programming rust python javascript typescript function functions api apis bug bugs debug debugging tests testing deploy deployment database server frontend backend github repository commit commits ui interface app application"),
    ("AI & prompts", "prompt prompts prompting ai llm model models inference assistant chatbot groq whisper transcription dictation machine learning generative token tokens context"),
    ("Planning & notes", "plan planning agenda meeting meetings notes schedule scheduled calendar deadline deadlines task tasks todo priorities priority milestone milestones reminder reminders project projects checklist"),
    ("Writing & communication", "write writing email emails message messages paragraph paragraphs grammar wording rewrite editing document documents letter letters draft drafts reply replies tone sentence sentences newsletter article articles"),
    ("Learning & research", "learn learning research study studying explain explanation tutorial tutorials lesson lessons science history mathematics question questions understand understanding book books course courses"),
    ("Business & design", "business customer customers client clients product products design designs marketing sales budget invoice invoices revenue pricing brand branding proposal proposals presentation presentations"),
    ("Everyday life", "home family friend friends dog dogs cat cats groceries shopping dinner breakfast lunch cooking travel trip vacation health exercise workout music movie movies game games"),
];
fn vocabulary() -> &'static HashMap<&'static str, usize> {
    static WORDS: std::sync::LazyLock<HashMap<&str, usize>> = std::sync::LazyLock::new(|| {
        TOPICS
            .iter()
            .enumerate()
            .flat_map(|(index, (_, words))| words.split_whitespace().map(move |word| (word, index)))
            .collect()
    });
    &WORDS
}
#[derive(Default)]
struct Evidence {
    topics: BTreeMap<usize, u64>,
    terms: BTreeMap<String, u64>,
}
impl Evidence {
    fn observe(&mut self, raw: &str) -> bool {
        // Count only whole simple words, never pieces of URLs or identifiers.
        let term = raw
            .trim_matches(|c: char| {
                matches!(
                    c,
                    '.' | ',' | '!' | '?' | ':' | ';' | '"' | '\'' | '(' | ')'
                )
            })
            .to_lowercase();
        let Some(topic) = vocabulary().get(term.as_str()) else {
            return false;
        };
        *self.topics.entry(*topic).or_default() += 1;
        *self.terms.entry(term).or_default() += 1;
        true
    }
    fn signals(&self) -> (Vec<Signal>, Vec<Signal>) {
        let mut topics: Vec<_> = self
            .topics
            .iter()
            .map(|(index, count)| Signal {
                name: TOPICS[*index].0.into(),
                count: *count,
            })
            .collect();
        let mut terms: Vec<_> = self
            .terms
            .iter()
            .map(|(name, count)| Signal {
                name: name.clone(),
                count: *count,
            })
            .collect();
        let sort = |a: &Signal, b: &Signal| b.count.cmp(&a.count).then(a.name.cmp(&b.name));
        topics.sort_by(sort);
        terms.sort_by(sort);
        terms.truncate(10);
        (topics, terms)
    }
}
pub struct Builder {
    stats: Statistics,
    days: BTreeMap<String, Usage>,
    apps: BTreeMap<String, Usage>,
    all: Evidence,
    block: Evidence,
    seen: u64,
}
impl Default for Builder {
    fn default() -> Self {
        Self {
            stats: Statistics {
                hours: vec![Usage::default(); 24],
                weekdays: vec![Usage::default(); 7],
                ..Statistics::default()
            },
            days: BTreeMap::new(),
            apps: BTreeMap::new(),
            all: Evidence::default(),
            block: Evidence::default(),
            seen: 0,
        }
    }
}
impl Builder {
    pub fn skip(&mut self) {
        self.stats.skipped_records += 1;
    }
    pub fn push(&mut self, record: &Transcript) {
        let Ok(date) = DateTime::parse_from_rfc3339(&record.created_at) else {
            self.skip();
            return;
        };
        let date = date.with_timezone(&Local);
        let count = record.text.split_whitespace().count() as u64;
        self.stats.total.add(record, count);
        self.days
            .entry(date.format("%Y-%m-%d").to_string())
            .or_default()
            .add(record, count);
        self.apps
            .entry(
                if record.app.is_empty() {
                    "Unknown application"
                } else {
                    &record.app
                }
                .into(),
            )
            .or_default()
            .add(record, count);
        self.stats.hours[date.hour() as usize].add(record, count);
        self.stats.weekdays[date.weekday().num_days_from_monday() as usize].add(record, count);
        for word in record.text.split_whitespace() {
            if !self.all.observe(word) {
                self.stats.unclassified_words += 1;
            }
            self.block.observe(word);
            self.seen += 1;
            if self.seen.is_multiple_of(PROFILE_WORDS) {
                let (topics, terms) = self.block.signals();
                self.stats.profiles.push(VoiceProfile {
                    from_word: self.seen - PROFILE_WORDS + 1,
                    through_word: self.seen,
                    completed_at: record.created_at.clone(),
                    topics,
                    terms,
                });
                self.block = Evidence::default();
            }
        }
    }
    pub fn finish(mut self) -> Statistics {
        self.stats.days = self
            .days
            .into_iter()
            .map(|(name, usage)| NamedUsage { name, usage })
            .collect();
        self.stats.apps = self
            .apps
            .into_iter()
            .map(|(name, usage)| NamedUsage { name, usage })
            .collect();
        self.stats
            .apps
            .sort_by(|a, b| b.usage.words.cmp(&a.usage.words).then(a.name.cmp(&b.name)));
        (self.stats.topics, self.stats.terms) = self.all.signals();
        self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn record(id: &str, text: &str, date: &str) -> Transcript {
        Transcript {
            id: id.into(),
            text: text.into(),
            created_at: date.into(),
            app: "Notepad.exe".into(),
            words: 0,
            duration_ms: 30_000,
            latency_ms: 1200,
            inserted: true,
            delivery_failure: None,
            audio_id: None,
        }
    }
    #[test]
    fn profiles_cut_at_exact_word_boundaries_even_within_one_transcript() {
        let text = format!(
            "{}{}{}",
            "code ".repeat(10_000),
            "prompt ".repeat(10_000),
            "notes ".repeat(8)
        );
        let mut builder = Builder::default();
        builder.push(&record("one", &text, "2020-01-02T12:00:00Z"));
        let stats = builder.finish();
        assert_eq!(stats.total.words, 20_008);
        assert_eq!(stats.profiles.len(), 2);
        assert_eq!(stats.profiles[0].through_word, 10_000);
        assert_eq!(stats.profiles[1].from_word, 10_001);
        assert_eq!(stats.profiles[0].topics[0].name, "Software & development");
        assert_eq!(stats.profiles[1].topics[0].name, "AI & prompts");
        assert_eq!(stats.profiles[1].terms[0].count, 10_000);
    }
    #[test]
    fn all_years_count_dates_are_local_and_profile_has_no_private_excerpts() {
        let mut builder = Builder::default();
        builder.push(&record(
            "old",
            "Code API https://secret.test gsk_secret_123 Asha@example.org",
            "2019-01-02T12:00:00Z",
        ));
        builder.push(&record(
            "new",
            "Notes\n\n• First task\n• Next task",
            "2026-10-29T12:00:00Z",
        ));
        builder.push(&record("bad", "never counted", "bad date"));
        let stats = builder.finish();
        assert_eq!(stats.days.len(), 2);
        assert_eq!(stats.total.recordings, 2);
        assert_eq!(stats.total.with_lists, 1);
        assert_eq!(stats.total.with_paragraphs, 1);
        assert_eq!(stats.skipped_records, 1);
        assert!(stats
            .terms
            .iter()
            .all(|term| !term.name.contains("secret") && !term.name.contains("Asha")));
        assert_eq!(stats.total.words, 12);
        assert_eq!(stats.hours.iter().map(|h| h.words).sum::<u64>(), 12);
        assert_eq!(stats.weekdays.iter().map(|h| h.recordings).sum::<u64>(), 2);
    }
}
