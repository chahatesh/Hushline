// SPDX-License-Identifier: MIT
//! Clipboard insertion is optional. Unsupported formats cause a refusal before mutation.
use super::{capture_target, input_epoch, Target};
use std::{
    mem::size_of,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use windows::{
    core::w,
    Win32::{
        Foundation::*,
        System::{DataExchange::*, Memory::*},
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
};
type Result<T> = std::result::Result<T, String>;
const MAX_PASTE_WORKERS: usize = 2;
static PASTE_WORKERS: AtomicUsize = AtomicUsize::new(0);
struct WorkerPermit<'a>(&'a AtomicUsize);
impl<'a> WorkerPermit<'a> {
    fn acquire(active: &'a AtomicUsize) -> Option<Self> {
        let mut count = active.load(Ordering::Acquire);
        loop {
            if count >= MAX_PASTE_WORKERS {
                return None;
            }
            match active.compare_exchange_weak(
                count,
                count + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(Self(active)),
                Err(actual) => count = actual,
            }
        }
    }
}
impl Drop for WorkerPermit<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
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
struct PendingRestore {
    owner: Owner,
    saved: Vec<(u32, Vec<u8>)>,
    sequence: u32,
    delivered: bool,
}
pub fn paste_text(
    target: &Target,
    text: &str,
    explicit: bool,
    cancel: &tokio_util::sync::CancellationToken,
    notice: Option<Box<dyn FnOnce(String) + Send>>,
) -> Result<String> {
    let permit = WorkerPermit::acquire(&PASTE_WORKERS).ok_or(
        "Windows clipboard editing is still busy. Your text is preserved; try again after the field responds.",
    )?;
    let target = target.clone();
    let text = text.to_string();
    let token = cancel.child_token();
    let worker_token = token.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("hush-clipboard".into())
        .spawn(move || {
            // The caller may time out while an out-of-process UIA provider is
            // stalled. Hold the permit until this actual worker and delayed
            // restoration finish, not just until the caller stops waiting.
            let _permit = permit;
            match prepare_paste(&target, &text, explicit, &worker_token) {
                Err(error) => {
                    let _ = tx.send(Err(error));
                }
                Ok(pending) => {
                    let _ = tx.send(if pending.delivered {
                        Ok(String::new())
                    } else {
                        Err(
                        "The field changed or Windows blocked paste. Your text is ready to copy."
                            .into(),
                    )
                    });
                    let warning = restore_later(pending);
                    if let Some(notice) = notice {
                        notice(warning);
                    }
                }
            }
        })
        .map_err(|_| "Windows clipboard editing could not start. Your text is preserved.")?;
    match rx.recv_timeout(Duration::from_millis(1800)) {
        Ok(result) => result,
        Err(_) => {
            token.cancel();
            Err("This field took too long to respond. Your text is ready to copy.".into())
        }
    }
}
fn prepare_paste(
    target: &Target,
    text: &str,
    explicit: bool,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<PendingRestore> {
    if cancel.is_cancelled() {
        return Err("Cancelled".into());
    }
    // A slow/out-of-process accessibility provider must never hold or replace
    // the user's clipboard while it is being inspected.
    let current = capture_target(!target.selected.is_empty())?;
    if current.hwnd != target.hwnd
        || current.runtime != target.runtime
        || current.runtime.is_empty()
        || current.caret != target.caret
        || current.selected != target.selected
        || (!explicit && current.epoch != target.epoch)
    {
        return Err("The field changed before insertion. Your text is ready to copy.".into());
    }
    if cancel.is_cancelled() {
        return Err("Cancelled".into());
    }
    let owner = Owner::new()?;
    let lock = open(owner.0)?;
    let saved = unsafe { snapshot()? };
    if cancel.is_cancelled() {
        return Err("Cancelled".into());
    }
    unsafe {
        EmptyClipboard().map_err(|_| "Clipboard could not be prepared.")?;
        if let Err(e) = set_raw(13, &bytes(text)) {
            let _ = restore(&saved);
            return Err(e);
        }
    }
    let sequence = unsafe { GetClipboardSequenceNumber() };
    drop(lock);
    // Only fast checks remain once the clipboard is prepared. Recheck the epoch
    // even for an explicitly approved preview: input after inspection revokes
    // that approval for this field. Never paste a newer user copy by accident.
    let valid = input_epoch() == current.epoch
        && unsafe { GetForegroundWindow() }.0 as isize == current.hwnd
        && unsafe { GetClipboardSequenceNumber() } == sequence;
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
    Ok(PendingRestore {
        owner,
        saved,
        sequence,
        delivered,
    })
}
fn restore_later(pending: PendingRestore) -> String {
    let PendingRestore {
        owner,
        saved,
        sequence,
        ..
    } = pending;
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
    }
    warning
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn clipboard_worker_capacity_is_bounded_until_actual_worker_release() {
        let active = AtomicUsize::new(0);
        let first = WorkerPermit::acquire(&active).unwrap();
        let second = WorkerPermit::acquire(&active).unwrap();
        assert!(WorkerPermit::acquire(&active).is_none());
        assert_eq!(active.load(Ordering::Acquire), MAX_PASTE_WORKERS);
        drop(first);
        let replacement = WorkerPermit::acquire(&active).unwrap();
        assert!(WorkerPermit::acquire(&active).is_none());
        drop(second);
        drop(replacement);
        assert_eq!(active.load(Ordering::Acquire), 0);
    }

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
        let (cleanup_tx, cleanup_rx) = std::sync::mpsc::channel();
        crate::native::insert_with_notice(
            &target,
            " clipboard",
            "clipboard",
            false,
            &CancellationToken::new(),
            Some(Box::new(move |warning| {
                let _ = cleanup_tx.send(warning);
            })),
        )
        .unwrap();
        assert!(cleanup_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .is_empty());
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

    #[test]
    #[ignore = "Requires an interactive desktop; opens owned edit controls and verifies whole-field replacement without submitting."]
    fn native_whole_field_capture_approval_and_changed_text() {
        struct Focus(HWND);
        impl Drop for Focus {
            fn drop(&mut self) {
                unsafe {
                    let _ = SetForegroundWindow(self.0);
                }
            }
        }
        let _focus = Focus(unsafe { GetForegroundWindow() });
        let h = Harness::open(false);
        assert_eq!(
            unsafe { GetForegroundWindow() },
            h.parent,
            "Interactive desktop focus could not be acquired"
        );
        unsafe {
            SetWindowTextW(
                h.edit,
                w!("Make a useful plan.\r\nKeep this second paragraph."),
            )
            .unwrap();
        }
        let captured = crate::native::capture_whole_field().unwrap();
        assert!(captured.original.contains("Keep this second paragraph."));
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(crate::native::replace_whole_field(&captured, "UNWANTED", &cancelled).is_err());
        assert_eq!(h.text(), captured.original);
        crate::native::replace_whole_field(
            &captured,
            "Please make a concise plan.\r\nPreserve the details.",
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(
            h.text(),
            "Please make a concise plan.\r\nPreserve the details."
        );
        let captured = crate::native::capture_whole_field().unwrap();
        unsafe {
            SetWindowTextW(h.edit, w!("Newer user edit")).unwrap();
        }
        assert!(crate::native::replace_whole_field(
            &captured,
            "UNWANTED",
            &CancellationToken::new()
        )
        .is_err());
        assert_eq!(h.text(), "Newer user edit");
        unsafe {
            let _ = SendMessageW(h.edit, 0x00CF, Some(WPARAM(1)), Some(LPARAM(0)));
        } // EM_SETREADONLY
        assert!(crate::native::capture_whole_field().is_err());
        let password = Harness::open(true);
        assert!(crate::native::capture_whole_field().is_err());
        assert!(password.text().is_empty());
    }
}
