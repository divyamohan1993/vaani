// Vaani helper — serves the voice UI on loopback and types recognized text into
// whatever window is focused. Recognition itself runs in real Chrome (the only
// robust Google Web Speech path); this process is the bridge into native apps.
//
// Design notes:
//  * Injection uses the LIVE foreground window (enigo/SendInput). We never call
//    SetForegroundWindow to redirect text — Windows' foreground lock would defeat
//    it. We suppress typing only when our own Chrome window is foreground (the
//    Win+H model: text lands wherever the user's caret already is).
//  * HTTP-only (no WebSocket): the page short-polls /poll for hotkey/tray events.
//  * Web assets are embedded in the binary, so the helper is fully self-contained.
//  * Threads: main = Win32 message loop (tray + global hotkey); worker = HTTP
//    server owning the single Enigo; worker = Chrome launch + window tracking.

// Windowless only in release; debug builds keep a console for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::VecDeque;
use std::io::{Cursor, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use enigo::{Enigo, Keyboard, Settings};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, Ellipse, EndPaint, FillRect,
    InvalidateRect, Rectangle, RoundRect, SelectObject, HGDIOBJ, PAINTSTRUCT, PS_SOLID,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, ReleaseCapture, SetCapture, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumWindows,
    GetClientRect, GetCursorPos, GetForegroundWindow, GetMessageW, GetSystemMetrics,
    GetWindowLongPtrW, GetWindowRect, GetWindowTextW, IsWindowVisible, LoadCursorW, LoadIconW,
    PostMessageW, PostQuitMessage, RegisterClassW, SetForegroundWindow, IDC_ARROW,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, TrackPopupMenu, TranslateMessage,
    GWL_EXSTYLE, HWND_TOPMOST, IDI_APPLICATION, LWA_ALPHA, LWA_COLORKEY, MF_SEPARATOR, MF_STRING,
    MSG, SM_CXFULLSCREEN, SM_CYFULLSCREEN, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
    TPM_RIGHTBUTTON, WM_APP, WM_COMMAND, WM_DESTROY, WM_HOTKEY, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONUP, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP, WS_VISIBLE,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PORT: u16 = 17653;
const WINDOW_TITLE_NEEDLE: &str = "Vaani"; // chrome --app titles the window by page <title>

const TRAY_MSG: u32 = WM_APP + 1;
const HOTKEY_ID: i32 = 1;
const ID_TOGGLE: usize = 10;
const ID_OPEN: usize = 11;
const ID_QUIT: usize = 12;
const ID_OP100: usize = 20;
const ID_OP75: usize = 21;
const ID_OP50: usize = 22;
const ID_OP25: usize = 23;
const ID_LANG_EN: usize = 30;
const ID_LANG_HI: usize = 31;

// Embedded web UI (paths relative to this source file).
const INDEX_HTML: &str = include_str!("../../app/index.html");
const STYLE_CSS: &str = include_str!("../../app/style.css");
const APP_JS: &str = include_str!("../../app/app.js");

struct Shared {
    chrome_hwnd: AtomicIsize,       // 0 until the Chrome app window is located
    queue: Mutex<VecDeque<String>>, // hotkey/tray commands drained by GET /poll
}

static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

// Native mic-dot window + its state (accessed from the single-threaded message loop
// and the HTTP thread; atomics keep it simple).
static MIC_HWND: AtomicIsize = AtomicIsize::new(0);
static LISTENING: AtomicBool = AtomicBool::new(false);
static MIC_ALPHA: AtomicI32 = AtomicI32::new(255);
// Drag tracking for the borderless dot (message loop thread only).
static DRAG_DOWN_X: AtomicI32 = AtomicI32::new(0);
static DRAG_DOWN_Y: AtomicI32 = AtomicI32::new(0);
static DRAG_WIN_X: AtomicI32 = AtomicI32::new(0);
static DRAG_WIN_Y: AtomicI32 = AtomicI32::new(0);
static DRAGGING: AtomicBool = AtomicBool::new(false);
static CAPTURED: AtomicBool = AtomicBool::new(false);

const MIC_SIZE: i32 = 44;
const KEYCOLOR: u32 = 0x00FF00FF; // magenta → transparent (color key)
const COL_IDLE: u32 = 0x00FF7C4F; // #4f7cff (BGR)
const COL_LIVE: u32 = 0x006D4DFF; // #ff4d6d (BGR)
const COL_WHITE: u32 = 0x00FFFFFF;

fn main() {
    // Single instance: if a Vaani is already serving, raise its window and exit.
    if already_running(DEFAULT_PORT) {
        let _ = raw_request(DEFAULT_PORT, "POST", "/show");
        return;
    }

    let chrome = find_chrome();
    let (server, port) = bind_server(DEFAULT_PORT);
    log(&format!("=== start v{VERSION} port={port} chrome={} ===", chrome.is_some()));

    let shared = Arc::new(Shared {
        chrome_hwnd: AtomicIsize::new(0),
        queue: Mutex::new(VecDeque::new()),
    });
    let _ = SHARED.set(Arc::clone(&shared));

    // Launch Chrome (speech engine + visible mic UI) and keep its window on top.
    {
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || launch_and_track_chrome(chrome, port, shared));
    }

    // HTTP server on a worker thread; one Enigo lives there (single-threaded loop).
    {
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || {
            let mut enigo = Enigo::new(&Settings::default()).expect("enigo init");
            for request in server.incoming_requests() {
                handle(request, &shared, &mut enigo);
            }
        });
    }

    // Main thread owns the Win32 message loop for the tray icon + global hotkey.
    run_message_loop();
}

