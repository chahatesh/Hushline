// SPDX-License-Identifier: MIT
//! Low-level hooks enqueue actions only. Audio, UI Automation, and HTTP never run here.
use crate::{
    model::Shortcuts,
    shortcuts::{self, Action, Chord, ALT, CTRL, SHIFT, WIN},
};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Mutex, OnceLock,
};
use windows::Win32::{
    Foundation::*,
    UI::{Accessibility::*, WindowsAndMessaging::*},
};
static EPOCH: AtomicU64 = AtomicU64::new(0);
static DICTATION_ACTIVE: AtomicBool = AtomicBool::new(false);
static CAPTURE_UNTIL: AtomicU64 = AtomicU64::new(0);
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
            if state.held.take().is_some() {
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
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), None, 0);
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), None, 0);
        match (keyboard, mouse) {
            (Ok(k), Ok(m)) => {
                let foreground = SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    None,
                    Some(focus_event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                let focus = SetWinEventHook(
                    EVENT_OBJECT_FOCUS,
                    EVENT_OBJECT_FOCUS,
                    None,
                    Some(focus_event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                let _ = ready_tx.send(Ok(()));
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = UnhookWindowsHookEx(k);
                let _ = UnhookWindowsHookEx(m);
                let _ = UnhookWinEvent(foreground);
                let _ = UnhookWinEvent(focus);
            }
            (k, m) => {
                if let Ok(k) = k {
                    let _ = UnhookWindowsHookEx(k);
                }
                if let Ok(m) = m {
                    let _ = UnhookWindowsHookEx(m);
                }
                let _=ready_tx.send(Err("Windows refused the global input hooks. Restart Hushline at normal user permissions.".into()));
            }
        }
    });
    ready_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .map_err(|_| "Shortcut handler did not start.")?
}
fn modifier(key: u16) -> bool {
    matches!(key,0x10..=0x12|0x5b..=0x5c|0xa0..=0xa5)
}
impl Keys {
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
        WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_MOUSEWHEEL | WM_MOUSEHWHEEL => true,
        _ => false,
    }
}
unsafe extern "system" fn focus_event(
    _: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    let root = if window.0.is_null() {
        window
    } else {
        GetAncestor(window, GA_ROOT)
    };
    if invalidates_focus(event, root, GetForegroundWindow()) {
        EPOCH.fetch_add(1, Ordering::SeqCst);
    }
}
fn invalidates_focus(event: u32, root: HWND, foreground: HWND) -> bool {
    event == EVENT_SYSTEM_FOREGROUND
        || (event == EVENT_OBJECT_FOCUS
            && (root.0.is_null() || foreground.0.is_null() || root == foreground))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clicking_the_nonactivating_pill_to_stop_does_not_revoke_paste() {
        assert!(!invalidates_mouse(WM_LBUTTONDOWN, || true));
        assert!(invalidates_mouse(WM_LBUTTONDOWN, || false));
        for message in [
            WM_RBUTTONDOWN,
            WM_MBUTTONDOWN,
            WM_MOUSEWHEEL,
            WM_MOUSEHWHEEL,
        ] {
            assert!(invalidates_mouse(message, || true));
            assert!(invalidates_mouse(message, || false));
        }
        assert!(!invalidates_mouse(WM_MOUSEMOVE, || false));
    }
    #[test]
    fn background_focus_notifications_do_not_revoke_the_active_field() {
        let active = HWND(123usize as *mut _);
        let background = HWND(456usize as *mut _);
        assert!(!invalidates_focus(EVENT_OBJECT_FOCUS, background, active));
        assert!(invalidates_focus(EVENT_OBJECT_FOCUS, active, active));
        assert!(invalidates_focus(
            EVENT_OBJECT_FOCUS,
            HWND::default(),
            active
        ));
        assert!(invalidates_focus(
            EVENT_SYSTEM_FOREGROUND,
            background,
            active
        ));
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
