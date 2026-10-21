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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum WritingStyle {
    Casual,
    #[default]
    Standard,
    Formal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PromptImproveMode {
    #[default]
    Clarify,
    Expand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PolishPolicy {
    Always,
    Sometimes,
    #[default]
    Never,
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
    pub prepare_microphone: bool,
    pub speech_model: String,
    pub cleanup_model: String,
    pub command_model: String,
    pub language: String,
    pub mode: Mode,
    pub writing_style: WritingStyle,
    pub startup_delay_ms: u32,
    pub recording_cue_enabled: bool,
    pub prompt_improve_mode: PromptImproveMode,
    pub enhanced: bool,
    pub polish_policy: PolishPolicy,
    pub history_enabled: bool,
    pub context_enabled: bool,
    pub selection_permission: bool,
    pub max_seconds: u32,
    pub transcription_timeout_seconds: u32,
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
            preferences_version: 5,
            provider: "groq".into(),
            offline_model: "tiny".into(),
            local_text_model: String::new(),
            formatting: "auto".into(),
            dock: "bottom".into(),
            microphone: String::new(),
            prepare_microphone: true,
            speech_model: "whisper-large-v3-turbo".into(),
            cleanup_model: "openai/gpt-oss-20b".into(),
            command_model: "openai/gpt-oss-120b".into(),
            language: String::new(),
            mode: Mode::Clean,
            writing_style: WritingStyle::Standard,
            startup_delay_ms: 300,
            recording_cue_enabled: true,
            prompt_improve_mode: PromptImproveMode::Clarify,
            enhanced: false,
            polish_policy: PolishPolicy::Never,
            history_enabled: true,
            context_enabled: false,
            selection_permission: false,
            max_seconds: 600,
            transcription_timeout_seconds: 5,
            silence_seconds: 0,
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
        if self.preferences_version >= 5 {
            return false;
        }
        // v0.1 shipped character injection as the default. Prefer one paste on upgrade.
        if self.preferences_version < 2 {
            self.insertion = "clipboard".into();
        }
        if self.preferences_version < 3 && self.max_seconds == 120 {
            self.max_seconds = 600;
        }
        if self.preferences_version < 3 && self.silence_seconds == 8 {
            self.silence_seconds = 0;
        }
        if self.preferences_version < 4 && (self.enhanced || self.mode == Mode::Polished) {
            self.polish_policy = PolishPolicy::Always;
        }
        self.writing_style = match self.mode {
            Mode::Literal => WritingStyle::Casual,
            Mode::Clean => WritingStyle::Standard,
            Mode::Polished => WritingStyle::Formal,
        };
        if self.mode == Mode::Polished {
            self.mode = Mode::Clean;
        }
        // The newly introduced action must not invalidate an existing custom
        // shortcut and prevent startup after an upgrade.
        let occupied: Vec<_> = crate::shortcuts::bindings(&self.shortcuts)
            .into_iter()
            .filter(|(_, action)| *action != crate::shortcuts::Action::ImprovePrompt)
            .filter_map(|(chord, _)| crate::shortcuts::parse(chord).ok())
            .collect();
        if crate::shortcuts::parse(&self.shortcuts.improve_prompt)
            .is_ok_and(|c| occupied.contains(&c))
        {
            for candidate in std::iter::once("Ctrl+Win+Shift+P".to_string())
                .chain((1..=24).map(|n| format!("Ctrl+Win+F{n}")))
            {
                if crate::shortcuts::parse(&candidate).is_ok_and(|c| !occupied.contains(&c)) {
                    self.shortcuts.improve_prompt = candidate;
                    break;
                }
            }
        }
        self.preferences_version = 5;
        true
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.startup_delay_ms > 1500 {
            return Err("Startup pause must be between 0 and 1,500 milliseconds.".into());
        }
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
        if !(3..=120).contains(&self.transcription_timeout_seconds) {
            return Err("Transcription timeout must be 3–120 seconds.".into());
        }
        if self.silence_seconds != 0 && !(2..=60).contains(&self.silence_seconds) {
            return Err("Silence timeout must be off, or 2–60 seconds.".into());
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
    pub improve_prompt: String,
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
            improve_prompt: "Ctrl+Win+P".into(),
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
    pub stage: String,
    pub message: String,
    pub level: f32,
    pub seconds: f32,
    pub last_text: String,
    pub preview: Option<String>,
    pub original: Option<String>,
    pub preview_id: Option<String>,
    pub preview_kind: String,
    pub has_key: bool,
    pub generation: u64,
    pub warning: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub diagnostics: Vec<StageDiagnostic>,
    pub saved_audio: Vec<crate::recovery::SavedAudio>,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageDiagnostic {
    pub generation: u64,
    pub stage: String,
    pub elapsed_ms: u64,
    pub outcome: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn style_upgrade_preserves_every_explicit_v4_preference() {
        for (mode, style) in [
            (Mode::Clean, WritingStyle::Standard),
            (Mode::Polished, WritingStyle::Formal),
            (Mode::Literal, WritingStyle::Casual),
        ] {
            for policy in [
                PolishPolicy::Always,
                PolishPolicy::Sometimes,
                PolishPolicy::Never,
            ] {
                let mut s = Settings {
                    preferences_version: 4,
                    mode: mode.clone(),
                    polish_policy: policy.clone(),
                    double_tap: true,
                    provider: "offline".into(),
                    history_enabled: false,
                    transcription_timeout_seconds: 12,
                    ..Settings::default()
                };
                s.shortcuts.push = "Ctrl+Alt+R".into();
                assert!(s.migrate());
                assert_eq!(s.writing_style, style);
                assert_eq!(s.polish_policy, policy);
                assert_eq!(
                    s.mode,
                    if mode == Mode::Literal {
                        Mode::Literal
                    } else {
                        Mode::Clean
                    }
                );
                assert!(s.double_tap);
                assert_eq!(s.provider, "offline");
                assert!(!s.history_enabled);
                assert_eq!(s.transcription_timeout_seconds, 12);
                assert_eq!(s.shortcuts.push, "Ctrl+Alt+R");
                assert_eq!(s.shortcuts.improve_prompt, "Ctrl+Win+P");
                assert_eq!(s.startup_delay_ms, 300);
                assert!(s.recording_cue_enabled);
                assert_eq!(s.prompt_improve_mode, PromptImproveMode::Clarify);
                assert!(!s.migrate());
            }
        }
    }
    #[test]
    fn startup_pause_bounds_and_preview_contract_round_trip() {
        for delay in [0, 300, 1500] {
            let s = Settings {
                startup_delay_ms: delay,
                ..Settings::default()
            };
            assert!(s.validate().is_ok());
        }
        assert!(Settings {
            startup_delay_ms: 1501,
            ..Settings::default()
        }
        .validate()
        .is_err());
        let v = View {
            stage: "prompt_editing".into(),
            preview_kind: "prompt".into(),
            preview_id: Some("approved-preview".into()),
            ..View::default()
        };
        let json = serde_json::to_value(v).unwrap();
        assert_eq!(json["previewKind"], "prompt");
        assert_eq!(json["previewId"], "approved-preview");
        assert_eq!(json["stage"], "prompt_editing");
    }
    #[test]
    fn new_prompt_shortcut_never_overwrites_existing_custom_chords() {
        let mut s = Settings {
            preferences_version: 4,
            ..Settings::default()
        };
        s.shortcuts.open = "Win+Ctrl+P".into();
        s.shortcuts.copy = "Ctrl+Win+Shift+P".into();
        assert!(s.migrate());
        assert_eq!(s.shortcuts.open, "Win+Ctrl+P");
        assert_eq!(s.shortcuts.copy, "Ctrl+Win+Shift+P");
        assert_eq!(s.shortcuts.improve_prompt, "Ctrl+Win+F1");
        assert!(s.validate().is_ok());
    }
    #[test]
    fn polish_upgrade_preserves_opt_in_and_does_not_rewrite_v3_limits() {
        for (old, expected) in [
            (
                r#"{"preferencesVersion":3,"enhanced":true}"#,
                PolishPolicy::Always,
            ),
            (
                r#"{"preferencesVersion":3,"enhanced":false}"#,
                PolishPolicy::Never,
            ),
            (
                r#"{"preferencesVersion":3,"mode":"polished"}"#,
                PolishPolicy::Always,
            ),
        ] {
            let mut settings: Settings = serde_json::from_str(old).unwrap();
            settings.max_seconds = 120;
            settings.silence_seconds = 8;
            assert!(settings.migrate());
            assert_eq!(settings.polish_policy, expected);
            assert_eq!(settings.max_seconds, 120);
            assert_eq!(settings.silence_seconds, 8);
            assert!(!settings.migrate());
        }
        assert!(serde_json::from_str::<Settings>(r#"{"polishPolicy":"invalid"}"#).is_err());
    }
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
    fn transcription_deadline_defaults_and_bounds_survive_upgrade() {
        let mut settings: Settings = serde_json::from_str(r#"{"preferencesVersion":4}"#).unwrap();
        assert_eq!(settings.transcription_timeout_seconds, 5);
        for seconds in [0, 2, 121, u32::MAX] {
            settings.transcription_timeout_seconds = seconds;
            assert!(settings.validate().is_err());
        }
        for seconds in [3, 5, 120] {
            settings.transcription_timeout_seconds = seconds;
            assert!(settings.validate().is_ok());
        }
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
    #[test]
    fn upgrades_old_recording_defaults_and_preserves_custom_limits() {
        let mut old: Settings = serde_json::from_str(
            r#"{"preferencesVersion":2,"maxSeconds":120,"silenceSeconds":8,"insertion":"unicode"}"#,
        )
        .unwrap();
        assert!(old.migrate());
        assert_eq!(old.max_seconds, 600);
        assert_eq!(old.silence_seconds, 0);
        assert_eq!(old.insertion, "unicode");
        assert!(old.prepare_microphone);
        let mut custom: Settings = serde_json::from_str(
            r#"{"preferencesVersion":2,"maxSeconds":300,"silenceSeconds":20}"#,
        )
        .unwrap();
        custom.migrate();
        assert_eq!(custom.max_seconds, 300);
        assert_eq!(custom.silence_seconds, 20);
    }
}
