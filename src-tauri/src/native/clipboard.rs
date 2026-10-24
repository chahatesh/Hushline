// SPDX-License-Identifier: MIT
//! Bounded clipboard preservation, including native images. Unknown handle types
//! cause a refusal before mutation, never a destructive best-effort snapshot.
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
        Graphics::Gdi::*,
        System::{DataExchange::*, Memory::*},
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
};
type Result<T> = std::result::Result<T, String>;
const MAX_FORMAT_BYTES: usize = 32 * 1024 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 96 * 1024 * 1024;
type Saved = Vec<(u32, SavedData)>;
#[derive(Debug)]
enum SavedData {
    Bytes(Vec<u8>),
    Bitmap(HBITMAP),
    Enhanced(HENHMETAFILE),
}
impl Drop for SavedData {
    fn drop(&mut self) {
        unsafe {
            match self {
                Self::Bitmap(bitmap) => {
                    let _ = DeleteObject(HGDIOBJ(bitmap.0));
                }
                Self::Enhanced(metafile) => {
                    let _ = DeleteEnhMetaFile(Some(*metafile));
                }
                Self::Bytes(_) => {}
            }
        }
    }
}
fn fits_snapshot(size: usize, total: usize) -> bool {
    size > 0
        && size <= MAX_FORMAT_BYTES
        && total
            .checked_add(size)
            .is_some_and(|n| n <= MAX_SNAPSHOT_BYTES)
}
unsafe fn clone_bitmap(bitmap: HBITMAP) -> Result<HBITMAP> {
    CopyImage(HANDLE(bitmap.0), IMAGE_BITMAP, 0, 0, LR_CREATEDIBSECTION)
        .map(|handle| HBITMAP(handle.0))
        .map_err(|_| "Clipboard image could not be preserved. Nothing was changed.".into())
}
unsafe fn clone_enhanced(metafile: HENHMETAFILE) -> Result<HENHMETAFILE> {
    let cloned = CopyEnhMetaFileW(metafile, windows::core::PCWSTR::null());
    if cloned.0.is_null() {
        Err("Clipboard drawing could not be preserved. Nothing was changed.".into())
    } else {
        Ok(cloned)
    }
}
impl SavedData {
    unsafe fn publish(&self, format: u32) -> Result<()> {
        // Keep the saved object alive; transfer a separate clone to Windows.
        let copy = match self {
            Self::Bytes(data) => return set_raw(format, data),
            Self::Bitmap(bitmap) => Self::Bitmap(clone_bitmap(*bitmap)?),
            Self::Enhanced(metafile) => Self::Enhanced(clone_enhanced(*metafile)?),
        };
        let handle = match &copy {
            Self::Bitmap(bitmap) => HANDLE(bitmap.0),
            Self::Enhanced(metafile) => HANDLE(metafile.0),
            _ => unreachable!(),
        };
        SetClipboardData(format, Some(handle))
            .map_err(|_| "Could not restore the clipboard image.")?;
        std::mem::forget(copy); // Windows owns the published GDI object now.
        Ok(())
    }
    #[cfg(test)]
    fn starts_with(&self, prefix: &[u8]) -> bool {
        matches!(self, Self::Bytes(data) if data.starts_with(prefix))
    }
}
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
unsafe fn snapshot() -> Result<Saved> {
    let mut format = 0;
    let mut all = Vec::new();
    let mut total = 0;
    loop {
        format = EnumClipboardFormats(format);
        if format == 0 {
            break;
        }
        // Owner-display, classic metafile pictures, palettes and private handle
        // formats need their own ownership protocols; never treat them as bytes.
        if [3, 9, 0x80, 0x83].contains(&format) || (0x200..0x400).contains(&format) {
            return Err("Clipboard contains an unsupported format that cannot be restored safely. Your transcript is ready to copy.".into());
        }
        let handle = GetClipboardData(format)
            .map_err(|_| "Cannot snapshot the clipboard. Nothing was changed.")?;
        if matches!(format, 2 | 0x82) {
            let mut info = BITMAP::default();
            if GetObjectW(
                HGDIOBJ(handle.0),
                size_of::<BITMAP>() as i32,
                Some(&mut info as *mut _ as *mut _),
            ) == 0
            {
                return Err("Clipboard image could not be inspected. Nothing was changed.".into());
            }
            let len = (info.bmWidthBytes.unsigned_abs() as usize)
                .saturating_mul(info.bmHeight.unsigned_abs() as usize)
                .saturating_mul(info.bmPlanes as usize);
            if !fits_snapshot(len, total) {
                return Err(
                    "Clipboard image is too large to preserve safely. Nothing was changed.".into(),
                );
            }
            all.push((format, SavedData::Bitmap(clone_bitmap(HBITMAP(handle.0))?)));
            total += len;
            continue;
        }
        if matches!(format, 14 | 0x8e) {
            let len = GetEnhMetaFileBits(HENHMETAFILE(handle.0), None) as usize;
            if !fits_snapshot(len, total) {
                return Err(
                    "Clipboard drawing is too large or unsupported. Nothing was changed.".into(),
                );
            }
            all.push((
                format,
                SavedData::Enhanced(clone_enhanced(HENHMETAFILE(handle.0))?),
            ));
            total += len;
            continue;
        }
        let memory = HGLOBAL(handle.0);
        let len = GlobalSize(memory);
        if !fits_snapshot(len, total) {
            return Err("Clipboard is too large or uses an unsupported format. Nothing was changed; use Unicode insertion.".into());
        }
        let data = GlobalLock(memory);
        if data.is_null() {
            return Err("Clipboard data cannot be preserved. Nothing was changed.".into());
        }
        let copy = std::slice::from_raw_parts(data as *const u8, len).to_vec();
        let _ = GlobalUnlock(memory);
        total += len;
        all.push((format, SavedData::Bytes(copy)));
    }
    Ok(all)
}
unsafe fn restore(all: &[(u32, SavedData)]) -> Result<()> {
    EmptyClipboard().map_err(|_| "Cannot restore the clipboard.")?;
    for (format, data) in all {
        data.publish(*format)?;
    }
    Ok(())
}
struct PendingRestore {
    owner: Owner,
    saved: Saved,
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
    if !super::windows::target_matches(target, &current, explicit) {
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
    drop(lock);
    // CloseClipboard completes publication and can synthesize additional formats.
    // Track the published clipboard, then verify ownership too so a newer user
    // copy cannot be mistaken for our transcript during this handoff.
    let sequence = unsafe { GetClipboardSequenceNumber() };
    // Only fast checks remain once the clipboard is prepared. Recheck the epoch
    // even for an explicitly approved preview: input after inspection revokes
    // that approval for this field. Never paste a newer user copy by accident.
    let valid = input_epoch() == current.epoch
        && unsafe { GetForegroundWindow() }.0 as isize == current.hwnd
        && publication_matches(
            owner.0,
            unsafe { GetClipboardOwner() }.unwrap_or_default(),
            sequence,
            unsafe { GetClipboardSequenceNumber() },
        );
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
    if publication_matches(
        owner.0,
        unsafe { GetClipboardOwner() }.unwrap_or_default(),
        sequence,
        unsafe { GetClipboardSequenceNumber() },
    ) {
        match open(owner.0) {
            Ok(_lock) => {
                if publication_matches(
                    owner.0,
                    unsafe { GetClipboardOwner() }.unwrap_or_default(),
                    sequence,
                    unsafe { GetClipboardSequenceNumber() },
                ) && unsafe { restore(&saved) }.is_err()
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
fn publication_matches(
    owner: HWND,
    current_owner: HWND,
    sequence: u32,
    current_sequence: u32,
) -> bool {
    !owner.0.is_null() && owner == current_owner && sequence == current_sequence
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::mpsc;
    use tokio_util::sync::CancellationToken;
    #[test]
    fn snapshot_budget_is_bounded_even_for_overflowing_metadata() {
        assert!(fits_snapshot(32 * 1024 * 1024, 64 * 1024 * 1024));
        assert!(!fits_snapshot(0, 0));
        assert!(!fits_snapshot(MAX_FORMAT_BYTES + 1, 0));
        assert!(!fits_snapshot(1, MAX_SNAPSHOT_BYTES));
        assert!(!fits_snapshot(1024, usize::MAX));
    }
    #[test]
    fn enhanced_drawing_copy_keeps_its_records_after_original_is_freed() {
        unsafe {
            let dc = CreateEnhMetaFileW(
                None,
                windows::core::PCWSTR::null(),
                None,
                windows::core::PCWSTR::null(),
            );
            assert!(!dc.0.is_null());
            let _ = Rectangle(dc, 1, 1, 20, 20);
            let original = SavedData::Enhanced(CloseEnhMetaFile(dc));
            let SavedData::Enhanced(handle) = original else {
                unreachable!()
            };
            let copy = SavedData::Enhanced(clone_enhanced(handle).unwrap());
            let SavedData::Enhanced(cloned) = copy else {
                unreachable!()
            };
            let mut expected = vec![0; GetEnhMetaFileBits(handle, None) as usize];
            assert_eq!(
                GetEnhMetaFileBits(handle, Some(&mut expected)) as usize,
                expected.len()
            );
            drop(original);
            let mut actual = vec![0; GetEnhMetaFileBits(cloned, None) as usize];
            assert_eq!(
                GetEnhMetaFileBits(cloned, Some(&mut actual)) as usize,
                actual.len()
            );
            assert_eq!(actual, expected);
        }
    }
    #[test]
    #[ignore = "Temporarily publishes an owned 4x4 bitmap and restores the original clipboard; no focus or private-data output."]
    fn native_image_clipboard_roundtrip() {
        struct Guard {
            owner: Owner,
            saved: Saved,
            sequence: u32,
        }
        impl Guard {
            fn current(&self) {
                assert!(
                    publication_matches(
                        self.owner.0,
                        unsafe { GetClipboardOwner() }.unwrap_or_default(),
                        self.sequence,
                        unsafe { GetClipboardSequenceNumber() }
                    ),
                    "A newer copy was preserved"
                );
            }
            fn claim(&mut self) {
                assert_eq!(
                    unsafe { GetClipboardOwner() }.unwrap_or_default(),
                    self.owner.0,
                    "A newer copy was preserved"
                );
                self.sequence = unsafe { GetClipboardSequenceNumber() };
            }
        }
        impl Drop for Guard {
            fn drop(&mut self) {
                if publication_matches(
                    self.owner.0,
                    unsafe { GetClipboardOwner() }.unwrap_or_default(),
                    self.sequence,
                    unsafe { GetClipboardSequenceNumber() },
                ) {
                    if let Ok(lock) = open(self.owner.0) {
                        if publication_matches(
                            self.owner.0,
                            unsafe { GetClipboardOwner() }.unwrap_or_default(),
                            self.sequence,
                            unsafe { GetClipboardSequenceNumber() },
                        ) {
                            if let Err(error) = unsafe { restore(&self.saved) } {
                                eprintln!("Owned clipboard fixture could not restore its snapshot: {error}");
                            }
                        }
                        drop(lock);
                    }
                }
            }
        }
        let owner = Owner::new().unwrap();
        let (saved, original_sequence, original_owner) = {
            let lock = open(owner.0).unwrap();
            let saved = unsafe { snapshot().unwrap() };
            let sequence = unsafe { GetClipboardSequenceNumber() };
            let original_owner = unsafe { GetClipboardOwner() }.unwrap_or_default();
            drop(lock);
            (saved, sequence, original_owner)
        };
        let mut guard = Guard {
            owner,
            saved,
            sequence: 0,
        };
        let lock = open(guard.owner.0).unwrap();
        assert_eq!(
            unsafe { GetClipboardSequenceNumber() },
            original_sequence,
            "A newer copy was preserved"
        );
        assert_eq!(
            unsafe { GetClipboardOwner() }.unwrap_or_default(),
            original_owner,
            "A newer copy was preserved"
        );
        let pixels = [0x00336699u32; 16];
        unsafe {
            let bitmap =
                SavedData::Bitmap(CreateBitmap(4, 4, 1, 32, Some(pixels.as_ptr() as *const _)));
            let SavedData::Bitmap(handle) = bitmap else {
                unreachable!()
            };
            assert!(!handle.0.is_null());
            EmptyClipboard().unwrap();
            if let Err(error) = bitmap.publish(2) {
                let _ = restore(&guard.saved);
                panic!("{error}");
            }
        }
        drop(lock);
        guard.claim();
        let lock = open(guard.owner.0).unwrap();
        guard.current();
        let image = unsafe { snapshot().expect("Copied images must not block paste") };
        assert!(image
            .iter()
            .any(|(format, value)| *format == 2 && matches!(value, SavedData::Bitmap(_))));
        unsafe {
            EmptyClipboard().unwrap();
            set_raw(13, &bytes("Hush owned image roundtrip")).unwrap();
        }
        drop(lock);
        guard.claim();
        let lock = open(guard.owner.0).unwrap();
        guard.current();
        unsafe {
            restore(&image).unwrap();
        }
        drop(lock);
        guard.claim();
        let lock = open(guard.owner.0).unwrap();
        guard.current();
        let restored = unsafe { snapshot().unwrap() };
        assert!(restored
            .iter()
            .any(|(format, value)| *format == 2 && matches!(value, SavedData::Bitmap(_))));
        drop(lock);
    }
    #[test]
    fn native_bitmap_copy_is_independent_and_preserves_rgb_pixels() {
        unsafe {
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = 4;
            info.bmiHeader.biHeight = -4;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            let mut bits = std::ptr::null_mut();
            let original = SavedData::Bitmap(
                CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).unwrap(),
            );
            std::slice::from_raw_parts_mut(bits as *mut u32, 16).fill(0x00664422);
            let SavedData::Bitmap(bitmap) = original else {
                unreachable!()
            };
            let copy = SavedData::Bitmap(clone_bitmap(bitmap).unwrap());
            let SavedData::Bitmap(cloned) = copy else {
                unreachable!()
            };
            assert_ne!(bitmap, cloned);
            drop(original);
            let mut result = BITMAP::default();
            assert_ne!(
                GetObjectW(
                    HGDIOBJ(cloned.0),
                    size_of::<BITMAP>() as i32,
                    Some(&mut result as *mut _ as *mut _)
                ),
                0
            );
            assert_eq!(
                (result.bmWidth, result.bmHeight.abs(), result.bmBitsPixel),
                (4, 4, 32)
            );
            assert!(!result.bmBits.is_null());
            assert!(std::slice::from_raw_parts(result.bmBits as *const u32, 16)
                .iter()
                .all(|pixel| pixel & 0x00ff_ffff == 0x0066_4422));
        }
    }
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
    #[test]
    fn clipboard_handoff_rejects_newer_copies_and_changed_owners() {
        let ours = HWND(123usize as *mut _);
        let other = HWND(456usize as *mut _);
        assert!(publication_matches(ours, ours, 10, 10));
        assert!(!publication_matches(ours, other, 10, 10));
        assert!(!publication_matches(ours, ours, 10, 11));
        assert!(!publication_matches(
            HWND::default(),
            HWND::default(),
            10,
            10
        ));
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
            let wait = std::env::var("HUSH_INTERACTIVE_TEST_WAIT_MS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(250)
                .min(60_000);
            let deadline = std::time::Instant::now() + Duration::from_millis(wait);
            while unsafe { GetForegroundWindow() }.0 as isize != parent
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(25));
            }
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
        fn wait_text(&self, expected: &str) {
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while self.text() != expected && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            assert_eq!(self.text(), expected);
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
            data: Saved,
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
        h.wait_text("Hello Hushline");
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
        h.wait_text("Hello Hushline clipboard");
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

    #[test]
    #[ignore = "Requires an interactive Windows desktop; verifies clipboard insertion with the real input/focus hooks running."]
    fn native_clipboard_with_live_hooks() {
        let h = Harness::open(false);
        assert_eq!(
            unsafe { GetForegroundWindow() },
            h.parent,
            "Owned test field must be foreground"
        );
        let (tx, _rx) = mpsc::channel();
        crate::native::start_hooks(&crate::model::Shortcuts::default(), tx).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let target = capture_target(false).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(
            input_epoch(),
            target.epoch,
            "Passive focused field changed the input epoch"
        );
        let (tx, rx) = mpsc::channel();
        crate::native::insert_with_notice(
            &target,
            "Clipboard with live hooks",
            "clipboard",
            false,
            &CancellationToken::new(),
            Some(Box::new(move |warning| {
                let _ = tx.send(warning);
            })),
        )
        .unwrap();
        h.wait_text("Clipboard with live hooks");
        assert!(rx.recv_timeout(Duration::from_secs(2)).unwrap().is_empty());
    }

    #[test]
    #[ignore = "Run through tests/native/paste-browser.mjs; accepts only its uniquely titled owned browser field."]
    fn native_browser_clipboard() {
        let title =
            std::env::var("HUSH_BROWSER_TEST_TITLE").expect("Owned browser test title required");
        assert!(title.starts_with("Hush owned paste test "));
        // A normal foreground request for this test's exact, disposable window.
        // If Windows refuses, the guard below fails without delivering any input.
        let full: Vec<u16> = format!("{title} - Google Chrome")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        if let Ok(owned) = unsafe { FindWindowW(None, windows::core::PCWSTR(full.as_ptr())) } {
            let _ = unsafe { SetForegroundWindow(owned) };
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let window = loop {
            let hwnd = unsafe { GetForegroundWindow() };
            let mut text = [0u16; 256];
            let len = unsafe { GetWindowTextW(hwnd, &mut text) };
            if String::from_utf16_lossy(&text[..len as usize]).starts_with(&title) {
                break hwnd;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Owned browser must be foreground; nothing was inserted"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        let (tx, _rx) = mpsc::channel();
        crate::native::start_hooks(&crate::model::Shortcuts::default(), tx).unwrap();
        std::thread::sleep(Duration::from_millis(250));
        let started = std::time::Instant::now();
        let target = capture_target(false).unwrap();
        println!(
            "Owned browser inspection: {} ms",
            started.elapsed().as_millis()
        );
        assert_eq!(target.hwnd, window.0 as isize);
        assert!(!target.terminal);
        std::thread::sleep(Duration::from_millis(400));
        let (tx, rx) = mpsc::channel();
        crate::native::insert_with_notice(
            &target,
            "Hush clipboard: first sentence.\n\n• First item\n• Second item",
            "clipboard",
            false,
            &CancellationToken::new(),
            Some(Box::new(move |warning| {
                let _ = tx.send(warning);
            })),
        )
        .unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(2)).unwrap().is_empty());
    }
}
