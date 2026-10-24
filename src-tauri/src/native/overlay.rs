// SPDX-License-Identifier: MIT
//! Nonactivating native overlay. No WebView, idle animation, or audio dependency.
use crate::{
    model::{Settings, View},
    shortcuts::Action,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, AtomicIsize, Ordering},
        mpsc, Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use windows::{
    core::{w, BOOL},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
};
const UPDATE: u32 = WM_APP + 17;
#[cfg(test)]
const QUERY_PAINT: u32 = WM_APP + 18;
#[cfg(test)]
const QUERY_SURFACES: u32 = WM_APP + 19;
struct Bridge {
    hwnd: AtomicIsize,
    view: Arc<Mutex<OverlayView>>,
    pending: AtomicBool,
}
#[derive(Clone, Default)]
struct OverlayView {
    generation: u64,
    phase: String,
    stage: String,
    message: String,
    warning: String,
    recoverable: bool,
    level: f32,
    seconds: f32,
    dock: String,
    theme: String,
}
const DETAILS_MS: u32 = 2500;
fn details_ms(view: &OverlayView) -> u32 {
    if view.phase == "error" || view.recoverable || !view.warning.is_empty() {
        crate::lifecycle::RECOVERY_NOTICE_MS as u32
    } else {
        DETAILS_MS
    }
}
#[derive(Default)]
struct Presentation {
    generation: u64,
    active: bool,
    notice: u64,
    compact: bool,
}
impl Presentation {
    fn sync(&mut self, view: &OverlayView) -> bool {
        let active = view.phase != "idle" && !view.phase.is_empty();
        let notice = notice_id(view);
        let expand = active
            && (!self.active
                || self.generation != view.generation
                || (self.notice != notice && !notice_text(view).is_empty()));
        if !active || expand {
            self.compact = false;
        }
        self.generation = view.generation;
        self.active = active;
        self.notice = notice;
        expand
    }
}
static BRIDGE: OnceLock<Bridge> = OnceLock::new();
/// Only this process's nonactivating overlay may preserve target authorization.
/// Called from the mouse hook: one native hit test, no locks, UIA or IPC.
pub fn contains_point(point: POINT) -> bool {
    let Some(bridge) = BRIDGE.get() else {
        return false;
    };
    let hwnd = HWND(bridge.hwnd.load(Ordering::Acquire) as *mut _);
    !hwnd.0.is_null() && unsafe { WindowFromPoint(point) } == hwnd
}
struct Painter {
    view: Arc<Mutex<OverlayView>>,
    tx: mpsc::Sender<Action>,
    dpi: Cell<u32>,
    font: Cell<HFONT>,
    small: Cell<HFONT>,
    animated: bool,
    alpha: Cell<u8>,
    paint_ok: Cell<bool>,
    layout: Cell<(bool, u8, u64, bool)>,
    presentation: RefCell<Presentation>,
    expanding: Cell<bool>,
    details_deadline: Cell<Option<Instant>>,
    pill: Cell<RECT>,
    notice: Cell<RECT>,
    from: Cell<RECT>,
    to: Cell<RECT>,
    fade_from: Cell<u8>,
    fade_to: Cell<u8>,
    transition: Cell<Option<Instant>>,
    duration: Cell<f64>,
    corner: Cell<f64>,
    corner_from: Cell<f64>,
    corner_to: Cell<f64>,
    surface: RefCell<Option<Surface>>,
    surface_builds: Cell<u32>,
    title: RefCell<String>,
    claimed: Rc<Cell<bool>>,
}
// GDI handles and the two small pixel buffers stay on the overlay thread. They
// are reused until the window changes size; no per-meter bitmap allocation.
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u32,
    width: i32,
    height: i32,
    pixels: Vec<u32>,
}
impl Surface {
    unsafe fn new(hdc: HDC, width: i32, height: i32) -> Option<Self> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(Some(hdc), &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        let dc = CreateCompatibleDC(Some(hdc));
        if dc.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            return None;
        }
        let old = SelectObject(dc, HGDIOBJ(bitmap.0));
        Some(Self {
            dc,
            bitmap,
            old,
            bits: bits.cast(),
            width,
            height,
            pixels: vec![0; (width * height) as usize],
        })
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.dc, self.old);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.dc);
        }
    }
}
impl Drop for Painter {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.font.get().0));
            let _ = DeleteObject(HGDIOBJ(self.small.get().0));
        }
    }
}

