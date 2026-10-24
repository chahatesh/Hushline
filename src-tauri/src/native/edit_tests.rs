// SPDX-License-Identifier: MIT
//! Disposable cross-process Windows edit fixture. No user window, clipboard
//! mutation, microphone, network, installed service, or foreground grant.
use super::*;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
};
use windows::core::w;

#[test]
#[ignore = "Private child fixture; invoked only by native_atomic_edit_external_control."]
fn native_edit_host() {
    assert_eq!(
        std::env::var("HUSH_NATIVE_EDIT_HOST").as_deref(),
        Ok("owned-test-only")
    );
    let (tx, rx) = mpsc::channel();
    let thread = std::thread::spawn(move || unsafe {
        let parent = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("Hush owned native edit fixture"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            640,
            400,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let edit = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(0x4 | 0x40),
            0,
            0,
            600,
            300,
            Some(parent),
            None,
            None,
            None,
        )
        .unwrap();
        tx.send((
            parent.0 as isize,
            edit.0 as isize,
            windows::Win32::System::Threading::GetCurrentThreadId(),
        ))
        .unwrap();
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = DestroyWindow(parent);
    });
    let (parent, edit, thread_id) = rx.recv_timeout(Duration::from_secs(3)).unwrap();
    println!("HUSH_EDIT_HOST {parent} {edit}");
    std::io::stdout().flush().unwrap();
    let _ = std::io::stdin().read_line(&mut String::new());
    unsafe {
        let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
    }
    thread.join().unwrap();
}
struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
unsafe fn read_owned(hwnd: HWND) -> String {
    let mut text = [0u16; 256];
    let len = message(
        hwnd,
        WM_GETTEXT,
        WPARAM(text.len()),
        LPARAM(text.as_mut_ptr() as isize),
        Duration::from_millis(500),
    )
    .unwrap();
    String::from_utf16_lossy(&text[..len])
}
unsafe fn replace_owned(hwnd: HWND, text: &str) {
    let text: Vec<u16> = windows_lines(text).encode_utf16().chain(Some(0)).collect();
    assert!(message(
        hwnd,
        EM_REPLACESEL,
        WPARAM(1),
        LPARAM(text.as_ptr() as isize),
        Duration::from_millis(500)
    )
    .is_some());
}
#[test]
fn native_atomic_edit_external_control() {
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native::edit::integration::native_edit_host",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("HUSH_NATIVE_EDIT_HOST", "owned-test-only")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut fixture = Fixture(child);
    let output = fixture.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if let Some((_, data)) = line.split_once("HUSH_EDIT_HOST ") {
                let _ = tx.send(data.to_string());
            }
        }
    });
    let handles: Vec<isize> = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Owned child fixture did not start")
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    let parent = HWND(handles[0] as *mut _);
    let edit = HWND(handles[1] as *mut _);
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(edit, Some(&mut pid));
        assert_eq!(
            pid,
            fixture.0.id(),
            "Refusing to edit anything outside our child fixture"
        );
        assert_eq!(GetAncestor(edit, GA_ROOT), parent);
        let start = inspect(edit, parent).unwrap();
        assert_eq!((start.start, start.end, start.length), (0, 0, 0));
        replace_owned(edit, "Hello\nWorld");
        assert_eq!(read_owned(edit), "Hello\r\nWorld");
        message(
            edit,
            EM_GETSEL + 1,
            WPARAM(0),
            LPARAM(5),
            Duration::from_millis(500),
        )
        .unwrap(); // EM_SETSEL
        replace_owned(edit, "Hi");
        assert_eq!(read_owned(edit), "Hi\r\nWorld");
        message(
            edit,
            0x00C7,
            WPARAM(0),
            LPARAM(0),
            Duration::from_millis(500),
        )
        .unwrap(); // EM_UNDO
        assert_eq!(read_owned(edit), "Hello\r\nWorld");
        let selection = inspect(edit, parent).unwrap();
        let target = Target {
            hwnd: parent.0 as isize,
            runtime: vec![1],
            caret: vec![],
            selected: String::new(),
            app: "owned fixture".into(),
            epoch: input_epoch(),
            terminal: false,
            native_edit: Some(selection.clone()),
        };
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(insert(&target, &selection, "UNWANTED", &cancel).is_err());
        assert!(insert(
            &target,
            &selection,
            "WRONG FOREGROUND",
            &CancellationToken::new()
        )
        .is_err());
        assert_eq!(read_owned(edit), "Hello\r\nWorld");
        message(
            edit,
            0x00CF,
            WPARAM(1),
            LPARAM(0),
            Duration::from_millis(500),
        )
        .unwrap(); // EM_SETREADONLY
        assert!(inspect(edit, parent).is_none());
        message(
            edit,
            0x00CF,
            WPARAM(0),
            LPARAM(0),
            Duration::from_millis(500),
        )
        .unwrap();
        SetWindowLongPtrW(edit, GWL_STYLE, GetWindowLongPtrW(edit, GWL_STYLE) | 0x20);
        assert!(inspect(edit, parent).is_none());
    }
    fixture
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"quit\n")
        .unwrap();
    assert!(fixture.0.wait().unwrap().success());
}
