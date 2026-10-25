// SPDX-License-Identifier: MIT
//! Session metadata only. Neither transcript contents nor audio enter diagnostics.
use crate::{model::StageDiagnostic, shortcuts::Action};
pub const RECOVERY_NOTICE_MS: u64 = 12_000;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Instant,
};
#[derive(Default)]
pub struct Diagnostics(Mutex<Trace>);
#[derive(Default)]
pub struct VisibilityClock(AtomicU64);
impl VisibilityClock {
    pub fn renew(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed) + 1
    }
    pub fn current(&self, ticket: u64) -> bool {
        self.0.load(Ordering::Relaxed) == ticket
    }
}
#[derive(Default)]
struct Trace {
    starts: BTreeMap<u64, Instant>,
    events: VecDeque<StageDiagnostic>,
}
impl Diagnostics {
    pub fn record(&self, generation: u64, stage: &str, outcome: &str) {
        let stage = match stage {
            "arming" | "microphone" | "recording" | "preparing_audio" | "transcribing"
            | "polishing" | "inserting" | "prompt_editing" | "target_inspection" => stage,
            _ => "idle",
        };
        let outcome = match outcome {
            "starting"
            | "listening"
            | "processing"
            | "preview"
            | "success"
            | "error"
            | "completed"
            | "cancelled"
            | "field_changed"
            | "protected_input"
            | "unsupported_field"
            | "clipboard_busy"
            | "clipboard_unsupported"
            | "native_timeout"
            | "local_fallback"
            | "native_error" => outcome,
            _ => "idle",
        };
        let mut trace = self.0.lock().unwrap();
        if trace
            .events
            .back()
            .is_some_and(|e| e.generation == generation && e.stage == stage && e.outcome == outcome)
        {
            return;
        }
        let elapsed_ms = trace
            .starts
            .entry(generation)
            .or_insert_with(Instant::now)
            .elapsed()
            .as_millis() as u64;
        trace.events.push_back(StageDiagnostic {
            generation,
            stage: stage.into(),
            elapsed_ms,
            outcome: outcome.into(),
        });
        while trace.starts.len() > 50 {
            if let Some((old, _)) = trace.starts.pop_first() {
                trace.events.retain(|e| e.generation != old);
            }
        }
        while trace.events.len() > 200 {
            trace.events.pop_front();
        }
    }
    pub fn list(&self) -> Vec<StageDiagnostic> {
        self.0.lock().unwrap().events.iter().cloned().collect()
    }
}
/// Strict, content-free reason codes. Never return any part of an error payload.
pub fn native_outcome<T>(result: &Result<T, String>) -> &'static str {
    let Err(error) = result else {
        return "completed";
    };
    let error = error.to_ascii_lowercase();
    if error == "cancelled" {
        "cancelled"
    } else if error.contains("took too long") || error.contains("timed out") {
        "native_timeout"
    } else if error.contains("password")
        || error.contains("read-only")
        || error.contains("protected")
    {
        "protected_input"
    } else if error.contains("clipboard")
        && (error.contains("unsupported")
            || error.contains("cannot restore")
            || error.contains("too large"))
    {
        "clipboard_unsupported"
    } else if error.contains("clipboard") && error.contains("busy") {
        "clipboard_busy"
    } else if error.contains("clipboard") {
        "native_error"
    } else if error.contains("changed")
        || error.contains("owning window")
        || error.contains("active window")
    {
        "field_changed"
    } else if error.contains("editable")
        || error.contains("field type")
        || error.contains("focused field")
    {
        "unsupported_field"
    } else {
        "native_error"
    }
}
pub fn aborts_processing(action: Action) -> bool {
    action == Action::Cancel
}
pub fn finish_current_before_recording(phase: &str, stopping: bool, action: Action) -> bool {
    (phase == "processing" || stopping)
        && matches!(action, Action::Push | Action::HandsFree | Action::Command)
}
pub fn blocks_delivery(action: Action) -> bool {
    matches!(
        action,
        Action::Cancel
            | Action::Open
            | Action::Scratchpad
            | Action::Copy
            | Action::Paste
            | Action::ImprovePrompt
    )
}
pub fn may_start(held: bool, hands_free: bool) -> bool {
    held || hands_free
}
/// Latest manual recovery action owns feedback; an older timeout cannot replace
/// a newer paste/copy result or a new recording's status.
#[derive(Default)]
pub struct FeedbackGate {
    next: u64,
    pending: Option<(u64, u64, bool)>,
}
impl FeedbackGate {
    pub fn begin(&mut self, generation: u64, standalone: bool) -> u64 {
        let standalone = standalone || self.pending.is_some_and(|(g, _, s)| g == generation && s);
        self.next += 1;
        self.pending = Some((generation, self.next, standalone));
        self.next
    }
    pub fn complete(&mut self, generation: u64, operation: u64) -> Option<bool> {
        if self
            .pending
            .is_some_and(|(g, o, _)| g == generation && o == operation)
        {
            self.pending.take().map(|(_, _, standalone)| standalone)
        } else {
            None
        }
    }
}
/// A completed hold is never a tap, even if the next recording starts quickly.
pub fn is_short_tap(elapsed: std::time::Duration) -> bool {
    elapsed <= std::time::Duration::from_millis(250)
}
#[derive(Debug, PartialEq, Eq)]
pub enum ReleaseIntent {
    Stop,
    KeepRecording,
    StartHandsFree,
}
pub fn release_intent(
    hands_free: bool,
    stop_on_release: bool,
    double_candidate: bool,
    elapsed: std::time::Duration,
) -> ReleaseIntent {
    if stop_on_release {
        ReleaseIntent::Stop
    } else if hands_free {
        ReleaseIntent::KeepRecording
    } else if double_candidate && is_short_tap(elapsed) {
        ReleaseIntent::StartHandsFree
    } else {
        ReleaseIntent::Stop
    }
}
/// A modifier-only chord must not cancel inference before its final key arrives.
#[derive(Default)]
pub struct PushGate {
    next: u64,
    pending: Option<(u64, u64)>,
}
impl PushGate {
    pub fn defer(&mut self, generation: u64) -> u64 {
        self.next += 1;
        self.pending = Some((self.next, generation));
        self.next
    }
    pub fn resolve(&mut self, id: u64, generation: u64) -> bool {
        if self.pending == Some((id, generation)) {
            self.pending = None;
            true
        } else {
            false
        }
    }
    pub fn clear(&mut self) {
        self.pending = None;
    }
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_record_shortcuts_cannot_discard_inflight_transcripts() {
        for action in [Action::Push, Action::HandsFree, Action::Command] {
            assert!(finish_current_before_recording("processing", false, action));
            assert!(finish_current_before_recording("listening", true, action));
            assert!(!finish_current_before_recording("idle", false, action));
            assert!(!finish_current_before_recording("listening", false, action));
            assert!(!finish_current_before_recording("preview", false, action));
        }
        assert!(!finish_current_before_recording(
            "processing",
            true,
            Action::Cancel
        ));
    }
    #[test]
    fn an_old_error_timer_cannot_hide_a_new_copy_or_paste_status() {
        let clock = VisibilityClock::default();
        let error = clock.renew();
        let pasted = clock.renew();
        assert!(!clock.current(error));
        assert!(clock.current(pasted));
        clock.renew();
        assert!(!clock.current(pasted));
    }
    #[test]
    fn stale_recovery_feedback_cannot_replace_a_new_paste_or_recording() {
        let mut gate = FeedbackGate::default();
        let old = gate.begin(1, true);
        let new = gate.begin(1, false);
        assert_eq!(gate.complete(1, old), None);
        assert_eq!(gate.complete(1, new), Some(true));
        assert_eq!(gate.complete(1, new), None);
        let old = gate.begin(1, true);
        let fresh = gate.begin(2, false);
        assert_eq!(gate.complete(1, old), None);
        assert_eq!(gate.complete(2, fresh), Some(false));
    }
    #[test]
    fn native_failure_diagnostics_include_only_fixed_reason_codes() {
        assert_eq!(native_outcome(&Ok::<_, String>(())), "completed");
        for (error, expected) in [
            ("Cancelled", "cancelled"),
            ("Focus or cursor changed", "field_changed"),
            (
                "Dictation is disabled in password fields",
                "protected_input",
            ),
            ("Clipboard is busy", "clipboard_busy"),
            (
                "Clipboard uses an unsupported format",
                "clipboard_unsupported",
            ),
            ("The field took too long to respond", "native_timeout"),
            ("Focus an editable field", "unsupported_field"),
            ("private text or secret-key", "native_error"),
            (
                "Cannot snapshot the clipboard. Nothing was changed.",
                "native_error",
            ),
        ] {
            assert_eq!(native_outcome::<()>(&Err(error.into())), expected);
        }
    }
    #[test]
    fn second_hold_release_finishes_instead_of_latching_hands_free() {
        use std::time::Duration as D;
        assert!(!is_short_tap(D::from_secs(2)));
        assert_eq!(
            release_intent(false, false, true, D::from_secs(2)),
            ReleaseIntent::Stop
        );
        assert_eq!(
            release_intent(true, true, false, D::from_millis(20)),
            ReleaseIntent::Stop
        );
        assert_eq!(
            release_intent(true, true, false, D::from_secs(2)),
            ReleaseIntent::Stop
        );
        assert_eq!(
            release_intent(true, false, false, D::from_millis(20)),
            ReleaseIntent::KeepRecording
        );
        assert_eq!(
            release_intent(false, false, true, D::from_millis(100)),
            ReleaseIntent::StartHandsFree
        );
        assert_eq!(
            release_intent(false, false, false, D::from_millis(100)),
            ReleaseIntent::Stop
        );
    }
    #[test]
    fn opening_dashboard_and_copying_do_not_cancel_transcription() {
        for action in [
            Action::Open,
            Action::Scratchpad,
            Action::Copy,
            Action::Paste,
        ] {
            assert!(!aborts_processing(action));
            assert!(blocks_delivery(action));
        }
        assert!(aborts_processing(Action::Cancel));
    }
    #[test]
    fn chord_prefix_cannot_restart_after_copy_or_a_new_job() {
        let mut gate = PushGate::default();
        let id = gate.defer(1);
        gate.clear();
        assert!(!gate.resolve(id, 1));
        let id = gate.defer(1);
        assert!(!gate.resolve(id, 2));
        let old = gate.defer(2);
        let new = gate.defer(2);
        assert!(!gate.resolve(old, 2));
        assert!(gate.resolve(new, 2));
        assert!(!gate.resolve(new, 2));
        assert!(!aborts_processing(Action::Push));
    }
    #[test]
    fn released_start_cannot_start_later_but_hands_free_can() {
        assert!(!may_start(false, false));
        assert!(may_start(true, false));
        assert!(may_start(false, true));
    }
    #[test]
    fn diagnostics_are_bounded_and_exclude_arbitrary_content() {
        let d = Diagnostics::default();
        for id in 0..100 {
            d.record(id, "transcribing", "processing");
            d.record(id, "sensitive transcript", "secret-key");
        }
        let events = d.list();
        assert!(events.len() <= 200);
        assert!(events.iter().all(|e| e.generation >= 50));
        let json = serde_json::to_string(&events).unwrap();
        assert!(!json.contains("sensitive") && !json.contains("secret"));
    }
}