/* ----------------------------- HTTP handling ----------------------------- */

fn handle(request: Request, shared: &Arc<Shared>, enigo: &mut Enigo) {
    let method = request.method().clone();
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or("/");

    match (&method, path) {
        (Method::Options, _) => reply(request, 204, "text/plain", String::new()),

        (Method::Get, "/health") => reply(
            request,
            200,
            "application/json",
            format!(r#"{{"app":"vaani-helper","version":"{VERSION}"}}"#),
        ),

        (Method::Get, "/") | (Method::Get, "/index.html") => {
            reply(request, 200, "text/html; charset=utf-8", INDEX_HTML.to_string())
        }
        (Method::Get, "/style.css") => {
            reply(request, 200, "text/css; charset=utf-8", STYLE_CSS.to_string())
        }
        (Method::Get, "/app.js") => reply(
            request,
            200,
            "application/javascript; charset=utf-8",
            APP_JS.to_string(),
        ),

        (Method::Get, "/poll") => {
            let cmd = shared.queue.lock().unwrap().pop_front();
            let body = match cmd {
                Some(a) => {
                    if debug_enabled() {
                        log(&format!("/poll deliver {a}"));
                    }
                    format!(r#"{{"action":"{a}"}}"#)
                }
                None => r#"{"action":null}"#.to_string(),
            };
            reply(request, 200, "application/json", body);
        }

        (Method::Post, "/type") => handle_type(request, shared, enigo),
        (Method::Post, "/show") => {
            reset_mic_position();
            reply(request, 200, "application/json", r#"{"ok":true}"#.to_string());
        }
        (Method::Post, "/log") => handle_log(request),
        (Method::Post, "/state") => handle_state(request),

        _ => reply(request, 404, "text/plain", "not found".to_string()),
    }
}

fn handle_type(mut request: Request, shared: &Arc<Shared>, enigo: &mut Enigo) {
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    let text = v.get("text").and_then(|x| x.as_str()).unwrap_or("");
    if text.is_empty() {
        reply(request, 400, "application/json", r#"{"ok":false}"#.to_string());
        return;
    }

    // Suppress if our own Chrome window is foreground — text would land in the UI.
    let fg = unsafe { GetForegroundWindow() };
    let chrome = shared.chrome_hwnd.load(Ordering::Relaxed);
    let suppressed = chrome != 0 && fg.0 as isize == chrome;
    if debug_enabled() {
        log(&format!("/type len={} suppressed={suppressed} text={text:?}", text.len()));
    }
    if suppressed {
        reply(
            request,
            200,
            "application/json",
            r#"{"ok":false,"suppressed":true}"#.to_string(),
        );
        return;
    }

    let ok = enigo.text(text).is_ok();
    reply(request, 200, "application/json", format!(r#"{{"ok":{ok}}}"#));
}

fn handle_log(mut request: Request) {
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    if debug_enabled() {
        log(&format!("PAGE: {body}"));
    }
    reply(request, 200, "application/json", r#"{"ok":true}"#.to_string());
}

// The page reports listening on/off so the dot can recolour (idle ↔ live).
fn handle_state(mut request: Request) {
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    let listening = v.get("listening").and_then(|x| x.as_bool()).unwrap_or(false);
    LISTENING.store(listening, Ordering::Relaxed);
    let h = MIC_HWND.load(Ordering::Relaxed);
    if h != 0 {
        unsafe {
            let _ = InvalidateRect(HWND(h as *mut _), None, BOOL(1));
        }
    }
    reply(request, 200, "application/json", r#"{"ok":true}"#.to_string());
}

fn reply(request: Request, code: u16, ctype: &str, body: String) {
    let bytes = body.into_bytes();
    let len = bytes.len();
    let headers = vec![
        header("Content-Type", ctype),
        header("Access-Control-Allow-Origin", "*"),
        header("Access-Control-Allow-Headers", "Content-Type"),
        header("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
        // Lets an https://type.dmj.one page reach this loopback server (PNA).
        header("Access-Control-Allow-Private-Network", "true"),
        header("Cache-Control", "no-store"),
    ];
    let resp = Response::new(StatusCode(code), headers, Cursor::new(bytes), Some(len), None);
    let _ = request.respond(resp);
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).unwrap()
}

// Lightweight troubleshooting log at %LOCALAPPDATA%\Vaani\vaani.log.
// Never logs recognized text unless VAANI_DEBUG is set (privacy by default).
fn log(msg: &str) {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let dir = format!(r"{local}\Vaani");
    let _ = std::fs::create_dir_all(&dir);
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(format!(r"{dir}\vaani.log")) {
        let _ = writeln!(f, "{t} {msg}");
    }
}

fn debug_enabled() -> bool {
    std::env::var("VAANI_DEBUG").map(|v| !v.is_empty()).unwrap_or(false)
}

/* ----------------------------- Window control ---------------------------- */

// Park the Chrome speech-engine window off-screen and hide its taskbar button.
// It stays "visible" (not minimized) so the recognizer is not throttled.
fn style_chrome_hidden(hwnd: HWND) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW.0 as isize);
        let _ = SetWindowPos(
            hwnd,
            HWND::default(),
            -2000,
            -2000,
            200,
            88,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
        );
    }
}

// Opacity of the mic dot (color-key transparency stays; overall alpha changes).
fn set_mic_alpha(pct: f64) {
    let a = ((pct.clamp(20.0, 100.0) / 100.0) * 255.0) as i32;
    MIC_ALPHA.store(a, Ordering::Relaxed);
    let _ = std::fs::write(alpha_file(), a.to_string());
    let h = MIC_HWND.load(Ordering::Relaxed);
    if h != 0 {
        unsafe {
            let _ = SetLayeredWindowAttributes(
                HWND(h as *mut _),
                COLORREF(KEYCOLOR),
                a as u8,
                LWA_COLORKEY | LWA_ALPHA,
            );
        }
    }
}

fn alpha_file() -> String {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    format!(r"{local}\Vaani\alpha.txt")
}
fn read_alpha() -> Option<i32> {
    std::fs::read_to_string(alpha_file()).ok()?.trim().parse().ok()
}

// Move the dot back to the default bottom-right spot.
fn reset_mic_position() {
    let h = MIC_HWND.load(Ordering::Relaxed);
    if h == 0 {
        return;
    }
    unsafe {
        let sw = GetSystemMetrics(SM_CXFULLSCREEN);
        let sh = GetSystemMetrics(SM_CYFULLSCREEN);
        let x = (sw - MIC_SIZE - 24).max(0);
        let y = (sh - MIC_SIZE - 24).max(0);
        let _ = SetWindowPos(
            HWND(h as *mut _),
            HWND_TOPMOST,
            x,
            y,
            MIC_SIZE,
            MIC_SIZE,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

fn pos_file() -> String {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    format!(r"{local}\Vaani\pos.txt")
}
fn read_pos() -> Option<(i32, i32)> {
    let s = std::fs::read_to_string(pos_file()).ok()?;
    let (a, b) = s.trim().split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}
fn write_pos(x: i32, y: i32) {
    let _ = std::fs::write(pos_file(), format!("{x},{y}"));
}

struct FindCtx {
    needle: String,
    found: isize,
}

unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut FindCtx);
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let mut buf = [0u16; 512];
    let n = GetWindowTextW(hwnd, &mut buf);
    if n > 0 {
        let title = String::from_utf16_lossy(&buf[..n as usize]);
        if title.contains(&ctx.needle) {
            ctx.found = hwnd.0 as isize;
            return BOOL(0); // stop enumeration
        }
    }
    BOOL(1)
}

fn find_window_by_title(needle: &str) -> Option<isize> {
    let mut ctx = FindCtx { needle: needle.to_string(), found: 0 };
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut ctx as *mut _ as isize));
    }
    (ctx.found != 0).then_some(ctx.found)
}

/* ------------------------------- Chrome ---------------------------------- */

fn find_chrome() -> Option<String> {
    if let Ok(p) = std::env::var("VAANI_CHROME") {
        if Path::new(&p).exists() {
            return Some(p);
        }
    }
    let pf = std::env::var("ProgramFiles").unwrap_or(r"C:\Program Files".into());
    let pf86 = std::env::var("ProgramFiles(x86)").unwrap_or(r"C:\Program Files (x86)".into());
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    [
        format!(r"{pf}\Google\Chrome\Application\chrome.exe"),
        format!(r"{pf86}\Google\Chrome\Application\chrome.exe"),
        format!(r"{local}\Google\Chrome\Application\chrome.exe"),
    ]
    .into_iter()
    .find(|c| Path::new(c).exists())
}

fn spawn_chrome(chrome: &str, port: u16) {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let profile = format!(r"{local}\Vaani\chrome-profile");
    let _ = std::fs::create_dir_all(&profile);
    let extra = std::env::var("VAANI_CHROME_ARGS").unwrap_or_default();

    let mut cmd = Command::new(chrome);
    cmd.arg(format!("--app=http://127.0.0.1:{port}/"))
        .arg(format!("--user-data-dir={profile}"))
        .arg("--window-size=200,90")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        // Auto-grant the microphone so first run needs zero clicks. The
        // "unsupported flag" warning bar it triggers is harmless here because
        // the Chrome window lives off-screen and is never seen.
        .arg("--use-fake-ui-for-media-stream");
    for a in extra.split_whitespace() {
        cmd.arg(a);
    }
    let _ = cmd.spawn();
}

fn launch_and_track_chrome(chrome: Option<String>, port: u16, shared: Arc<Shared>) {
    let Some(chrome) = chrome else { return };
    spawn_chrome(&chrome, port);
    for _ in 0..40 {
        if let Some(h) = find_window_by_title(WINDOW_TITLE_NEEDLE) {
            shared.chrome_hwnd.store(h, Ordering::Relaxed);
            style_chrome_hidden(HWND(h as *mut _));
            log(&format!("chrome window found hwnd={h}"));
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    log("chrome window NOT found after retries");
}

fn push_cmd(action: &str) {
    if let Some(s) = SHARED.get() {
        s.queue.lock().unwrap().push_back(action.to_string());
    }
}

/* ------------------------------- Server ---------------------------------- */

// Minimal loopback HTTP probe/POST without pulling in a client dependency.
fn raw_request(port: u16, method: &str, path: &str) -> Option<String> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(Duration::from_millis(700))).ok();
    let req = format!(
        "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    s.write_all(req.as_bytes()).ok()?;
    let mut resp = String::new();
    s.read_to_string(&mut resp).ok();
    Some(resp)
}

fn already_running(port: u16) -> bool {
    raw_request(port, "GET", "/health")
        .map(|r| r.contains("vaani-helper"))
        .unwrap_or(false)
}

fn bind_server(pref: u16) -> (Server, u16) {
    for p in pref..pref.saturating_add(12) {
        if let Ok(s) = Server::http(("127.0.0.1", p)) {
            return (s, p);
        }
    }
    let s = Server::http(("127.0.0.1", 0)).expect("bind loopback");
    let port = s.server_addr().to_ip().map(|a| a.port()).unwrap_or(0);
    (s, port)
}

/* --------------------- Native mic dot + tray + hotkey -------------------- */

fn run_message_loop() {
    unsafe {
        let hinstance = GetModuleHandleW(None).expect("module handle");
        let class_name = w!("VaaniMic");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);

        // Default bottom-right, or the user's saved position. (Empty title so the
        // chrome-window finder, which matches "Vaani", never picks this window.)
        let (x, y) = read_pos().unwrap_or_else(|| {
            let sw = GetSystemMetrics(SM_CXFULLSCREEN);
            let sh = GetSystemMetrics(SM_CYFULLSCREEN);
            ((sw - MIC_SIZE - 24).max(0), (sh - MIC_SIZE - 24).max(0))
        });

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!(""),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            MIC_SIZE,
            MIC_SIZE,
            None,
            None,
            hinstance,
            None,
        )
        .expect("create mic window");

        if let Some(a) = read_alpha() {
            MIC_ALPHA.store(a.clamp(51, 255), Ordering::Relaxed);
        }
        let _ = SetLayeredWindowAttributes(
            hwnd,
            COLORREF(KEYCOLOR),
            MIC_ALPHA.load(Ordering::Relaxed) as u8,
            LWA_COLORKEY | LWA_ALPHA,
        );
        MIC_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

        // Global hotkey: Ctrl+Alt+Space toggles listening.
        let _ = RegisterHotKey(hwnd, HOTKEY_ID, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x20);

        add_tray_icon(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        remove_tray_icon(hwnd);
    }
}

// Paint the dot: a coloured circle (idle/live) with a white microphone glyph.
unsafe fn paint_mic(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    let (w, h) = (rc.right, rc.bottom);

    let keyb = CreateSolidBrush(COLORREF(KEYCOLOR));
    FillRect(hdc, &rc, keyb);

    let col = if LISTENING.load(Ordering::Relaxed) { COL_LIVE } else { COL_IDLE };
    let brush = CreateSolidBrush(COLORREF(col));
    let pen = CreatePen(PS_SOLID, 1, COLORREF(col));
    let ob = SelectObject(hdc, HGDIOBJ(brush.0));
    let op = SelectObject(hdc, HGDIOBJ(pen.0));
    let _ = Ellipse(hdc, 2, 2, w - 2, h - 2);

    let white = CreateSolidBrush(COLORREF(COL_WHITE));
    let wpen = CreatePen(PS_SOLID, 1, COLORREF(COL_WHITE));
    SelectObject(hdc, HGDIOBJ(white.0));
    SelectObject(hdc, HGDIOBJ(wpen.0));
    // Microphone glyph, scaled to the window so it works at any dot size.
    let cx = w / 2;
    let hw = (w * 12 / 100).max(3); // head half-width
    let head_t = h * 27 / 100;
    let head_b = h * 60 / 100;
    let stem_b = h * 73 / 100;
    let base_hw = (w * 18 / 100).max(4);
    let base_b = h * 81 / 100;
    let sw = (w * 3 / 100).max(1); // stem half-width
    let _ = RoundRect(hdc, cx - hw, head_t, cx + hw, head_b, hw * 2, hw * 2); // capsule head
    let _ = Rectangle(hdc, cx - sw, head_b, cx + sw, stem_b); // stem
    let _ = RoundRect(hdc, cx - base_hw, stem_b, cx + base_hw, base_b, 4, 4); // base

    SelectObject(hdc, ob);
    SelectObject(hdc, op);
    let _ = DeleteObject(HGDIOBJ(brush.0));
    let _ = DeleteObject(HGDIOBJ(pen.0));
    let _ = DeleteObject(HGDIOBJ(white.0));
    let _ = DeleteObject(HGDIOBJ(wpen.0));
    let _ = DeleteObject(HGDIOBJ(keyb.0));
    let _ = EndPaint(hwnd, &ps);
}

fn tray_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    };
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.uCallbackMessage = TRAY_MSG;
    unsafe {
        if let Ok(icon) = LoadIconW(None, IDI_APPLICATION) {
            data.hIcon = icon;
        }
    }
    let tip: Vec<u16> = "Vaani — voice typing (Ctrl+Alt+Space)".encode_utf16().collect();
    for (i, c) in tip.iter().take(data.szTip.len() - 1).enumerate() {
        data.szTip[i] = *c;
    }
    data
}

fn add_tray_icon(hwnd: HWND) {
    let data = tray_data(hwnd);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &data);
    }
}