pub fn start(tx: mpsc::Sender<Action>) -> Result<(), String> {
    let view = Arc::new(Mutex::new(OverlayView {
        phase: "idle".into(),
        ..OverlayView::default()
    }));
    let shared = view.clone();
    let (ready, rx) = mpsc::sync_channel::<Result<isize, String>>(1);
    std::thread::spawn(move || unsafe {
        let instance = match GetModuleHandleW(None) {
            Ok(h) => HINSTANCE(h.0),
            Err(_) => {
                let _ = ready.send(Err("Cannot initialize the native overlay.".into()));
                return;
            }
        };
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: w!("HushNativeOverlay"),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            let _ = ready.send(Err("Cannot register the native overlay.".into()));
            return;
        }
        let mut animations = BOOL(1);
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut animations as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let claimed = Rc::new(Cell::new(false));
        let painter = Rc::new(Painter {
            view: shared,
            tx,
            dpi: Cell::new(0),
            font: Cell::new(HFONT::default()),
            small: Cell::new(HFONT::default()),
            animated: animations.as_bool(),
            alpha: Cell::new(165),
            paint_ok: Cell::new(false),
            layout: Cell::new((false, 255, 0, false)),
            presentation: RefCell::new(Presentation::default()),
            expanding: Cell::new(false),
            details_deadline: Cell::new(None),
            pill: Cell::new(RECT::default()),
            notice: Cell::new(RECT::default()),
            from: Cell::new(RECT::default()),
            to: Cell::new(RECT::default()),
            fade_from: Cell::new(165),
            fade_to: Cell::new(165),
            transition: Cell::new(None),
            duration: Cell::new(0.17),
            corner: Cell::new(8.0),
            corner_from: Cell::new(8.0),
            corner_to: Cell::new(8.0),
            surface: RefCell::new(None),
            surface_builds: Cell::new(0),
            title: RefCell::new(String::new()),
            claimed: claimed.clone(),
        });
        let pointer = Rc::into_raw(painter);
        match CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            w!("HushNativeOverlay"),
            w!("Hush dictation"),
            WS_POPUP,
            0,
            0,
            360,
            64,
            None,
            None,
            Some(instance),
            Some(pointer as *const _),
        ) {
            Ok(hwnd) => {
                let _ = ready.send(Ok(hwnd.0 as isize));
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = DestroyWindow(hwnd);
            }
            Err(_) => {
                if !claimed.get() {
                    drop(Rc::from_raw(pointer));
                }
                let _ = ready.send(Err("Cannot create the native overlay.".into()));
            }
        }
    });
    let hwnd = rx
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| "Native overlay did not initialize.")??;
    BRIDGE
        .set(Bridge {
            hwnd: AtomicIsize::new(hwnd),
            view,
            pending: AtomicBool::new(false),
        })
        .map_err(|_| "Native overlay is already running.".to_string())
}
pub fn update(view: &View) {
    if let Some(bridge) = BRIDGE.get() {
        if let Ok(mut v) = bridge.view.lock() {
            *v = OverlayView {
                generation: view.generation,
                phase: view.phase.clone(),
                stage: view.stage.clone(),
                message: view.message.clone(),
                warning: view.warning.clone(),
                recoverable: view.recoverable,
                level: view.level,
                seconds: view.seconds,
                dock: v.dock.clone(),
                theme: v.theme.clone(),
            };
        }
        queue_update(bridge);
    }
}
fn queue_update(bridge: &Bridge) {
    if !bridge.pending.swap(true, Ordering::AcqRel) {
        let hwnd = HWND(bridge.hwnd.load(Ordering::Relaxed) as *mut _);
        if unsafe { PostMessageW(Some(hwnd), UPDATE, WPARAM(0), LPARAM(0)) }.is_err() {
            bridge.pending.store(false, Ordering::Release);
        }
    }
}
fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16))
}
unsafe fn fonts(hwnd: HWND, p: &Painter) {
    let dpi = GetDpiForWindow(hwnd).max(96);
    if p.dpi.get() == dpi {
        return;
    }
    for f in [p.font.get(), p.small.get()] {
        if !f.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(f.0));
        }
    }
    p.dpi.set(dpi);
    let make = |size: i32| {
        CreateFontW(
            -size * dpi as i32 / 96,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            0,
            w!("Segoe UI"),
        )
    };
    p.font.set(make(13));
    p.small.set(make(11));
}
pub fn configure(settings: &Settings) {
    if let Some(bridge) = BRIDGE.get() {
        if let Ok(mut v) = bridge.view.lock() {
            v.dock = settings.dock.clone();
            v.theme = settings.theme.clone();
        }
        queue_update(bridge);
    }
}
fn dock_id(value: &str) -> u8 {
    match value {
        "left" => 1,
        "right" => 2,
        "hidden" => 3,
        _ => 0,
    }
}
fn dimensions(active: bool, side: bool) -> (f64, f64) {
    match (active, side) {
        (false, false) => (48.0, 16.0),
        (true, false) => (168.0, 32.0),
        (false, true) => (16.0, 48.0),
        (true, true) => (40.0, 128.0),
    }
}
fn visible_dimensions(active: bool, side: bool, compact: bool) -> (f64, f64) {
    if active && compact {
        if side {
            (40.0, 112.0)
        } else {
            (168.0, 28.0)
        }
    } else {
        dimensions(active, side)
    }
}
fn wave_origin(extent: f64) -> f64 {
    (extent - 52.6) / 2.0
}
fn dock_bounds(work: RECT, dock: u8, width: i32, height: i32, scale: f64) -> RECT {
    let gap = (7.0 * scale) as i32;
    let bottom = if matches!(dock, 1 | 2) {
        work.top + (work.bottom - work.top) / 2 + (24.0 * scale) as i32
    } else {
        work.bottom - gap
    };
    let left = match dock {
        1 => work.left + gap,
        2 => work.right - width - gap,
        _ => work.left + (work.right - work.left - width) / 2,
    };
    RECT {
        left,
        top: bottom - height,
        right: left + width,
        bottom,
    }
}
fn eased_progress(t: f64, entering: bool) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if entering {
        1.0 - (1.0 - t).powi(3)
    } else {
        t * t
    }
}
fn notice_text(view: &OverlayView) -> String {
    if view.phase == "idle" {
        return String::new();
    }
    let message = if view.phase == "error"
        || (view.phase == "success" && (view.recoverable || view.message.starts_with("Copied")))
        || (matches!(view.phase.as_str(), "starting" | "listening")
            && view.message.starts_with("Hands-free"))
    {
        view.message.as_str()
    } else {
        ""
    };
    [message, view.warning.as_str()]
        .into_iter()
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
        .chars()
        .take(1200)
        .collect()
}
fn notice_id(view: &OverlayView) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    notice_text(view).hash(&mut hash);
    view.recoverable.hash(&mut hash);
    hash.finish()
}
fn can_recover(view: &OverlayView) -> bool {
    view.recoverable && matches!(view.phase.as_str(), "error" | "success")
}
fn recovery_buttons(card: RECT, scale: f64) -> (RECT, RECT) {
    let padding = (14.0 * scale) as i32;
    let gap = (8.0 * scale) as i32;
    let middle = (card.left + card.right) / 2;
    let top = card.bottom - (36.0 * scale) as i32;
    let bottom = card.bottom - (10.0 * scale) as i32;
    (
        RECT {
            left: card.left + padding,
            top,
            right: middle - gap / 2,
            bottom,
        },
        RECT {
            left: middle + gap / 2,
            top,
            right: card.right - padding,
            bottom,
        },
    )
}
fn recovery_action(view: &OverlayView, card: RECT, scale: f64, x: i32, y: i32) -> Option<Action> {
    if !can_recover(view) || card.bottom == 0 {
        return None;
    }
    let (paste, copy) = recovery_buttons(card, scale);
    let inside = |r: RECT| x >= r.left && x < r.right && y >= r.top && y < r.bottom;
    if inside(paste) {
        Some(Action::Paste)
    } else if inside(copy) {
        Some(Action::Copy)
    } else {
        None
    }
}
fn timer_bounds(width: f64, height: f64, side: bool) -> (f64, f64, f64, f64) {
    if side {
        (4.0, 6.0, width - 4.0, 21.0)
    } else {
        (12.0, (height - 15.0) / 2.0, 42.0, (height + 15.0) / 2.0)
    }
}
fn recording_level(view: &OverlayView) -> f32 {
    if view.phase == "listening" {
        view.level.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
fn processing_progress(stage: &str) -> usize {
    match stage {
        "preparing_audio" => 2,
        "transcribing" => 5,
        "polishing" => 8,
        "inserting" => 11,
        _ => 0,
    }
}
unsafe fn notice_height(
    hwnd: HWND,
    p: &Painter,
    value: &str,
    width: i32,
    recoverable: bool,
) -> i32 {
    if value.is_empty() {
        return 0;
    }
    let dc = GetDC(Some(hwnd));
    let old = SelectObject(dc, HGDIOBJ(p.font.get().0));
    let mut text: Vec<u16> = value.encode_utf16().collect();
    let scale = p.dpi.get() as f64 / 96.0;
    let mut rect = RECT {
        right: width - (28.0 * scale) as i32,
        ..RECT::default()
    };
    DrawTextW(
        dc,
        &mut text,
        &mut rect,
        DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
    );
    let _ = SelectObject(dc, old);
    ReleaseDC(Some(hwnd), dc);
    (rect.bottom + (28.0 * scale) as i32).clamp((60.0 * scale) as i32, (280.0 * scale) as i32)
        + if recoverable {
            (36.0 * scale) as i32
        } else {
            0
        }
}
unsafe fn geometry(hwnd: HWND, _p: &Painter, rect: RECT) {
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
}
unsafe fn position(hwnd: HWND, p: &Painter, animate: bool) {
    let view = p.view.lock().map(|v| v.clone()).unwrap_or_default();
    let active = view.phase != "idle" && !view.phase.is_empty();
    let compact = active && p.presentation.borrow().compact;
    let dock = dock_id(&view.dock);
    if !active && dock == 3 {
        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = KillTimer(Some(hwnd), 2);
        p.layout.set((active, dock, notice_id(&view), compact));
        return;
    }
    let monitor = MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(monitor, &mut info).as_bool() {
        return;
    }
    fonts(hwnd, p);
    let scale = p.dpi.get() as f64 / 96.0;
    let (width, height) = visible_dimensions(active, dock == 1 || dock == 2, compact);
    let width = (width * scale) as i32;
    let height = (height * scale) as i32;
    let mut rect = dock_bounds(info.rcWork, dock, width, height, scale);
    let x = rect.left;
    let notice = if compact {
        String::new()
    } else {
        notice_text(&view)
    };
    let card_width = ((340.0 * scale) as i32).min(info.rcWork.right - info.rcWork.left - 20);
    let card_height = notice_height(hwnd, p, &notice, card_width, can_recover(&view));
    if card_height > 0 {
        let full_width = width.max(card_width);
        let footer = (32.0 * scale) as i32;
        let full_height = footer + card_height;
        rect.left = match dock {
            1 => x,
            2 => x + width - full_width,
            _ => x + (width - full_width) / 2,
        };
        rect.left = rect
            .left
            .clamp(info.rcWork.left, info.rcWork.right - full_width);
        rect.top = (rect.bottom - full_height).max(info.rcWork.top);
        rect.right = rect.left + full_width;
        rect.bottom = rect.top + full_height;
        p.pill.set(RECT {
            left: 0,
            top: full_height - footer,
            right: full_width,
            bottom: full_height,
        });
        p.notice.set(RECT {
            left: 0,
            top: 0,
            right: full_width,
            bottom: card_height,
        });
    } else {
        p.pill.set(RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        });
        p.notice.set(RECT::default());
    }
    let alpha = if active { 255 } else { 165 };
    let mut from = RECT::default();
    let _ = GetWindowRect(hwnd, &mut from);
    let visible = IsWindowVisible(hwnd).as_bool();
    p.from.set(from);
    p.to.set(rect);
    p.fade_from.set(p.alpha.get());
    p.fade_to.set(alpha);
    p.corner_from.set(p.corner.get());
    p.corner_to.set(if card_height > 0 {
        18.0
    } else {
        (width.min(height) as f64 / scale) / 2.0
    });
    // Mark the layout before SetWindowPos can synchronously reenter for DPI changes.
    p.layout.set((active, dock, notice_id(&view), compact));
    p.expanding
        .set(rect.bottom - rect.top > from.bottom - from.top);
    p.duration.set(if card_height > 0 {
        0.22
    } else if active && !compact {
        0.17
    } else {
        0.13
    });
    if animate && visible && p.animated {
        p.transition.set(Some(Instant::now()));
        SetTimer(Some(hwnd), 2, 16, None);
    } else {
        p.transition.set(None);
        let _ = KillTimer(Some(hwnd), 2);
        p.alpha.set(alpha);
        p.corner.set(p.corner_to.get());
        geometry(hwnd, p, rect);
    }
}
unsafe fn advance_animation(hwnd: HWND, p: &Painter) {
    if let Some(started) = p.transition.get() {
        let t = (started.elapsed().as_secs_f64() / p.duration.get()).min(1.0);
        let eased = eased_progress(t, p.expanding.get());
        let mix = |a: i32, b: i32| (a as f64 + (b - a) as f64 * eased).round() as i32;
        let a = p.from.get();
        let b = p.to.get();
        p.alpha
            .set(mix(p.fade_from.get() as i32, p.fade_to.get() as i32) as u8);
        p.corner
            .set(p.corner_from.get() + (p.corner_to.get() - p.corner_from.get()) * eased);
        geometry(
            hwnd,
            p,
            RECT {
                left: mix(a.left, b.left),
                top: mix(a.top, b.top),
                right: mix(a.right, b.right),
                bottom: mix(a.bottom, b.bottom),
            },
        );
        if t >= 1.0 {
            p.transition.set(None);
            let _ = KillTimer(Some(hwnd), 2);
        }
    }
}
fn timer_alignment(side: bool) -> DRAW_TEXT_FORMAT {
    if side {
        DT_CENTER
    } else {
        DT_LEFT
    }
}
unsafe fn text(hdc: HDC, value: &str, rect: RECT, font: HFONT, _color: COLORREF, side: bool) {
    // An empty Vec has a dangling sentinel pointer. Some Windows text paths
    // inspect the buffer even for a zero character count; never pass it to GDI.
    if value.is_empty() {
        return;
    }
    let previous = SelectObject(hdc, HGDIOBJ(font.0));
    // The DIB is an alpha mask. Drawing dark ink here would multiply its
    // darkness into coverage and make light-theme labels nearly invisible.
    let _ = SetTextColor(hdc, rgb(255, 255, 255));
    let mut text: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
    let count = text.len() - 1;
    let mut rect = rect;
    let _ = DrawTextW(
        hdc,
        &mut text[..count],
        &mut rect,
        timer_alignment(side) | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
    );
    let _ = SelectObject(hdc, previous);
}
unsafe fn wrapped_text(hdc: HDC, value: &str, mut rect: RECT, font: HFONT) {
    let previous = SelectObject(hdc, HGDIOBJ(font.0));
    let _ = SetTextColor(hdc, rgb(255, 255, 255));
    let mut text: Vec<u16> = value.encode_utf16().collect();
    if !text.is_empty() {
        DrawTextW(
            hdc,
            &mut text,
            &mut rect,
            DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
    }
    let _ = SelectObject(hdc, previous);
}
// Signed distance to a rounded box gives fractional edge coverage at every DPI.
// Unlike a window region, per-pixel alpha retains the antialiased outer pixels.
fn coverage(x: f64, y: f64, width: f64, height: f64, radius: f64) -> f64 {
    if (x >= radius && x <= width - radius && y >= 0.5 && y <= height - 0.5)
        || (y >= radius && y <= height - radius && x >= 0.5 && x <= width - 0.5)
    {
        return 1.0;
    }
    let qx = (x - width / 2.0).abs() - (width / 2.0 - radius);
    let qy = (y - height / 2.0).abs() - (height / 2.0 - radius);
    let distance = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius;
    (0.5 - distance).clamp(0.0, 1.0)
}
fn composite(pixel: &mut u32, color: COLORREF, opacity: f64) {
    if opacity <= 0.0 {
        return;
    }
    if opacity >= 1.0 {
        *pixel = 0xff000000 | (color.0 & 255) << 16 | (color.0 & 0xff00) | (color.0 >> 16 & 255);
        return;
    }
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
    let inverse = 255 - alpha;
    let r = color.0 & 255;
    let g = (color.0 >> 8) & 255;
    let b = (color.0 >> 16) & 255;
    let mix = |source: u32, shift: u32| {
        (source * alpha + ((*pixel >> shift) & 255) * inverse + 127) / 255
    };
    *pixel = (alpha + ((*pixel >> 24) * inverse + 127) / 255) << 24
        | mix(r, 16) << 16
        | mix(g, 8) << 8
        | mix(b, 0);
}
fn rounded(
    pixels: &mut [u32],
    width: i32,
    height: i32,
    rect: (f64, f64, f64, f64),
    color: COLORREF,
) {
    rounded_radius(pixels, width, height, rect, color, rect.2.min(rect.3) / 2.0);
}
fn rounded_radius(
    pixels: &mut [u32],
    width: i32,
    height: i32,
    rect: (f64, f64, f64, f64),
    color: COLORREF,
    radius: f64,
) {
    let (left, top, w, h) = rect;
    for y in (top.floor() as i32 - 1).max(0)..((top + h).ceil() as i32 + 1).min(height) {
        for x in (left.floor() as i32 - 1).max(0)..((left + w).ceil() as i32 + 1).min(width) {
            let alpha = coverage(x as f64 + 0.5 - left, y as f64 + 0.5 - top, w, h, radius);
            composite(&mut pixels[(y * width + x) as usize], color, alpha);
        }
    }
}
unsafe fn paint(hwnd: HWND, p: &Painter) {
    fonts(hwnd, p);
    let view = p.view.lock().map(|v| v.clone()).unwrap_or_default();
    let active = view.phase != "idle" && !view.phase.is_empty();
    let expanded = p.notice.get().bottom > 0;
    // A new dock may be queued while an old paint is pending. Draw the
    // orientation that owns the current geometry until positioning catches up.
    let side = matches!(p.layout.get().1, 1 | 2) && !expanded;
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    let (width, height) = (client.right.max(1), client.bottom.max(1));
    let Ok(mut slot) = p.surface.try_borrow_mut() else {
        let _ = EndPaint(hwnd, &ps);
        return;
    };
    if slot
        .as_ref()
        .is_none_or(|s| s.width != width || s.height != height)
    {
        *slot = Surface::new(hdc, width, height);
        p.surface_builds.set(p.surface_builds.get() + 1);
    }
    let Some(surface) = slot.as_mut() else {
        let _ = EndPaint(hwnd, &ps);
        return;
    };
    let dc = surface.dc;
    let memory = std::slice::from_raw_parts_mut(surface.bits, (width * height) as usize);
    memory.fill(0);
    let pixels = &mut surface.pixels;
    pixels.fill(0);
    let scale = p.dpi.get() as f64 / 96.0;
    let w = width as f64 / scale;
    let footer = if expanded {
        (32.0 * scale) as i32
    } else {
        height
    };
    let top = if expanded {
        (height - footer).max(0)
    } else {
        0
    };
    let oy = top as f64 / scale;
    let h = (height - top) as f64 / scale;
    p.pill.set(RECT {
        left: 0,
        top,
        right: width,
        bottom: height,
    });
    let light = view.theme == "light";
    let waiting = matches!(view.phase.as_str(), "starting" | "processing");
    let background = if waiting {
        rgb(16, 18, 25)
    } else if light {
        rgb(247, 242, 255)
    } else {
        rgb(35, 23, 55)
    };
    let ink = if waiting {
        rgb(153, 163, 183)
    } else if light {
        rgb(53, 29, 85)
    } else {
        rgb(244, 236, 255)
    };
    let (violet, turquoise) = if light {
        (rgb(111, 57, 208), rgb(7, 133, 124))
    } else {
        (rgb(179, 134, 255), rgb(62, 234, 210))
    };
    let accent = match view.phase.as_str() {
        "error" => {
            if light {
                rgb(192, 40, 87)
            } else {
                rgb(255, 134, 166)
            }
        }
        "success" => turquoise,
        _ => violet,
    };
    let radius = (p.corner.get() * scale).min(width.min(height) as f64 / 2.0);
    // A single continuous surface unfolds above the anchored lower edge.
    rounded_radius(
        pixels,
        width,
        height,
        (0.5, 0.5, width as f64 - 1.0, height as f64 - 1.0),
        if waiting {
            rgb(44, 49, 63)
        } else if light {
            rgb(208, 184, 244)
        } else {
            rgb(92, 59, 138)
        },
        radius,
    );
    rounded_radius(
        pixels,
        width,
        height,
        (1.5, 1.5, width as f64 - 3.0, height as f64 - 3.0),
        background,
        (radius - 1.0).max(0.0),
    );
    let mut shape = |x: f64, y: f64, w: f64, h: f64, color| {
        rounded(
            pixels,
            width,
            height,
            (x * scale, (y + oy) * scale, w * scale, h * scale),
            color,
        )
    };
    if !active {
        if side {
            shape((w - 2.0) / 2.0, (h - 18.0) / 2.0, 2.0, 18.0, accent)
        } else {
            shape((w - 20.0) / 2.0, (h - 2.0) / 2.0, 20.0, 2.0, accent)
        }
    } else {
        let level = recording_level(&view);
        let time = if p.animated && view.phase == "listening" {
            view.seconds * 3.0
        } else {
            0.0
        };
        for i in 0..11 {
            let variation = 0.25 + (i as f32 * 1.31 + time).sin().abs() * 0.75;
            let amplitude = 3.0 + level as f64 * 15.0 * variation as f64;
            let color = if waiting {
                if view.phase == "processing" && i < processing_progress(&view.stage) {
                    rgb(131, 146, 173)
                } else {
                    rgb(67, 77, 99)
                }
            } else if matches!(view.phase.as_str(), "error" | "success") {
                accent
            } else {
                mix_color(violet, turquoise, i as f64 / 10.0)
            };
            if side {
                shape(
                    (w - amplitude) / 2.0,
                    wave_origin(h) + i as f64 * 5.0,
                    amplitude,
                    2.6,
                    color,
                )
            } else {
                shape(
                    wave_origin(w) + i as f64 * 5.0,
                    h / 2.0 - amplitude / 2.0,
                    2.6,
                    amplitude,
                    color,
                )
            }
        }
        let (cx, cy) = if side {
            (w / 2.0, h - 16.0)
        } else {
            (w - 16.0, h / 2.0)
        };
        {
            for i in -4..=4 {
                let v = i as f64 * 0.65;
                shape(cx + v - 0.6, cy + v - 0.6, 1.2, 1.2, ink);
                shape(cx + v - 0.6, cy - v - 0.6, 1.2, 1.2, ink);
            }
        }
        let timer = format!(
            "{}:{:02}",
            view.seconds as u32 / 60,
            view.seconds as u32 % 60
        );
        let (left, upper, right, lower) = timer_bounds(w, h, side);
        let rect = RECT {
            left: (left * scale) as i32,
            top: top + (upper * scale) as i32,
            right: (right * scale) as i32,
            bottom: top + (lower * scale) as i32,
        };
        let _ = SetBkMode(dc, TRANSPARENT);
        text(dc, &timer, rect, p.small.get(), ink, side);
    }
    if expanded && top > 0 {
        let padding = (14.0 * scale) as i32;
        let _ = SetBkMode(dc, TRANSPARENT);
        wrapped_text(
            dc,
            &notice_text(&view),
            RECT {
                left: padding,
                top: padding,
                right: width - padding,
                bottom: (top
                    - padding
                    - if can_recover(&view) {
                        (36.0 * scale) as i32
                    } else {
                        0
                    })
                .max(padding),
            },
            p.font.get(),
        );
        if can_recover(&view) {
            let (paste, copy) = recovery_buttons(p.notice.get(), scale);
            for (rect, label, color) in [
                (paste, "Paste here", rgb(101, 78, 217)),
                (copy, "Copy text", rgb(52, 63, 87)),
            ] {
                rounded_radius(
                    pixels,
                    width,
                    height,
                    (
                        rect.left as f64,
                        rect.top as f64,
                        (rect.right - rect.left) as f64,
                        (rect.bottom - rect.top) as f64,
                    ),
                    color,
                    8.0 * scale,
                );
                text(dc, label, rect, p.small.get(), rgb(255, 255, 255), true);
            }
        }
    }
    let _ = GdiFlush();
    let reveal = if let Some(started) = p.transition.get() {
        if p.to.get().bottom - p.to.get().top > p.from.get().bottom - p.from.get().top {
            (started.elapsed().as_secs_f64() / p.duration.get() / 0.75).clamp(0.0, 1.0)
        } else {
            1.0
        }
    } else {
        1.0
    };
    let buttons = (expanded && can_recover(&view)).then(|| recovery_buttons(p.notice.get(), scale));
    for (index, (pixel, mask)) in pixels.iter_mut().zip(memory.iter()).enumerate() {
        if mask & 255 == 0 {
            continue;
        }
        let color = if buttons.is_some_and(|(a, b)| {
            let x = index as i32 % width;
            let y = index as i32 / width;
            [a, b]
                .iter()
                .any(|r| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
        }) {
            rgb(255, 255, 255)
        } else {
            ink
        };
        composite(pixel, color, (mask & 255) as f64 / 255.0 * reveal);
    }
    memory.copy_from_slice(pixels);
    let _ = GdiFlush();
    let painted = UpdateLayeredWindow(
        hwnd,
        None,
        None,
        Some(&SIZE {
            cx: width,
            cy: height,
        }),
        Some(dc),
        Some(&POINT { x: 0, y: 0 }),
        COLORREF(0),
        Some(&BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: p.alpha.get(),
            AlphaFormat: AC_SRC_ALPHA as u8,
        }),
        ULW_ALPHA,
    );
    p.paint_ok.set(painted.is_ok());
    #[cfg(test)]
    if active
        && p.alpha.get() == 255
        && p.transition.get().is_none()
        && p.layout.get()
            == (
                active,
                dock_id(&view.dock),
                notice_id(&view),
                p.presentation.borrow().compact,
            )
    {
        if let Some(directory) = std::env::var_os("HUSHLINE_OVERLAY_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            let _ = std::fs::create_dir_all(&directory);
            let data: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
            let _ = std::fs::write(directory.join(format!("bar-{width}x{height}.bgra")), &data);
            let _ = std::fs::write(
                directory.join(format!(
                    "{}-{}-bar-{width}x{height}.bgra",
                    view.theme, view.phase
                )),
                data,
            );
        }
    }
    let _ = EndPaint(hwnd, &ps);
}

fn mix_color(a: COLORREF, b: COLORREF, t: f64) -> COLORREF {
    let channel = |shift: u32| {
        let a = ((a.0 >> shift) & 255u32) as f64;
        let b = ((b.0 >> shift) & 255u32) as f64;
        (a + (b - a) * t).round() as u8
    };
    rgb(channel(0), channel(8), channel(16))
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam.0 as *const CREATESTRUCTW);
        let data = &*(create.lpCreateParams as *const Painter);
        data.claimed.set(true);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        return LRESULT(1);
    }
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Painter;
    if pointer.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    Rc::increment_strong_count(pointer);
    let p = Rc::from_raw(pointer);
    match message {
        #[cfg(test)]
        QUERY_PAINT => LRESULT(p.alpha.get() as isize | if p.paint_ok.get() { 256 } else { 0 }),
        #[cfg(test)]
        QUERY_SURFACES => LRESULT(p.surface_builds.get() as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_SETCURSOR if lparam.0 as u16 as u32 == HTCLIENT => {
            if let Ok(cursor) = LoadCursorW(None, IDC_ARROW) {
                SetCursor(Some(cursor));
            }
            LRESULT(1)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            paint(hwnd, &p);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            p.dpi.set(0);
            position(hwnd, &p, false);
            LRESULT(0)
        }
        UPDATE => {
            if let Some(bridge) = BRIDGE.get() {
                bridge.pending.store(false, Ordering::Release);
            }
            let view = p.view.lock().map(|v| v.clone()).unwrap_or_default();
            let active = view.phase != "idle" && !view.phase.is_empty();
            let dock = dock_id(&view.dock);
            if p.presentation.borrow_mut().sync(&view) {
                let duration = details_ms(&view);
                p.details_deadline.set(Some(
                    Instant::now() + Duration::from_millis(duration as u64),
                ));
                SetTimer(Some(hwnd), 3, duration, None);
            } else if !active {
                p.details_deadline.set(None);
                let _ = KillTimer(Some(hwnd), 3);
            }
            // WM_TIMER is low priority. Apply deadlines during meter traffic too,
            // so a busy message queue cannot indefinitely postpone contraction.
            if p.details_deadline
                .get()
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                p.details_deadline.set(None);
                p.presentation.borrow_mut().compact = true;
                let _ = KillTimer(Some(hwnd), 3);
            }
            let compact = p.presentation.borrow().compact;
            if p.layout.get() != (active, dock, notice_id(&view), compact) {
                // Snap across docks/monitors; unfold upward within the current dock.
                let same_dock = p.layout.get().1 == dock;
                let same_monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)
                    == MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST);
                position(hwnd, &p, same_dock && same_monitor);
            }
            advance_animation(hwnd, &p);
            // Waiting states are static; only real microphone meter updates
            // drive the waveform. No perpetual loading animation or timer.
            let _ = KillTimer(Some(hwnd), 1);
            let label = if active {
                view.message.as_str()
            } else {
                "Ready — click to dictate; right-click for settings"
            };
            let title_changed = p.title.borrow().as_str() != label;
            if title_changed {
                let title: Vec<u16> = format!("Hush · {label}")
                    .encode_utf16()
                    .chain(Some(0))
                    .collect();
                let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(title.as_ptr()));
                *p.title.borrow_mut() = label.to_string();
            }
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == 3 {
                if let Some(deadline) = p.details_deadline.get() {
                    if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                        // A timer message already queued for an older session
                        // must not collapse freshly opened details.
                        SetTimer(Some(hwnd), 3, remaining.as_millis().max(1) as u32, None);
                        return LRESULT(0);
                    }
                }
                let _ = KillTimer(Some(hwnd), 3);
                p.details_deadline.set(None);
                let should_contract = {
                    let presentation = p.presentation.borrow();
                    presentation.active && !presentation.compact
                };
                if should_contract {
                    p.presentation.borrow_mut().compact = true;
                    position(hwnd, &p, true);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            } else if wparam.0 == 2 {
                advance_animation(hwnd, &p);
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            position(hwnd, &p, false);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let view = p.view.lock().map(|v| v.clone()).unwrap_or_default();
            let mut client = RECT::default();
            let _ = GetClientRect(hwnd, &mut client);
            let x = lparam.0 as u16 as i32;
            let y = (lparam.0 >> 16) as u16 as i32;
            let side = matches!(dock_id(&view.dock), 1 | 2) && p.notice.get().bottom == 0;
            let scale = p.dpi.get() as f64 / 96.0;
            let pill = p.pill.get();
            let in_pill = x >= pill.left && x < pill.right && y >= pill.top && y < pill.bottom;
            if let Some(action) = recovery_action(&view, p.notice.get(), scale, x, y) {
                let _ = p.tx.send(action);
                return LRESULT(0);
            }
            let cancel = in_pill
                && view.phase != "idle"
                && if side {
                    y >= client.bottom - (30.0 * scale) as i32
                } else {
                    x >= pill.right - (28.0 * scale) as i32
                };
            let action = if cancel
                || (in_pill && matches!(view.phase.as_str(), "starting" | "processing"))
            {
                Action::Cancel
            } else if !in_pill || view.phase == "error" || view.phase == "preview" {
                Action::Open
            } else {
                Action::HandsFree
            };
            let _ = p.tx.send(action);
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            let _ = p.tx.send(Action::Open);
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = p.tx.send(Action::Cancel);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Rc::from_raw(pointer));
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_notices_stay_readable_without_extending_recording_details() {
        let mut view = OverlayView {
            phase: "listening".into(),
            ..Default::default()
        };
        assert_eq!(details_ms(&view), 2500);
        view.phase = "error".into();
        assert!(details_ms(&view) >= 12_000);
        view.phase = "success".into();
        view.recoverable = true;
        assert!(details_ms(&view) >= 12_000);
        view.recoverable = false;
        view.warning = "Microphone warning".into();
        assert!(details_ms(&view) >= 12_000);
    }
    #[test]
    #[ignore = "Shows owned light/dark recovery cards and checks their native button messages; no microphone, paste, clipboard writes or network."]
    fn native_overlay_recovery_controls() {
        let before = unsafe { GetForegroundWindow() };
        let (tx, rx) = mpsc::channel();
        start(tx).unwrap();
        let hwnd = HWND(BRIDGE.get().unwrap().hwnd.load(Ordering::Relaxed) as *mut _);
        let scale = unsafe { GetDpiForWindow(hwnd) }.max(96) as f64 / 96.0;
        for theme in ["light", "dark"] {
            configure(&Settings {
                theme: theme.into(),
                ..Settings::default()
            });
            update(&View {
                phase: "error".into(),
                message: "Your text is safe".into(),
                warning:
                    "Focus the field you want, then paste here. You can also copy your transcript."
                        .into(),
                recoverable: true,
                last_text: "Public native recovery fixture".into(),
                ..View::default()
            });
            unsafe {
                SendMessageW(hwnd, UPDATE, None, None);
            }
            std::thread::sleep(Duration::from_millis(280));
            let mut client = RECT::default();
            unsafe {
                GetClientRect(hwnd, &mut client).unwrap();
                SendMessageW(hwnd, WM_PAINT, None, None);
            }
            let card = RECT {
                bottom: client.bottom - (32.0 * scale) as i32,
                ..client
            };
            let (paste, copy) = recovery_buttons(card, scale);
            for (rect, expected) in [(paste, Action::Paste), (copy, Action::Copy)] {
                let x = (rect.left + rect.right) / 2;
                let y = (rect.top + rect.bottom) / 2;
                unsafe {
                    SendMessageW(
                        hwnd,
                        WM_LBUTTONUP,
                        Some(WPARAM(0)),
                        Some(LPARAM(((y as isize) << 16) | x as isize)),
                    );
                }
                assert_eq!(
                    rx.recv_timeout(Duration::from_millis(500)).unwrap(),
                    expected
                );
                assert_eq!(
                    unsafe { GetForegroundWindow() },
                    before,
                    "Recovery stole focus"
                );
            }
        }
        // Recovery must stay usable after the old 2.5-second collapse deadline.
        std::thread::sleep(Duration::from_millis(3100));
        let mut still_expanded = RECT::default();
        unsafe {
            GetClientRect(hwnd, &mut still_expanded).unwrap();
        }
        assert!(still_expanded.bottom > (64.0 * scale) as i32);
        update(&View {
            phase: "idle".into(),
            ..View::default()
        });
    }
    #[test]
    fn recovery_buttons_are_explicit_and_never_apply_a_pending_preview() {
        let mut view = OverlayView {
            phase: "error".into(),
            recoverable: true,
            ..Default::default()
        };
        let card = RECT {
            left: 0,
            top: 0,
            right: 340,
            bottom: 130,
        };
        for scale in [1.0, 1.5, 2.0] {
            let scaled = RECT {
                right: (340.0 * scale) as i32,
                bottom: (130.0 * scale) as i32,
                ..card
            };
            let (paste, copy) = recovery_buttons(scaled, scale);
            assert!(paste.right < copy.left && paste.top >= 0 && copy.bottom < scaled.bottom);
            assert_eq!(
                recovery_action(&view, scaled, scale, paste.left + 1, paste.top + 1),
                Some(Action::Paste)
            );
            assert_eq!(
                recovery_action(&view, scaled, scale, copy.left + 1, copy.top + 1),
                Some(Action::Copy)
            );
        }
        view.phase = "preview".into();
        assert_eq!(recovery_action(&view, card, 1.0, 20, 100), None);
        view.phase = "listening".into();
        assert_eq!(recovery_action(&view, card, 1.0, 20, 100), None);
    }
    #[test]
    fn timer_precedes_centered_wave_and_cancel_at_every_size() {
        assert_eq!(timer_alignment(true), DT_CENTER);
        assert_eq!(timer_alignment(false), DT_LEFT);
        for (width, height) in [(168.0, 32.0), (168.0, 28.0), (340.0, 32.0)] {
            let (left, top, right, bottom) = timer_bounds(width, height, false);
            assert!(left > 0.0 && top >= 0.0 && bottom <= height);
            assert!(right < wave_origin(width));
            assert!(wave_origin(width) + 52.6 < width - 28.0);
        }
        for height in [112.0, 128.0] {
            let (_, top, _, bottom) = timer_bounds(40.0, height, true);
            assert!(top > 0.0 && bottom < wave_origin(height));
            assert!(wave_origin(height) + 52.6 < height - 24.0);
        }
    }
    #[test]
    fn preparation_and_processing_never_react_to_meter_or_run_wave_motion() {
        let mut view = OverlayView {
            phase: "starting".into(),
            level: 0.9,
            ..Default::default()
        };
        assert_eq!(recording_level(&view), 0.0);
        view.phase = "processing".into();
        assert_eq!(recording_level(&view), 0.0);
        assert_eq!(
            ["preparing_audio", "transcribing", "polishing", "inserting"].map(processing_progress),
            [2, 5, 8, 11]
        );
        view.phase = "listening".into();
        assert_eq!(recording_level(&view), 0.9);
        view.level = 10.0;
        assert_eq!(recording_level(&view), 1.0);
    }
    #[test]
    fn meter_updates_do_not_reopen_collapsed_details() {
        let mut presentation = Presentation::default();
        let mut view = OverlayView {
            generation: 1,
            phase: "listening".into(),
            ..OverlayView::default()
        };
        assert!(presentation.sync(&view));
        presentation.compact = true;
        for n in 0..150 {
            view.level = n as f32 / 150.0;
            view.seconds = n as f32 * 0.02;
            assert!(!presentation.sync(&view));
            assert!(presentation.compact);
        }
        view.phase = "processing".into();
        assert!(!presentation.sync(&view));
        assert!(presentation.compact);
        view.warning = "Please review this recording".into();
        assert!(presentation.sync(&view));
        assert!(!presentation.compact);
        presentation.compact = true;
        assert!(!presentation.sync(&view));
        view.phase = "idle".into();
        assert!(!presentation.sync(&view));
        assert!(!presentation.compact);
        view.phase = "listening".into();
        view.generation += 1;
        assert!(presentation.sync(&view));
        for extent in [28.0, 88.0, 96.0, 128.0, 168.0, 340.0] {
            assert!((wave_origin(extent) + 52.6 / 2.0 - extent / 2.0).abs() < 0.001);
        }
    }
    #[test]
    fn unfolding_keeps_the_lower_edge_anchored_at_every_dock_and_scale() {
        let work = RECT {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1040,
        };
        for scale in [1.0, 1.5, 2.0] {
            for dock in [0, 1, 2] {
                let side = dock != 0;
                let rect = |active| {
                    let (w, h) = dimensions(active, side);
                    dock_bounds(work, dock, (w * scale) as i32, (h * scale) as i32, scale)
                };
                let rest = rect(false);
                let active = rect(true);
                assert_eq!(rest.bottom, active.bottom);
                assert!(active.top < rest.top);
                if dock == 1 {
                    assert_eq!(rest.left, active.left);
                }
                if dock == 2 {
                    assert_eq!(rest.right, active.right);
                }
            }
        }
        for entering in [true, false] {
            let values: Vec<_> = (0..=20)
                .map(|i| eased_progress(i as f64 / 20.0, entering))
                .collect();
            assert_eq!(values[0], 0.0);
            assert_eq!(values[20], 1.0);
            assert!(values.windows(2).all(|v| v[0] <= v[1]));
        }
    }
    #[test]
    fn pill_edges_have_fractional_alpha_and_correct_premultiplication() {
        assert_eq!(coverage(0.0, 0.0, 48.0, 16.0, 8.0), 0.0);
        assert_eq!(coverage(24.0, 8.0, 48.0, 16.0, 8.0), 1.0);
        let alpha = coverage(3.5, 1.5, 48.0, 16.0, 8.0);
        assert!(alpha > 0.0 && alpha < 1.0);
        let mut pixel = 0;
        composite(&mut pixel, rgb(255, 100, 40), alpha);
        assert_eq!((pixel >> 16) & 255, pixel >> 24);
        assert!((pixel & 255) < pixel >> 24);
    }
    #[test]
    fn notices_include_the_reason_and_recovery_warning_but_not_old_session_text() {
        let mut v = OverlayView {
            phase: "error".into(),
            message: "Microphone unavailable".into(),
            warning: "Choose another device in Settings".into(),
            ..OverlayView::default()
        };
        assert_eq!(
            notice_text(&v),
            "Microphone unavailable\n\nChoose another device in Settings"
        );
        v.phase = "listening".into();
        assert_eq!(notice_text(&v), "Choose another device in Settings");
        v.phase = "success".into();
        assert_eq!(notice_text(&v), "Choose another device in Settings");
    }
    #[test]
    fn hands_free_activation_notice_survives_ready_without_rearming_compact_details() {
        let mut view = OverlayView {
            generation: 1,
            phase: "starting".into(),
            message: "Hands-free · press the shortcut again to stop".into(),
            ..Default::default()
        };
        let mut presentation = Presentation::default();
        assert_eq!(notice_text(&view), view.message);
        assert!(presentation.sync(&view));
        view.phase = "listening".into();
        assert_eq!(notice_text(&view), view.message);
        assert!(!presentation.sync(&view));
        presentation.compact = true; // The existing details deadline expired.
        for reading in 0..150 {
            view.level = 0.6;
            view.seconds = reading as f32 * 0.02;
            assert!(!presentation.sync(&view));
            assert!(presentation.compact);
        }
        view.phase = "processing".into();
        assert!(notice_text(&view).is_empty());
        view.phase = "listening".into();
        view.message = "Listening".into();
        assert!(notice_text(&view).is_empty());
    }
    #[test]
    #[ignore = "Shows synthetic 50-Hz voice activity for five seconds; no microphone or network."]
    fn native_overlay_active_workload() {
        use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
        fn cpu() -> f64 {
            unsafe {
                let (mut created, mut exit, mut kernel, mut user) = (
                    FILETIME::default(),
                    FILETIME::default(),
                    FILETIME::default(),
                    FILETIME::default(),
                );
                GetProcessTimes(
                    GetCurrentProcess(),
                    &mut created,
                    &mut exit,
                    &mut kernel,
                    &mut user,
                )
                .unwrap();
                let ticks = |v: FILETIME| (v.dwHighDateTime as u64) << 32 | v.dwLowDateTime as u64;
                (ticks(kernel) + ticks(user)) as f64 / 10_000_000.0
            }
        }
        let (tx, _) = mpsc::channel();
        start(tx).unwrap();
        configure(&Settings::default());
        update(&View {
            phase: "listening".into(),
            message: "Synthetic voice activity".into(),
            ..View::default()
        });
        std::thread::sleep(Duration::from_millis(2900));
        let bridge = BRIDGE.get().unwrap();
        let hwnd = HWND(bridge.hwnd.load(Ordering::Relaxed) as *mut _);
        let surfaces = unsafe { SendMessageW(hwnd, QUERY_SURFACES, None, None).0 };
        let started = Instant::now();
        let before = cpu();
        let mut readings = 0;
        while started.elapsed() < Duration::from_secs(5) {
            update(&View {
                phase: "listening".into(),
                message: "Synthetic voice activity".into(),
                level: (readings as f32 * 0.12).sin().abs(),
                seconds: started.elapsed().as_secs_f32(),
                ..View::default()
            });
            readings += 1;
            std::thread::sleep(Duration::from_millis(20));
        }
        let used = cpu() - before;
        let logical = std::thread::available_parallelism().unwrap().get();
        println!("Synthetic meter readings: {readings}; native CPU seconds: {used:.4}; all-logical CPU: {:.3}%",
            used/started.elapsed().as_secs_f64()/logical as f64*100.0);
        assert_eq!(
            unsafe { SendMessageW(hwnd, QUERY_SURFACES, None, None).0 },
            surfaces
        );
        update(&View {
            phase: "idle".into(),
            ..View::default()
        });
    }
    #[test]
    #[ignore = "Shows the real native overlay briefly; requires a Windows desktop without concurrent focus changes."]
    fn native_overlay_does_not_activate() {
        let before = unsafe { GetForegroundWindow() };
        let (tx, _) = mpsc::channel();
        start(tx).unwrap();
        let initial = BRIDGE.get().unwrap();
        let initial_hwnd = HWND(initial.hwnd.load(Ordering::Relaxed) as *mut _);
        assert_ne!(
            unsafe { GetClassLongPtrW(initial_hwnd, GCLP_HCURSOR) },
            0,
            "Overlay has no explicit cursor and can inherit the busy cursor"
        );
        assert_eq!(
            unsafe {
                SendMessageW(
                    initial_hwnd,
                    WM_SETCURSOR,
                    None,
                    Some(LPARAM(HTCLIENT as isize)),
                )
                .0
            },
            1
        );
        // Exercise the empty initial status that can be painted before first use.
        unsafe {
            let _ = SendMessageW(initial_hwnd, WM_PAINT, None, None);
        }
        configure(&Settings::default());
        update(&View {
            phase: "idle".into(),
            ..View::default()
        });
        let mut resting = RECT::default();
        unsafe {
            SendMessageW(initial_hwnd, UPDATE, None, None);
            GetWindowRect(initial_hwnd, &mut resting).unwrap();
        }
        update(&View {
            phase: "listening".into(),
            message: "Native overlay verification".into(),
            level: 0.6,
            seconds: 1.2,
            ..View::default()
        });
        let bridge = BRIDGE.get().unwrap();
        let hwnd = HWND(bridge.hwnd.load(Ordering::Relaxed) as *mut _);
        unsafe {
            SendMessageW(hwnd, UPDATE, None, None);
        }
        std::thread::sleep(Duration::from_millis(40));
        let mut opening = RECT::default();
        unsafe {
            GetWindowRect(hwnd, &mut opening).unwrap();
        }
        assert_eq!(
            opening.bottom, resting.bottom,
            "Opening moved the resting lower edge"
        );
        let wait = std::time::Instant::now();
        while !unsafe { IsWindowVisible(hwnd) }.as_bool() && wait.elapsed() < Duration::from_secs(2)
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(unsafe { IsWindowVisible(hwnd) }.as_bool());
        assert_eq!(
            unsafe { GetForegroundWindow() },
            before,
            "Overlay changed keyboard focus"
        );
        let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        assert_ne!(ex & WS_EX_NOACTIVATE.0, 0);
        assert_ne!(ex & WS_EX_TOOLWINDOW.0, 0);
        let scale = unsafe { GetDpiForWindow(hwnd) }.max(96) as f64 / 96.0;
        let mut bounds = RECT::default();
        let settled = Instant::now();
        loop {
            unsafe {
                GetWindowRect(hwnd, &mut bounds).unwrap();
            }
            if bounds.right - bounds.left == (168.0 * scale) as i32
                || settled.elapsed() >= Duration::from_secs(1)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(bounds.right - bounds.left, (168.0 * scale) as i32);
        assert_eq!(bounds.bottom - bounds.top, (32.0 * scale) as i32);
        assert!(
            contains_point(POINT {
                x: (bounds.left + bounds.right) / 2,
                y: (bounds.top + bounds.bottom) / 2
            }),
            "Native pill hit test must recognize the stop surface"
        );
        assert!(!contains_point(POINT {
            x: bounds.left - 20,
            y: bounds.top - 20
        }));
        let surface_count = unsafe { SendMessageW(hwnd, QUERY_SURFACES, None, None).0 };
        for _ in 0..20 {
            unsafe {
                SendMessageW(hwnd, WM_PAINT, None, None);
            }
        }
        assert_eq!(
            unsafe { SendMessageW(hwnd, QUERY_SURFACES, None, None).0 },
            surface_count,
            "Steady-state painting reallocated the native drawing surface"
        );
        let state = unsafe {
            SendMessageW(hwnd, WM_PAINT, None, None);
            SendMessageW(hwnd, QUERY_PAINT, None, None).0
        };
        assert_eq!(state & 255, 255, "Active bar must be fully opaque");
        assert_ne!(
            state & 256,
            0,
            "Per-pixel alpha surface was rejected by Windows"
        );
        configure(&Settings {
            dock: "left".into(),
            ..Settings::default()
        });
        std::thread::sleep(Duration::from_millis(250));
        unsafe {
            GetWindowRect(hwnd, &mut bounds).unwrap();
        }
        assert_eq!(bounds.right - bounds.left, (40.0 * scale) as i32);
        assert_eq!(bounds.bottom - bounds.top, (128.0 * scale) as i32);
        // The real wrapped error surface must remain nonactivating at every dock.
        for dock in ["bottom", "left", "right"] {
            configure(&Settings {
                dock: dock.into(),
                ..Settings::default()
            });
            update(&View { phase:"error".into(),
                message:"Your microphone is unavailable. Reconnect it or choose another microphone in Settings.".into(),
                warning:"Click this message to open Hush. Your clipboard has not changed.".into(),
                ..View::default() });
            std::thread::sleep(Duration::from_millis(280));
            unsafe {
                GetWindowRect(hwnd, &mut bounds).unwrap();
            }
            assert!(bounds.right - bounds.left >= (330.0 * scale) as i32);
            assert!(bounds.bottom - bounds.top > (90.0 * scale) as i32);
            assert_eq!(unsafe { GetForegroundWindow() }, before);
            let painted = unsafe {
                SendMessageW(hwnd, WM_PAINT, None, None);
                SendMessageW(hwnd, QUERY_PAINT, None, None).0
            };
            assert_ne!(painted & 256, 0, "Error card failed to paint");
        }
        configure(&Settings {
            dock: "left".into(),
            ..Settings::default()
        });
        update(&View {
            phase: "idle".into(),
            ..View::default()
        });
        let wait = std::time::Instant::now();
        while wait.elapsed() < Duration::from_millis(250) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(unsafe { IsWindowVisible(hwnd) }.as_bool());
        unsafe {
            GetWindowRect(hwnd, &mut bounds).unwrap();
        }
        assert_eq!(bounds.right - bounds.left, (16.0 * scale) as i32);
        assert_eq!(bounds.bottom - bounds.top, (48.0 * scale) as i32);
        configure(&Settings {
            dock: "hidden".into(),
            ..Settings::default()
        });
        std::thread::sleep(Duration::from_millis(200));
        assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool());
    }

    #[test]
    #[ignore = "Shows the real native bar for three seconds; no microphone or network."]
    fn native_overlay_collapses_without_stopping_recording() {
        let before = unsafe { GetForegroundWindow() };
        let (tx, rx) = mpsc::channel();
        start(tx).unwrap();
        configure(&Settings::default());
        update(&View {
            generation: 1,
            phase: "listening".into(),
            message: "Hands-free · press the shortcut again to stop".into(),
            level: 0.6,
            ..View::default()
        });
        let bridge = BRIDGE.get().unwrap();
        let hwnd = HWND(bridge.hwnd.load(Ordering::Relaxed) as *mut _);
        std::thread::sleep(Duration::from_millis(300));
        unsafe {
            SendMessageW(hwnd, WM_TIMER, Some(WPARAM(3)), None);
        }
        let mut initial = RECT::default();
        unsafe {
            SendMessageW(hwnd, UPDATE, None, None);
            GetWindowRect(hwnd, &mut initial).unwrap()
        };
        for reading in 0..140 {
            update(&View {
                generation: 1,
                phase: "listening".into(),
                message: "Hands-free · press the shortcut again to stop".into(),
                level: 0.6,
                seconds: reading as f32 * 0.02,
                ..View::default()
            });
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut compact = RECT::default();
        unsafe {
            SendMessageW(hwnd, UPDATE, None, None);
            GetWindowRect(hwnd, &mut compact).unwrap()
        };
        let scale = unsafe { GetDpiForWindow(hwnd) }.max(96) as f64 / 96.0;
        assert_eq!(compact.right - compact.left, (168.0 * scale) as i32);
        assert_eq!(compact.bottom - compact.top, (28.0 * scale) as i32);
        assert_eq!(initial.bottom, compact.bottom);
        assert_eq!(bridge.view.lock().unwrap().phase, "listening");
        for dock in ["left", "right"] {
            configure(&Settings {
                dock: dock.into(),
                ..Settings::default()
            });
            unsafe {
                SendMessageW(hwnd, UPDATE, None, None);
                GetWindowRect(hwnd, &mut compact).unwrap();
                SendMessageW(hwnd, WM_PAINT, None, None);
            }
            assert_eq!(compact.right - compact.left, (40.0 * scale) as i32);
            assert_eq!(compact.bottom - compact.top, (112.0 * scale) as i32);
        }
        // The visible close control must remain clickable after contraction.
        let x = (20.0 * scale) as u32;
        let y = (96.0 * scale) as u32;
        unsafe {
            SendMessageW(
                hwnd,
                WM_LBUTTONUP,
                None,
                Some(LPARAM(((y << 16) | x) as isize)),
            );
        }
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            Action::Cancel
        );
        assert_eq!(unsafe { GetForegroundWindow() }, before);
    }
}
