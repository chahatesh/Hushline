// SPDX-License-Identifier: MIT
//! Session metadata only. Neither transcript contents nor audio enter diagnostics.
use crate::{model::StageDiagnostic, shortcuts::Action};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Mutex,
    time::Instant,
};
#[derive(Default)]
pub struct Diagnostics(Mutex<Trace>);
#[derive(Default)]
struct Trace {
    starts: BTreeMap<u64, Instant>,
    events: VecDeque<StageDiagnostic>,
}
impl Diagnostics {
    pub fn record(&self, generation: u64, stage: &str, outcome: &str) {
        let stage = match stage {
            "arming" | "microphone" | "recording" | "preparing_audio" | "transcribing"
            | "polishing" | "inserting" | "prompt_editing" => stage,
            _ => "idle",
        };
        let outcome = match outcome {
            "starting" | "listening" | "processing" | "preview" | "success" | "error" => outcome,
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
pub fn aborts_processing(action: Action) -> bool {
    action == Action::Cancel
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
