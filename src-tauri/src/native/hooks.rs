// SPDX-License-Identifier: MIT
//! Low-level hooks enqueue actions only. Audio, UI Automation, and HTTP never run here.
use crate::{
    model::Shortcuts,
    shortcuts::{self, Action, Chord, ALT, CTRL, SHIFT, WIN},
};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    mpsc, Mutex, OnceLock,
};
use windows::Win32::{
    Foundation::*,
    System::Threading::GetCurrentThreadId,
    UI::{Accessibility::*, WindowsAndMessaging::*},
};
static EPOCH: AtomicU64 = AtomicU64::new(0);
static DICTATION_ACTIVE: AtomicBool = AtomicBool::new(false);
static CAPTURE_UNTIL: AtomicU64 = AtomicU64::new(0);
static HOOK_THREAD: AtomicU32 = AtomicU32::new(0);
const REFRESH_HOOKS: u32 = WM_APP + 41;
/// Event-driven recovery; never polls keys or installs hooks from an input callback.
pub fn refresh_hooks() {
    let thread = HOOK_THREAD.load(Ordering::Acquire);
    if thread != 0 {
        unsafe {
            let _ = PostThreadMessageW(thread, REFRESH_HOOKS, WPARAM(0), LPARAM(0));
        }
    }
}
fn monotonic_ms() -> u64 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
}
pub fn shortcut_capture(enabled: bool) {
    CAPTURE_UNTIL.store(
        if enabled { monotonic_ms() + 15_000 } else { 0 },
        Ordering::SeqCst,
    );
    if enabled {
        if let Some(lock) = STATE.get() {
            if let Ok(mut s) = lock.lock() {
                if s.held.take().is_some() {
                    if let Some(tx) = SENDER.get() {
                        let _ = tx.send(Action::Release);
                    }
                }
            }
        }
    }
}
pub fn set_active(value: bool) {
    DICTATION_ACTIVE.store(value, Ordering::SeqCst);
}
static SENDER: OnceLock<mpsc::Sender<Action>> = OnceLock::new();
static STATE: OnceLock<Mutex<Keys>> = OnceLock::new();
struct Keys {
    down: [bool; 256],
    bindings: Vec<(Chord, Action)>,
    held: Option<Chord>,
    consumed: [bool; 256],
}
pub fn input_epoch() -> u64 {
    EPOCH.load(Ordering::SeqCst)
}
pub fn update_shortcuts(s: &Shortcuts) {
    let bindings = shortcuts::bindings(s)
        .into_iter()
        .filter_map(|(c, a)| shortcuts::parse(c).ok().map(|c| (c, a)))
        .collect();
    if let Some(state) = STATE.get() {
        if let Ok(mut state) = state.lock() {
            state.bindings = bindings;
            if state.reset() {
                if let Some(tx) = SENDER.get() {
                    let _ = tx.send(Action::Release);
                }
            }
        }
    } else {
        let _ = STATE.set(Mutex::new(Keys {
            down: [false; 256],
            bindings,
            held: None,
            consumed: [false; 256],
        }));
    }
}
pub fn start_hooks(s: &Shortcuts, tx: mpsc::Sender<Action>) -> Result<(), String> {
    update_shortcuts(s);
    SENDER
        .set(tx)
        .map_err(|_| "Shortcut handler already started.")?;
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || unsafe {
        // PostThreadMessage requires the destination's message queue to exist.
        let mut msg = MSG::default();
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        HOOK_THREAD.store(GetCurrentThreadId(), Ordering::Release);
        let keyboard_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), None, 0);
        let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), None, 0);
        match (keyboard_hook, mouse_hook) {
            (Ok(mut k), Ok(mut m)) => {
                let foreground = SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    None,
                    Some(focus_event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                let desktop = SetWinEventHook(
                    EVENT_SYSTEM_DESKTOPSWITCH,
                    EVENT_SYSTEM_DESKTOPSWITCH,
                    None,
                    Some(focus_event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                let _ = ready_tx.send(Ok(()));
                while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                    if msg.message == REFRESH_HOOKS {
                        // Keep the old pair if Windows temporarily refuses a replacement.
                        let next_keyboard =
                            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), None, 0);
                        let next_mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), None, 0);
                        match (next_keyboard, next_mouse) {
                            (Ok(next_k), Ok(next_m)) => {
                                let _ = UnhookWindowsHookEx(k);
                                let _ = UnhookWindowsHookEx(m);
                                k = next_k;
                                m = next_m;
                                if STATE
                                    .get()
                                    .and_then(|s| s.lock().ok())
                                    .is_some_and(|mut s| s.reset())
                                {
                                    if let Some(tx) = SENDER.get() {
                                        let _ = tx.send(Action::Release);
                                    }
                                }
                            }
                            (next_k, next_m) => {
                                if let Ok(next_k) = next_k {
                                    let _ = UnhookWindowsHookEx(next_k);
                                }
                                if let Ok(next_m) = next_m {
                                    let _ = UnhookWindowsHookEx(next_m);
                                }
                            }
                        }
                        continue;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = UnhookWindowsHookEx(k);
                let _ = UnhookWindowsHookEx(m);
                let _ = UnhookWinEvent(foreground);
                let _ = UnhookWinEvent(desktop);
            }
            (k, m) => {
                if let Ok(k) = k {
                    let _ = UnhookWindowsHookEx(k);
                }
                if let Ok(m) = m {
                    let _ = UnhookWindowsHookEx(m);
                }
                let _ = ready_tx.send(Err(
                    "Windows refused the global shortcuts. Reopen Hush at normal user permissions."
                        .into(),
                ));
            }
        }
        HOOK_THREAD.store(0, Ordering::Release);
    });
    ready_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .map_err(|_| "Shortcut handler did not start.")?
}
fn modifier(key: u16) -> bool {
    matches!(key,0x10..=0x12|0x5b..=0x5c|0xa0..=0xa5)
}
impl Keys {
    fn reset(&mut self) -> bool {
        self.down.fill(false);
        self.consumed.fill(false);
        self.held.take().is_some()
    }
    fn modifiers(&self) -> u8 {
        let d = &self.down;
        (if d[0x11] || d[0xa2] || d[0xa3] {
            CTRL
        } else {
            0
        }) | (if d[0x5b] || d[0x5c] { WIN } else { 0 })
            | (if d[0x12] || d[0xa4] || d[0xa5] {
                ALT
            } else {
                0
            })
            | (if d[0x10] || d[0xa0] || d[0xa1] {
                SHIFT
            } else {
                0
            })
    }
    fn satisfied(&self, c: &Chord) -> bool {
        self.modifiers() == c.modifiers
            && match c.key {
                Some(k) => self.down[k as usize],
                None => !self
                    .down
                    .iter()
                    .enumerate()
                    .any(|(k, d)| *d && !modifier(k as u16)),
            }
    }
    fn route(&mut self, key: u16, down: bool, recording: bool) -> (bool, Vec<Action>) {
        if key > 255 {
            return (false, vec![]);
        }
        let index = key as usize;
        let repeated = self.down[index] == down;
        self.down[index] = down;
        let was_consumed = self.consumed[index];
        if !down {
            self.consumed[index] = false;
        }
        if repeated {
            return (was_consumed, vec![]);
        }
        let mut actions = Vec::with_capacity(1);
        // Actions start only on a fresh key DOWN. Releasing Space/Alt/V cannot
        // fall back to Ctrl+Win and accidentally start/stop another recording.
        let matched = if down {
            self.bindings
                .iter()
                .find(|(c, a)| {
                    if *a == Action::Cancel && c.key == Some(0x1b) {
                        return recording && key == 0x1b;
                    }
                    self.satisfied(c) && (c.key == Some(key) || modifier(key))
                })
                .cloned()
        } else {
            None
        };
        if let Some((chord, action)) = matched.as_ref() {
            if self.held.as_ref() != Some(chord) {
                actions.push(*action);
                self.held = if matches!(action, Action::Push | Action::Command) {
                    Some(chord.clone())
                } else {
                    None
                };
            }
            if !modifier(key) || matches!(key, 0x5b | 0x5c) {
                self.consumed[index] = true;
            }
        }
        if self.held.as_ref().is_some_and(|c| !self.satisfied(c)) {
            self.held = None;
            if !actions
                .iter()
                .any(|a| matches!(a, Action::HandsFree | Action::Command | Action::Cancel))
            {
                actions.push(Action::Release);
            }
        }
        (was_consumed || self.consumed[index], actions)
    }
}
fn handle(key: u16, down: bool) -> bool {
    let Some(lock) = STATE.get() else {
        return false;
    };
    let Ok(mut s) = lock.lock() else {
        return false;
    };
    if monotonic_ms() < CAPTURE_UNTIL.load(Ordering::SeqCst) && key <= 255 {
        s.down[key as usize] = down;
        s.held = None;
        let consumed = s.consumed[key as usize];
        if !down {
            s.consumed[key as usize] = false;
        }
        return consumed;
    }
    let previous = (key <= 255).then(|| s.down[key as usize]);
    let (consumed, actions) = s.route(key, down, DICTATION_ACTIVE.load(Ordering::SeqCst));
    for action in actions {
        if let Some(tx) = SENDER.get() {
            let _ = tx.send(action);
        }
    }
    if down && previous == Some(false) && !modifier(key) && !consumed {
        EPOCH.fetch_add(1, Ordering::SeqCst);
    }
    consumed
}
unsafe extern "system" fn keyboard(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(l.0 as *const KBDLLHOOKSTRUCT);
        if !(info.flags.contains(LLKHF_INJECTED) && info.dwExtraInfo == super::INPUT_TAG) {
            let down = w.0 == WM_KEYDOWN as usize || w.0 == WM_SYSKEYDOWN as usize;
            if handle(info.vkCode as u16, down) {
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, w, l)
}
unsafe extern "system" fn mouse(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(l.0 as *const MSLLHOOKSTRUCT);
        if !(info.flags & 1 != 0 && info.dwExtraInfo == super::INPUT_TAG) {
            if w.0 == WM_XBUTTONDOWN as usize || w.0 == WM_XBUTTONUP as usize {
                let key = if info.mouseData >> 16 == 1 { 5 } else { 6 };
                if handle(key, w.0 == WM_XBUTTONDOWN as usize) {
                    return LRESULT(1);
                }
            } else if invalidates_mouse(w.0 as u32, || super::overlay::contains_point(info.pt)) {
                EPOCH.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    CallNextHookEx(None, code, w, l)
}
fn invalidates_mouse(message: u32, overlay_hit: impl FnOnce() -> bool) -> bool {
    match message {
        // The overlay never activates a window or moves the original caret.
        // Stop must not revoke the very target it is about to deliver to.
        WM_LBUTTONDOWN => !overlay_hit(),
        WM_RBUTTONDOWN | WM_MBUTTONDOWN => true,
        // Scrolling does not move the editing caret. Fresh delivery inspection
        // still verifies the active window, editability and protection.
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => false,
        _ => false,
    }
}
unsafe extern "system" fn focus_event(
    _: HWINEVENTHOOK,
    event: u32,
    _: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    if event == EVENT_SYSTEM_DESKTOPSWITCH {
        EPOCH.fetch_add(1, Ordering::SeqCst);
        refresh_hooks();
        return;
    }
    if invalidates_focus(event) {
        EPOCH.fetch_add(1, Ordering::SeqCst);
    }
}
fn invalidates_focus(event: u32) -> bool {
    // Providers repeat EVENT_OBJECT_FOCUS during layout/activation even when
    // the editable field has not changed. The delivery inspection compares the
    // actual field/selection; physical input and foreground changes still fence it.
    event == EVENT_SYSTEM_FOREGROUND
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clicking_the_nonactivating_pill_to_stop_does_not_revoke_paste() {
        assert!(!invalidates_mouse(WM_LBUTTONDOWN, || true));
        assert!(invalidates_mouse(WM_LBUTTONDOWN, || false));
        for message in [WM_RBUTTONDOWN, WM_MBUTTONDOWN] {
            assert!(invalidates_mouse(message, || true));
            assert!(invalidates_mouse(message, || false));
        }
        assert!(!invalidates_mouse(WM_MOUSEWHEEL, || false));
        assert!(!invalidates_mouse(WM_MOUSEHWHEEL, || false));
        assert!(!invalidates_mouse(WM_MOUSEMOVE, || false));
    }
    #[test]
    fn provider_focus_notifications_do_not_revoke_an_unchanged_field() {
        assert!(!invalidates_focus(EVENT_OBJECT_FOCUS));
        assert!(invalidates_focus(EVENT_SYSTEM_FOREGROUND));
    }
    fn keys() -> Keys {
        Keys {
            down: [false; 256],
            consumed: [false; 256],
            held: None,
            bindings: shortcuts::bindings(&Shortcuts::default())
                .into_iter()
                .map(|(c, a)| (shortcuts::parse(c).unwrap(), a))
                .collect(),
        }
    }
    #[test]
    fn a_desktop_switch_clears_missed_releases_and_preserves_bindings() {
        let mut s = keys();
        s.route(0xa2, true, false);
        s.route(0x5b, true, false);
        assert!(s.reset());
        assert!(!s.reset(), "Only the held capture is released, once");
        assert!(s.down.iter().all(|key| !key));
        assert!(s.consumed.iter().all(|key| !key));
        s.route(0xa3, true, false);
        assert_eq!(s.route(0x5c, true, false).1, vec![Action::Push]);
        assert_eq!(s.route(0x5c, false, true).1, vec![Action::Release]);
        s.route(0xa3, false, false);
        s.route(0x20, true, false); // Lost Space key-up on an inactive desktop.
        s.reset();
        s.route(0xa2, true, false);
        assert_eq!(s.route(0x5b, true, false).1, vec![Action::Push]);
    }
    #[test]
    fn release_of_specific_chord_never_retriggers_push() {
        for (key, expected) in [
            (0x20, Action::HandsFree),
            (0xa4, Action::Command),
            (0x56, Action::Paste),
        ] {
            let mut s = keys();
            s.route(0xa2, true, false);
            assert_eq!(s.route(0x5b, true, false), (true, vec![Action::Push]));
            assert_eq!(s.route(key, true, true).1, vec![expected]);
            let up = s.route(key, false, true);
            assert!(!up.1.contains(&Action::Push));
            if expected == Action::Command {
                assert_eq!(up.1, vec![Action::Release]);
            } else {
                assert!(up.1.is_empty());
            }
            assert!(
                s.route(0x5b, false, true).0,
                "Win release must also be swallowed"
            );
            s.route(0xa2, false, false);
        }
    }
    #[test]
    fn processing_prefix_yields_once_to_open_copy_paste_and_prompt_without_release_retrigger() {
        for recording in [false, true] {
            for (key, expected) in [
                (0x48, Action::Open),
                (0x43, Action::Copy),
                (0x56, Action::Paste),
                (0x50, Action::ImprovePrompt),
            ] {
                // Modifier-only push is emitted as a prefix. The actor defers
                // that prefix; this fresh action must replace it exactly once.
                for modifier_order in [[0xa2, 0x5b], [0x5b, 0xa2]] {
                    let mut s = keys();
                    if expected == Action::ImprovePrompt {
                        for (chord, action) in &mut s.bindings {
                            if *action == expected {
                                *chord = shortcuts::parse("Ctrl+Win+P").unwrap();
                            }
                        }
                    }
                    assert!(s.route(modifier_order[0], true, recording).1.is_empty());
                    assert_eq!(
                        s.route(modifier_order[1], true, recording).1,
                        vec![Action::Push]
                    );
                    let (consumed, actions) = s.route(key, true, recording);
                    assert!(consumed);
                    assert_eq!(actions, vec![expected]);
                    assert!(
                        s.route(key, true, recording).1.is_empty(),
                        "A held primary key must not repeat the action"
                    );
                    assert!(
                        s.route(key, false, recording).1.is_empty(),
                        "Primary-key release must not restart Push"
                    );
                    assert!(s.route(modifier_order[1], false, recording).1.is_empty());
                    assert!(s.route(modifier_order[0], false, recording).1.is_empty());
                    assert!(s.held.is_none());
                    assert!(!s.down.iter().any(|down| *down));
                }
            }
        }
    }
    #[test]
    fn hold_releases_once_and_key_repeats_are_ignored() {
        let mut s = keys();
        s.route(0x5b, true, false);
        assert_eq!(s.route(0xa2, true, false).1, vec![Action::Push]);
        assert!(s.route(0xa2, true, true).1.is_empty());
        assert_eq!(s.route(0xa2, false, true).1, vec![Action::Release]);
        assert!(s.route(0x5b, false, false).1.is_empty());
    }
    #[test]
    fn prompt_shortcut_never_starts_a_microphone_or_opens_the_dashboard() {
        for modifiers in [[0xa2, 0xa0], [0xa1, 0xa3]] {
            let mut s = keys();
            for key in modifiers {
                assert!(s.route(key, true, false).1.is_empty());
            }
            assert_eq!(
                s.route(0x50, true, false),
                (true, vec![Action::ImprovePrompt])
            );
            assert!(s.route(0x50, true, false).1.is_empty());
            assert!(s.route(0x50, false, false).1.is_empty());
            for key in modifiers {
                assert!(s.route(key, false, false).1.is_empty());
            }
        }
    }
    #[test]
    fn idle_escape_passes_through_and_active_escape_cancels() {
        let mut s = keys();
        assert_eq!(s.route(0x1b, true, false), (false, vec![]));
        s.route(0x1b, false, false);
        s.route(0xa2, true, false);
        s.route(0x5b, true, false);
        assert_eq!(s.route(0x1b, true, true), (true, vec![Action::Cancel]));
        assert!(s.route(0x1b, false, false).0);
    }
    #[test]
    fn fast_repeated_press_release_is_not_lost() {
        let mut s = keys();
        for _ in 0..100 {
            s.route(0xa2, true, false);
            assert_eq!(s.route(0x5b, true, false).1, vec![Action::Push]);
            assert_eq!(s.route(0x5b, false, true).1, vec![Action::Release]);
            s.route(0xa2, false, false);
        }
    }
    #[test]
    fn space_and_short_taps_never_open_the_dashboard() {
        let mut s = keys();
        for _ in 0..20 {
            assert!(s.route(0x20, true, false).1.is_empty());
            assert!(s.route(0x20, true, false).1.is_empty());
            assert!(s.route(0x20, false, false).1.is_empty());
            for (key, down) in [
                (0xa2, true),
                (0x5b, true),
                (0x20, true),
                (0x20, false),
                (0x5b, false),
                (0xa2, false),
            ] {
                assert!(!s.route(key, down, true).1.contains(&Action::Open));
            }
        }
    }
}