fn remove_tray_icon(hwnd: HWND) {
    let data = tray_data(hwnd);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

fn show_tray_menu(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let _ = AppendMenuW(menu, MF_STRING, ID_TOGGLE, w!("Start / stop listening\tCtrl+Alt+Space"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, ID_LANG_EN, w!("Language: English"));
        let _ = AppendMenuW(menu, MF_STRING, ID_LANG_HI, w!("Language: हिन्दी"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, ID_OP100, w!("Transparency: off (solid)"));
        let _ = AppendMenuW(menu, MF_STRING, ID_OP75, w!("Transparency: low"));
        let _ = AppendMenuW(menu, MF_STRING, ID_OP50, w!("Transparency: medium"));
        let _ = AppendMenuW(menu, MF_STRING, ID_OP25, w!("Transparency: high (faint)"));
        let _ = AppendMenuW(menu, MF_STRING, ID_OPEN, w!("Reset position"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, ID_QUIT, w!("Quit Vaani"));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd); // so the menu dismisses on click-away
        let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, None);
        let _ = PostMessageW(hwnd, 0, WPARAM(0), LPARAM(0));
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                paint_mic(hwnd);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let mut p = POINT::default();
                let _ = GetCursorPos(&mut p);
                DRAG_DOWN_X.store(p.x, Ordering::Relaxed);
                DRAG_DOWN_Y.store(p.y, Ordering::Relaxed);
                let mut r = RECT::default();
                let _ = GetWindowRect(hwnd, &mut r);
                DRAG_WIN_X.store(r.left, Ordering::Relaxed);
                DRAG_WIN_Y.store(r.top, Ordering::Relaxed);
                DRAGGING.store(false, Ordering::Relaxed);
                CAPTURED.store(true, Ordering::Relaxed);
                SetCapture(hwnd);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if CAPTURED.load(Ordering::Relaxed) && (wparam.0 & 0x0001) != 0 {
                    let mut p = POINT::default();
                    let _ = GetCursorPos(&mut p);
                    let dx = p.x - DRAG_DOWN_X.load(Ordering::Relaxed);
                    let dy = p.y - DRAG_DOWN_Y.load(Ordering::Relaxed);
                    if !DRAGGING.load(Ordering::Relaxed) && dx.abs() + dy.abs() > 4 {
                        DRAGGING.store(true, Ordering::Relaxed);
                    }
                    if DRAGGING.load(Ordering::Relaxed) {
                        let nx = DRAG_WIN_X.load(Ordering::Relaxed) + dx;
                        let ny = DRAG_WIN_Y.load(Ordering::Relaxed) + dy;
                        let _ = SetWindowPos(
                            hwnd,
                            HWND::default(),
                            nx,
                            ny,
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if CAPTURED.load(Ordering::Relaxed) {
                    let _ = ReleaseCapture();
                    CAPTURED.store(false, Ordering::Relaxed);
                    if DRAGGING.load(Ordering::Relaxed) {
                        let mut r = RECT::default();
                        let _ = GetWindowRect(hwnd, &mut r);
                        write_pos(r.left, r.top);
                    } else {
                        push_cmd("toggle"); // a click (not a drag) toggles listening
                    }
                }
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                show_tray_menu(hwnd);
                LRESULT(0)
            }
            WM_HOTKEY => {
                if wparam.0 as i32 == HOTKEY_ID {
                    push_cmd("toggle");
                }
                LRESULT(0)
            }
            TRAY_MSG => {
                let event = (lparam.0 as u32) & 0xFFFF;
                if event == WM_RBUTTONUP {
                    show_tray_menu(hwnd);
                } else if event == WM_LBUTTONUP {
                    push_cmd("toggle");
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                match wparam.0 & 0xFFFF {
                    ID_TOGGLE => push_cmd("toggle"),
                    ID_OPEN => reset_mic_position(),
                    ID_OP100 => set_mic_alpha(100.0),
                    ID_OP75 => set_mic_alpha(75.0),
                    ID_OP50 => set_mic_alpha(50.0),
                    ID_OP25 => set_mic_alpha(25.0),
                    ID_LANG_EN => push_cmd("lang:en-IN"),
                    ID_LANG_HI => push_cmd("lang:hi-IN"),
                    ID_QUIT => {
                        // Close our hidden Chrome window, then exit the loop.
                        if let Some(s) = SHARED.get() {
                            let h = s.chrome_hwnd.load(Ordering::Relaxed);
                            if h != 0 {
                                let _ = PostMessageW(HWND(h as *mut _), 0x0010, WPARAM(0), LPARAM(0)); // WM_CLOSE
                            }
                        }
                        PostQuitMessage(0);
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
