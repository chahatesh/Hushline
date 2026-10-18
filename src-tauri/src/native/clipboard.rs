// SPDX-License-Identifier: MIT
//! Clipboard insertion is optional. Unsupported formats cause a refusal before mutation.
use super::{capture_target, input_epoch, Target};
use std::{mem::size_of, time::Duration};
use windows::{
    core::w,
    Win32::{
        Foundation::*,
        System::{DataExchange::*, Memory::*},
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
};
type Result<T> = std::result::Result<T, String>;
struct Clipboard;
impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}
struct Owner(HWND);
impl Owner {
    fn new() -> Result<Self> {
        unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("HushlineClipboard"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
            .map(Self)
            .map_err(|_| "Could not create a clipboard owner.".into())
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}
fn open(hwnd: HWND) -> Result<Clipboard> {
    for _ in 0..10 {
        if unsafe { OpenClipboard(Some(hwnd)) }.is_ok() {
            return Ok(Clipboard);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Err("Clipboard is busy. Transcript preserved.".into())
}
unsafe fn set_raw(format: u32, bytes: &[u8]) -> Result<()> {
    let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1))
        .map_err(|_| "Could not allocate clipboard data.")?;
    let data = GlobalLock(memory);
    if data.is_null() {
        let _ = GlobalFree(Some(memory));
        return Err("Could not access clipboard memory.".into());
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), data as *mut u8, bytes.len());
    let _ = GlobalUnlock(memory);
    if SetClipboardData(format, Some(HANDLE(memory.0))).is_err() {
        let _ = GlobalFree(Some(memory));
        return Err("Could not set clipboard contents.".into());
    }
    Ok(())
}
fn bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(Some(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}
pub fn copy_text(text: &str) -> Result<()> {
    let owner = Owner::new()?;
    let _lock = open(owner.0)?;
    unsafe {
        EmptyClipboard().map_err(|_| "Could not clear the clipboard.")?;
        set_raw(13, &bytes(text))
    }
}
unsafe fn snapshot() -> Result<Vec<(u32, Vec<u8>)>> {
    let mut format = 0;
    let mut all = Vec::new();
    let mut total = 0;
    loop {
        format = EnumClipboardFormats(format);
        if format == 0 {
            break;
        }
        // Handles to GDI objects / owner-display data cannot be duplicated as global memory.
        if [2, 3, 9, 14, 0x80, 0x81, 0x82, 0x83, 0x8e].contains(&format) {
            return Err("Clipboard contains a format Hushline cannot restore safely. Choose Unicode insertion or copy the transcript manually.".into());
        }
        let handle = GetClipboardData(format)
            .map_err(|_| "Cannot snapshot the clipboard. Nothing was changed.")?;
        let memory = HGLOBAL(handle.0);
        let len = GlobalSize(memory);
        if len == 0 || len > 16_000_000 || total + len > 32_000_000 {
            return Err("Clipboard is too large or uses an unsupported format. Nothing was changed; use Unicode insertion.".into());
        }
        let data = GlobalLock(memory);
        if data.is_null() {
            return Err("Clipboard data cannot be preserved. Nothing was changed.".into());
        }
        let copy = std::slice::from_raw_parts(data as *const u8, len).to_vec();
        let _ = GlobalUnlock(memory);
        total += len;
        all.push((format, copy));
    }
    Ok(all)
}
unsafe fn restore(all: &[(u32, Vec<u8>)]) -> Result<()> {
    EmptyClipboard().map_err(|_| "Cannot restore the clipboard.")?;
    for (format, data) in all {
        set_raw(*format, data)?;
    }
    Ok(())
}
pub fn paste_text(
    target: &Target,
    text: &str,
    explicit: bool,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<String> {
    if cancel.is_cancelled() {
        return Err("Cancelled".into());
    }
    let owner = Owner::new()?;
    let lock = open(owner.0)?;
    let saved = unsafe { snapshot()? };
    unsafe {
        EmptyClipboard().map_err(|_| "Clipboard could not be prepared.")?;
        if let Err(e) = set_raw(13, &bytes(text)) {
            let _ = restore(&saved);
            return Err(e);
        }
    }
    let sequence = unsafe { GetClipboardSequenceNumber() };
    drop(lock);
    let current = capture_target(!target.selected.is_empty());
    let valid = current.is_ok_and(|c| {
        c.hwnd == target.hwnd
            && c.runtime == target.runtime
            && c.caret == target.caret
            && c.selected == target.selected
            && (explicit || input_epoch() == target.epoch)
    });
    let mut delivered = false;
    if valid && !cancel.is_cancelled() {
        let event = |vk, up| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    dwExtraInfo: super::INPUT_TAG,
                    ..Default::default()
                },
            },
        };
        let inputs = [
            event(0x11, false),
            event(0x56, false),
            event(0x56, true),
            event(0x11, true),
        ];
        delivered = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) } == 4;
    }
    std::thread::sleep(Duration::from_millis(600));
    let mut warning = String::new();
    if unsafe { GetClipboardSequenceNumber() } == sequence {
        match open(owner.0) {
            Ok(_lock) => {
                if unsafe { GetClipboardSequenceNumber() } == sequence
                    && unsafe { restore(&saved) }.is_err()
                {
                    warning=" Clipboard restoration failed; the transcript may still be on the clipboard.".into();
                }
            }
            Err(_) => {
                warning =
                    " Clipboard remained busy; the transcript may still be on the clipboard.".into()
            }
        }
    } else {
        warning = " Newer clipboard contents were preserved.".into();
    }
    if !delivered {
        return Err(format!(
            "Target changed or Windows blocked paste. Transcript preserved.{warning}"
        ));
    }
    Ok(format!("Paste shortcut delivered; the receiving application does not provide a delivery receipt.{warning}"))
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    struct Harness {
        parent: HWND,
        edit: HWND,
        thread: u32,
    }
    impl Harness {
        fn open(password: bool) -> Self {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || unsafe {
                let parent = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("STATIC"),
                    w!("Hushline native integration test"),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    120,
                    120,
                    640,
                    400,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
                let extra = if password { 0x20 } else { 0x4 | 0x40 };
                let edit = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("EDIT"),
                    w!(""),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | WS_TABSTOP | WINDOW_STYLE(extra),
                    10,
                    10,
                    600,
                    330,
                    Some(parent),
                    None,
                    None,
                    None,
                )
                .unwrap();
                let _ = SetForegroundWindow(parent);
                let _ = SetFocus(Some(edit));
                let thread = windows::Win32::System::Threading::GetCurrentThreadId();
                tx.send((parent.0 as isize, edit.0 as isize, thread))
                    .unwrap();
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = DestroyWindow(parent);
            });
            let (parent, edit, thread) = rx.recv_timeout(Duration::from_secs(3)).unwrap();
            std::thread::sleep(Duration::from_millis(250));
            Self {
                parent: HWND(parent as *mut _),
                edit: HWND(edit as *mut _),
                thread,
            }
        }
        fn text(&self) -> String {
            unsafe {
                let mut buffer = [0u16; 1024];
                let count = GetWindowTextW(self.edit, &mut buffer);
                String::from_utf16_lossy(&buffer[..count as usize])
            }
        }
    }
    impl Drop for Harness {
        fn drop(&mut self) {
            unsafe {
                let _ = PostThreadMessageW(self.thread, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }

    #[test]
    #[ignore = "Requires an interactive Windows desktop; opens owned test controls and temporarily uses clipboard."]
    fn native_input_clipboard_focus_and_password() {
        let previous = unsafe { GetForegroundWindow() };
        let owner = Owner::new().unwrap();
        let original = {
            let _lock = open(owner.0).unwrap();
            unsafe { snapshot().unwrap() }
        };
        // A guard restores only our sentinel; it never overwrites a user's newer copy.
        struct Restore {
            owner: Owner,
            data: Vec<(u32, Vec<u8>)>,
            sequence: u32,
            previous: HWND,
        }
        impl Drop for Restore {
            fn drop(&mut self) {
                if unsafe { GetClipboardSequenceNumber() } == self.sequence {
                    if let Ok(_lock) = open(self.owner.0) {
                        if unsafe { GetClipboardSequenceNumber() } == self.sequence {
                            let _ = unsafe { restore(&self.data) };
                        }
                    }
                }
                unsafe {
                    let _ = SetForegroundWindow(self.previous);
                }
            }
        }
        copy_text("hushline-test-sentinel").unwrap();
        let mut guard = Restore {
            owner,
            data: original,
            sequence: unsafe { GetClipboardSequenceNumber() },
            previous,
        };
        let h = Harness::open(false);
        assert_eq!(
            unsafe { GetForegroundWindow() },
            h.parent,
            "Interactive desktop focus could not be acquired"
        );
        let target = crate::native::capture_target(false).unwrap();
        crate::native::insert(
            &target,
            "Hello Hushline",
            "unicode",
            false,
            &CancellationToken::new(),
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.text(), "Hello Hushline");
        let target = crate::native::capture_target(false).unwrap();
        crate::native::insert(
            &target,
            " clipboard",
            "clipboard",
            false,
            &CancellationToken::new(),
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.text(), "Hello Hushline clipboard");
        {
            let _lock = open(guard.owner.0).unwrap();
            let saved = unsafe { snapshot().unwrap() };
            assert!(saved
                .iter()
                .any(|(f, b)| *f == 13 && b.starts_with(&bytes("hushline-test-sentinel"))));
        }
        guard.sequence = unsafe { GetClipboardSequenceNumber() };
        let token = CancellationToken::new();
        token.cancel();
        let target = crate::native::capture_target(false).unwrap();
        assert!(crate::native::insert(&target, "UNWANTED", "unicode", false, &token).is_err());
        assert!(!h.text().contains("UNWANTED"));
        let other = Harness::open(false);
        assert!(crate::native::insert(
            &target,
            "WRONG TARGET",
            "unicode",
            false,
            &CancellationToken::new()
        )
        .is_err());
        assert!(other.text().is_empty());
        let password = Harness::open(true);
        assert!(crate::native::capture_target(false)
            .unwrap_err()
            .contains("password"));
        assert!(password.text().is_empty());
    }
}
