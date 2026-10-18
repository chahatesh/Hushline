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
pub fn set_active(value: bool) {
    DICTATION_ACTIVE.store(value, Ordering::SeqCst);
}
static SENDER: OnceLock<mpsc::Sender<Action>> = OnceLock::new();
static STATE: OnceLock<Mutex<Keys>> = OnceLock::new();
struct Keys {
    down: [bool; 256],
    bindings: Vec<(Chord, Action)>,
    active: Option<Action>,
    push_active: bool,
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
            state.active = None;
            state.push_active = false;
        }
    } else {
        let _ = STATE.set(Mutex::new(Keys {
            down: [false; 256],
            bindings,
            active: None,
            push_active: false,
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
fn handle(key: u16, down: bool) -> bool {
    let Some(lock) = STATE.get() else {
        return false;
    };
    let Ok(mut s) = lock.lock() else {
        return false;
    };
    if key > 255 {
        return false;
    }
    let repeat = s.down[key as usize] == down;
    s.down[key as usize] = down;
    let d = &s.down;
    let mods = (if d[0x11] || d[0xa2] || d[0xa3] {
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
        });
    let matched = s
        .bindings
        .iter()
        .find(|(chord, action)| {
            if *action == Action::Cancel && chord.key == Some(0x1b) {
                return DICTATION_ACTIVE.load(Ordering::SeqCst) && s.down[0x1b];
            }
            chord.modifiers == mods
                && match chord.key {
                    Some(k) => s.down[k as usize],
                    None => !s
                        .down
                        .iter()
                        .enumerate()
                        .any(|(k, d)| *d && !modifier(k as u16)),
                }
        })
        .map(|(_, a)| *a);
    if !repeat && matched != s.active {
        if let Some(action) = matched {
            if let Some(tx) = SENDER.get() {
                let _ = tx.send(action);
            }
            if action == Action::Push {
                s.push_active = true;
            }
        } else if s.push_active {
            if let Some(tx) = SENDER.get() {
                let _ = tx.send(Action::Release);
            }
            s.push_active = false;
        }
        s.active = matched;
    }
    let consumed = matched.is_some() && !modifier(key);
    if down && !repeat && !modifier(key) && !consumed {
        EPOCH.fetch_add(1, Ordering::SeqCst);
    }
    // Suppress the Win key while a Hushline chord is active to avoid opening Start.
    consumed || ((key == 0x5b || key == 0x5c) && (mods & CTRL != 0 || s.push_active))
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
            } else if [
                WM_LBUTTONDOWN,
                WM_RBUTTONDOWN,
                WM_MBUTTONDOWN,
                WM_MOUSEWHEEL,
                WM_MOUSEHWHEEL,
            ]
            .contains(&(w.0 as u32))
            {
                EPOCH.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    CallNextHookEx(None, code, w, l)
}
unsafe extern "system" fn focus_event(
    _: HWINEVENTHOOK,
    _: u32,
    _: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    EPOCH.fetch_add(1, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_router_cancels_with_modifiers_and_leaves_idle_escape_alone() {
        let (tx, rx) = mpsc::channel();
        let _ = SENDER.set(tx);
        update_shortcuts(&Shortcuts::default());
        set_active(false);
        assert!(!handle(0x1b, true));
        handle(0x1b, false);
        assert!(rx.try_recv().is_err());
        handle(0xa2, true);
        assert!(handle(0x5b, true));
        assert_eq!(rx.recv().unwrap(), Action::Push);
        set_active(true);
        assert!(handle(0x20, true));
        assert_eq!(rx.recv().unwrap(), Action::HandsFree);
        handle(0x20, false);
        while rx.try_recv().is_ok() {}
        assert!(handle(0x1b, true));
        assert_eq!(rx.recv().unwrap(), Action::Cancel);
        handle(0x1b, false);
        handle(0x5b, false);
        handle(0xa2, false);
        set_active(false);
    }
}
