// SPDX-License-Identifier: MIT
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Literal,
    #[default]
    Clean,
    Polished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub preferences_version: u32,
    pub provider: String,
    pub offline_model: String,
    pub local_text_model: String,
    pub formatting: String,
    pub dock: String,
    pub microphone: String,
    pub speech_model: String,
    pub cleanup_model: String,
    pub command_model: String,
    pub language: String,
    pub mode: Mode,
    pub enhanced: bool,
    pub history_enabled: bool,
    pub context_enabled: bool,
    pub selection_permission: bool,
    pub max_seconds: u32,
    pub silence_seconds: u32,
    pub vad_threshold: f32,
    pub double_tap: bool,
    pub theme: String,
    pub insertion: String,
    pub shortcuts: Shortcuts,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preferences_version: 2,
            provider: "groq".into(),
            offline_model: "tiny".into(),
            local_text_model: String::new(),
            formatting: "auto".into(),
            dock: "bottom".into(),
            microphone: String::new(),
            speech_model: "whisper-large-v3-turbo".into(),
            cleanup_model: "openai/gpt-oss-20b".into(),
            command_model: "openai/gpt-oss-120b".into(),
            language: String::new(),
            mode: Mode::Clean,
            enhanced: false,
            history_enabled: true,
            context_enabled: false,
            selection_permission: false,
            max_seconds: 120,
            silence_seconds: 8,
            vad_threshold: 0.012,
            double_tap: false,
            theme: "system".into(),
            insertion: "clipboard".into(),
            shortcuts: Shortcuts::default(),
        }
    }
}

impl Settings {
    pub fn migrate(&mut self) -> bool {
        if self.preferences_version >= 2 {
            return false;
        }
        // v0.1 shipped character injection as the default. Prefer one paste on upgrade.
        self.insertion = "clipboard".into();
        self.preferences_version = 2;
        true
    }
    pub fn validate(&self) -> Result<(), String> {
        if !["groq", "offline"].contains(&self.provider.as_str())
            || !["tiny", "base"].contains(&self.offline_model.as_str())
            || !["auto", "paragraphs", "notes", "off"].contains(&self.formatting.as_str())
            || !["bottom", "left", "right", "hidden"].contains(&self.dock.as_str())
        {
            return Err("Invalid dictation, formatting, or dock preference.".into());
        }
        if self.local_text_model.len() > 128
            || self
                .local_text_model
                .chars()
                .any(|c| !c.is_ascii_alphanumeric() && !"-_.:/".contains(c))
        {
            return Err("Invalid local text model name.".into());
        }
        if !(5..=600).contains(&self.max_seconds) {
            return Err("Maximum recording must be 5–600 seconds.".into());
        }
        if !(2..=60).contains(&self.silence_seconds) {
            return Err("Silence timeout must be 2–60 seconds.".into());
        }
        if !self.vad_threshold.is_finite() || !(0.001..=0.2).contains(&self.vad_threshold) {
            return Err("Voice sensitivity is out of range.".into());
        }
        if !["unicode", "clipboard"].contains(&self.insertion.as_str()) {
            return Err("Unknown insertion method.".into());
        }
        if !["light", "dark", "system"].contains(&self.theme.as_str()) {
            return Err("Unknown theme.".into());
        }
        if !self.language.is_empty()
            && (self.language.len() != 2 || !self.language.chars().all(|c| c.is_ascii_lowercase()))
        {
            return Err("Use a two-letter language code, or Automatic.".into());
        }
        for model in [&self.speech_model, &self.cleanup_model, &self.command_model] {
            if model.is_empty()
                || model.len() > 128
                || !model
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_/.:".contains(c))
            {
                return Err("Invalid model name.".into());
            }
        }
        crate::shortcuts::validate(&self.shortcuts)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Shortcuts {
    pub push: String,
    pub hands_free: String,
    pub command: String,
    pub paste: String,
    pub copy: String,
    pub open: String,
    pub scratchpad: String,
    pub cancel: String,
}
impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            push: "Ctrl+Win".into(),
            hands_free: "Ctrl+Win+Space".into(),
            command: "Ctrl+Win+Alt".into(),
            paste: "Ctrl+Win+V".into(),
            copy: "Ctrl+Win+C".into(),
            open: "Ctrl+Win+H".into(),
            scratchpad: "Ctrl+Win+J".into(),
            cancel: "Escape".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub source: String,
    pub replacement: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub id: String,
    pub trigger: String,
    pub content: String,
    pub confirm: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    pub id: String,
    pub name: String,
    pub app_match: String,
    pub instruction: String,
    pub enhanced: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub id: String,
    pub text: String,
    pub created_at: String,
    pub app: String,
    pub words: u32,
    pub duration_ms: u64,
    pub latency_ms: u64,
    pub inserted: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub version: u32,
    pub settings: Settings,
    pub dictionary: Vec<Entry>,
    pub snippets: Vec<Snippet>,
    pub styles: Vec<Style>,
}
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub phase: String,
    pub message: String,
    pub level: f32,
    pub seconds: f32,
    pub last_text: String,
    pub preview: Option<String>,
    pub original: Option<String>,
    pub has_key: bool,
    pub generation: u64,
    pub warning: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub offline: crate::offline::Status,
    pub settings: Settings,
    pub dictionary: Vec<Entry>,
    pub snippets: Vec<Snippet>,
    pub styles: Vec<Style>,
    pub history: Vec<Transcript>,
    pub scratchpad: String,
    pub view: View,
    pub startup_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_bounds_and_rejects_nan() {
        let mut s = Settings::default();
        assert!(s.validate().is_ok());
        s.max_seconds = 601;
        assert!(s.validate().is_err());
        s.max_seconds = 120;
        s.vad_threshold = f32::NAN;
        assert!(s.validate().is_err());
    }
    #[test]
    fn does_not_serialize_a_secret() {
        let json = serde_json::to_string(&Settings::default()).unwrap();
        assert!(!json.contains("apiKey"));
        assert!(!json.contains("gsk_"));
    }
    #[test]
    fn legacy_settings_upgrade_to_paste_once_without_losing_the_shortcut() {
        let mut settings: Settings =
            serde_json::from_str(r#"{"insertion":"unicode","shortcuts":{"push":"Ctrl+Alt+R"}}"#)
                .unwrap();
        assert!(settings.migrate());
        assert_eq!(settings.insertion, "clipboard");
        assert_eq!(settings.shortcuts.push, "Ctrl+Alt+R");
        settings.insertion = "unicode".into();
        assert!(!settings.migrate());
        assert_eq!(settings.insertion, "unicode");
    }
}
