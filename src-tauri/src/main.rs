#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::broadcast,
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::handshake::server::{Request, Response},
    tungstenite::Message,
};

#[derive(Debug, Deserialize)]
struct ProxyPayload {
    endpoint: Option<String>,
    method: Option<String>,
    #[serde(rename = "baseUrl")]
    base_url: Option<String>,
    body: Option<serde_json::Value>,
    #[serde(default)]
    headers: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct ProxyErrorResponse {
    success: bool,
    code: i32,
    msg: String,
}

async fn handle_proxy_request(client: &reqwest::Client, body_bytes: &[u8]) -> (u16, String, String) {
    let payload: ProxyPayload = match serde_json::from_slice(body_bytes) {
        Ok(p) => p,
        Err(e) => {
            let err_json = serde_json::to_string(&ProxyErrorResponse {
                success: false,
                code: 400,
                msg: format!("Invalid proxy JSON body: {e}"),
            })
            .unwrap_or_default();
            return (400, "application/json".into(), err_json);
        }
    };

    let base_url = payload.base_url.unwrap_or_default();
    if base_url.is_empty() {
        let err_json = serde_json::to_string(&ProxyErrorResponse {
            success: false,
            code: 400,
            msg: "Missing baseUrl".into(),
        })
        .unwrap_or_default();
        return (400, "application/json".into(), err_json);
    }

    let mut endpoint = payload.endpoint.unwrap_or_default();
    if !endpoint.starts_with('/') {
        endpoint = format!("/{endpoint}");
    }
    let target_url = format!("{base_url}{endpoint}");

    let method_str = payload.method.unwrap_or_else(|| "GET".into()).to_uppercase();
    let method = match method_str.as_str() {
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        _ => reqwest::Method::GET,
    };

    let mut req = client.request(method.clone(), &target_url);

    // Inject headers (especially Origin, Authorization, token, etc.)
    for (key, val) in payload.headers {
        let key_lower = key.to_lowercase();
        if key_lower == "content-length" || key_lower == "host" {
            continue;
        }
        if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
            if let Ok(header_val) = reqwest::header::HeaderValue::from_str(&val) {
                req = req.header(header_name, header_val);
            }
        }
    }

    if let Some(body_val) = payload.body {
        if method != reqwest::Method::GET {
            req = req.json(&body_val);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/json")
                .to_string();
            let text = resp.text().await.unwrap_or_default();
            (status, content_type, text)
        }
        Err(e) => {
            let err_json = serde_json::to_string(&ProxyErrorResponse {
                success: false,
                code: 502,
                msg: format!("Proxy target request failed: {e}"),
            })
            .unwrap_or_default();
            (502, "application/json".into(), err_json)
        }
    }
}

/// Single Unified Server on port 44567:
/// - Handles WebSocket connection on standard handshake for Turbo Bar sync.
/// - Handles HTTP OPTIONS / POST proxy / GET probe on the same port.
/// - Lifecycle auto-exit: automatically exits the process when all clients disconnect (e.g. plugin reload/closed, PS quit).
async fn serve_unified(app_handle: tauri::AppHandle, _turbo_active: Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:44567")
        .await
        .expect("bind Turbo Bar unified socket (44567)");

    let (tx, _) = broadcast::channel::<String>(32);
    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(45))
            .build()
            .expect("create reqwest client"),
    );

    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            continue;
        };

        let tx = tx.clone();
        let client = Arc::clone(&client);
        let app_handle_child = app_handle.clone();
        #[cfg(target_os = "macos")]
        let turbo_active_ws = Arc::clone(&_turbo_active);

        tokio::spawn(async move {
            // Peek at incoming bytes to check if this is an HTTP or WebSocket handshake
            let mut peek_buf = [0u8; 1024];
            let n = match stream.peek(&mut peek_buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };

            let Ok(req_str) = std::str::from_utf8(&peek_buf[..n]) else {
                return;
            };

            let is_websocket = req_str.lines().any(|l| {
                let lower = l.to_lowercase();
                lower.starts_with("upgrade:") && lower.contains("websocket")
            });

            if is_websocket {
                // Check if this WebSocket connection is from the Photoshop UXP host plugin
                let is_uxp_host = req_str.lines().any(|l| {
                    let low = l.to_lowercase();
                    low.contains("client=uxp") || low.contains("photoshop") || low.contains("adobe")
                });

                let callback = |_req: &Request, resp: Response| Ok(resp);
                let Ok(ws) = accept_hdr_async(stream, callback).await else {
                    return;
                };

                let (mut writer, mut reader) = ws.split();
                let mut rx = tx.subscribe();

                loop {
                    tokio::select! {
                        msg = reader.next() => match msg {
                            Some(Ok(Message::Text(text))) => {
                                // Check mode from UXP state messages to control PS focus tracker
                                #[cfg(target_os = "macos")]
                                if is_uxp_host {
                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                                        if v.get("type").and_then(|t| t.as_str()) == Some("state") {
                                            let is_turbo = v.pointer("/payload/mode")
                                                .and_then(|m| m.as_str())
                                                == Some("turbo");
                                            let was_turbo = turbo_active_ws.swap(is_turbo, Ordering::Relaxed);
                                            if is_turbo {
                                                // If newly activated, start tracker
                                                if !was_turbo {
                                                    if let Some(win) = app_handle_child.get_webview_window("turbo") {
                                                        spawn_ps_focus_tracker(win, Arc::clone(&turbo_active_ws));
                                                    }
                                                } else {
                                                    // Already turbo, ensure window is brought forward if PS is frontmost
                                                    if is_ps_or_bar_frontmost() {
                                                        if let Some(win) = app_handle_child.get_webview_window("turbo") {
                                                            show_turbo_bar(&win);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                let _ = tx.send(text.to_string());
                            }
                            Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                            _ => {}
                        },
                        msg = rx.recv() => match msg {
                            Ok(text) => { if writer.send(Message::Text(text.into())).await.is_err() { break; } }
                            Err(broadcast::error::RecvError::Lagged(_)) => {}
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }

                // Exit when the ComfyPanel UXP plugin disconnects.
                if is_uxp_host {
                    app_handle_child.exit(0);
                }
            } else {
                // Regular HTTP Handling (CORS preflight, proxy request, or probe)
                let mut buf = Vec::new();
                let mut temp = [0u8; 4096];
                let mut header_end_idx = None;
                let mut content_length: usize = 0;

                loop {
                    let n = match stream.read(&mut temp).await {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => return,
                    };
                    buf.extend_from_slice(&temp[..n]);

                    if header_end_idx.is_none() {
                        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            header_end_idx = Some(pos + 4);
                            if let Ok(headers_str) = std::str::from_utf8(&buf[..pos]) {
                                for line in headers_str.lines() {
                                    if line.to_lowercase().starts_with("content-length:") {
                                        if let Some(val_str) = line.split(':').nth(1) {
                                            content_length = val_str.trim().parse().unwrap_or(0);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(h_end) = header_end_idx {
                        if buf.len() >= h_end + content_length {
                            break;
                        }
                    }
                }

                let Some(h_end) = header_end_idx else {
                    return;
                };

                let Ok(req_head) = std::str::from_utf8(&buf[..h_end]) else {
                    return;
                };

                let first_line = req_head.lines().next().unwrap_or_default();
                let mut parts = first_line.split_whitespace();
                let method = parts.next().unwrap_or("GET");
                let path = parts.next().unwrap_or("/");

                // CORS Preflight
                if method == "OPTIONS" {
                    let response = "HTTP/1.1 204 No Content\r\n\
Access-Control-Allow-Origin: *\r\n\
Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
Access-Control-Allow-Headers: Content-Type, Authorization, Origin, token\r\n\
Access-Control-Max-Age: 86400\r\n\
Connection: close\r\n\r\n";
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                }

                // HTTP Proxy: /comfypanel/runninghub/proxy or /proxy
                if method == "POST" && (path == "/comfypanel/runninghub/proxy" || path == "/proxy") {
                    let body_slice = &buf[h_end..h_end + content_length];
                    let (status, content_type, res_text) = handle_proxy_request(&client, body_slice).await;

                    let response_headers = format!(
                        "HTTP/1.1 {status} OK\r\n\
Access-Control-Allow-Origin: *\r\n\
Content-Type: {content_type}\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\r\n",
                        res_text.len()
                    );

                    let _ = stream.write_all(response_headers.as_bytes()).await;
                    let _ = stream.write_all(res_text.as_bytes()).await;
                    return;
                }

                // Default probe response for GET / or any readiness check
                let default_res = "HTTP/1.1 200 OK\r\n\
Access-Control-Allow-Origin: *\r\n\
Content-Length: 2\r\n\
Content-Type: text/plain\r\n\
Connection: close\r\n\r\nOK";
                let _ = stream.write_all(default_res.as_bytes()).await;
            }
        });
    }
}

// ── Non-activating window helpers ─────────────────────────────────────────

#[cfg(target_os = "macos")]
fn show_turbo_bar(win: &tauri::WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let _ = win.show();
    let _ = win.set_always_on_top(true);

    // orderFrontRegardless must run on the main thread.
    if let Ok(handle) = win.window_handle() {
        if let RawWindowHandle::AppKit(h) = handle.as_raw() {
            let ns_win_ptr = unsafe {
                let ns_view = h.ns_view.as_ptr() as *mut AnyObject;
                let ns_win: *mut AnyObject = msg_send![ns_view, window];
                ns_win as usize
            };
            if ns_win_ptr != 0 {
                let _ = win.app_handle().run_on_main_thread(move || unsafe {
                    let _: () = msg_send![ns_win_ptr as *mut AnyObject, orderFrontRegardless];
                });
            }
        }
    }
}

/// macOS: Subscribe to NSWorkspace app-activation/hide notifications.
/// Show/hide Turbo Bar instantly when PS gains or loses focus.
/// Stops when turbo_active is set to false.
#[cfg(target_os = "macos")]
fn spawn_ps_focus_tracker(win: tauri::WebviewWindow, turbo_active: Arc<AtomicBool>) {
    use block2::RcBlock;
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::unbounded_channel::<bool>();

    // Register notification observer on the main thread (required by NSNotificationCenter).
    let tx_activate = tx.clone();
    let tx_hide = tx.clone();
    let tx_unhide = tx.clone();

    let _ = win.app_handle().run_on_main_thread(move || unsafe {
        let workspace_cls = objc2::runtime::AnyClass::get(c"NSWorkspace")
            .map(|c| c as *const _ as *mut AnyObject);
        let Some(ws_cls) = workspace_cls else { return };
        let workspace: *mut AnyObject = msg_send![ws_cls, sharedWorkspace];
        let nc: *mut AnyObject = msg_send![workspace, notificationCenter];

        // Helper: register one notification with a block that sends `visible` on tx.
        let register = |name: &std::ffi::CStr, tx: mpsc::UnboundedSender<bool>, visible: bool| {
            let block = RcBlock::new(move |_notif: *mut AnyObject| {
                let _ = tx.send(visible);
            });
            let ns_name: *mut AnyObject = msg_send![
                objc2::runtime::AnyClass::get(c"NSString").unwrap() as *const _ as *mut AnyObject,
                stringWithUTF8String: name.as_ptr()
            ];
            let _: *mut AnyObject = msg_send![
                nc,
                addObserverForName: ns_name,
                object: workspace,
                queue: std::ptr::null_mut::<AnyObject>(),
                usingBlock: &*block
            ];
        };

        // NSWorkspaceDidActivateApplicationNotification
        register(c"NSWorkspaceDidActivateApplicationNotification", tx_activate, true);
        // NSWorkspaceDidHideApplicationNotification
        register(c"NSWorkspaceDidHideApplicationNotification", tx_hide, false);
        // NSWorkspaceDidUnhideApplicationNotification
        register(c"NSWorkspaceDidUnhideApplicationNotification", tx_unhide, true);
    });

    // Show immediately if PS is already frontmost when turbo activates.
    if is_ps_or_bar_frontmost() {
        show_turbo_bar(&win);
    }

    tauri::async_runtime::spawn(async move {
        while rx.recv().await.is_some() {
            if !turbo_active.load(Ordering::Relaxed) {
                let _ = win.hide();
                break;
            }
            if is_ps_or_bar_frontmost() {
                show_turbo_bar(&win);
            } else {
                let _ = win.hide();
            }
        }
    });
}


#[cfg(target_os = "macos")]
fn is_ps_or_bar_frontmost() -> bool {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    unsafe {
        let workspace_cls = objc2::runtime::AnyClass::get(c"NSWorkspace")
            .map(|c| c as *const _ as *mut AnyObject);
        let Some(workspace_cls_ptr) = workspace_cls else {
            return false;
        };
        let workspace: *mut AnyObject = msg_send![workspace_cls_ptr, sharedWorkspace];
        if workspace.is_null() {
            return false;
        }
        let app: *mut AnyObject = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return false;
        }
        // Check if the application is hidden (e.g. Cmd+H)
        let is_hidden: bool = msg_send![app, isHidden];
        if is_hidden {
            return false;
        }
        let bundle_id: *mut AnyObject = msg_send![app, bundleIdentifier];
        if bundle_id.is_null() {
            return false;
        }
        let cstr: *const std::ffi::c_char = msg_send![bundle_id, UTF8String];
        if cstr.is_null() {
            return false;
        }
        let s = std::ffi::CStr::from_ptr(cstr).to_str().unwrap_or("");
        // Match Photoshop (com.adobe.Photoshop) or our own bar (com.comfypanel.turbo)
        s.contains("Photoshop") || s.contains("comfypanel")
    }
}

/// macOS: Set NSNonactivatingPanelMask on the window's styleMask so that
/// clicking the Turbo Bar never steals key-window / focus from Photoshop.
#[cfg(target_os = "macos")]
fn apply_non_activating_macos(win: &tauri::WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // NSNonactivatingPanelMask = 1 << 7 (128)
    const NS_NONACTIVATING_PANEL_MASK: usize = 128;
    // NSFloatingWindowLevel = 3 (NSWindowLevel)
    const NS_FLOATING_WINDOW_LEVEL: isize = 3;

    let handle = match win.window_handle() {
        Ok(h) => h,
        Err(_) => return,
    };
    let ns_view = match handle.as_raw() {
        RawWindowHandle::AppKit(h) => h.ns_view.as_ptr() as *mut AnyObject,
        _ => return,
    };

    unsafe {
        let ns_win: *mut AnyObject = msg_send![ns_view, window];
        if !ns_win.is_null() {
            use std::sync::Once;
            static REGISTER_PANEL_CLASS: Once = Once::new();

            extern "C" fn yes_getter(_this: *mut AnyObject, _cmd: *const std::ffi::c_void) -> bool {
                true
            }

            REGISTER_PANEL_CLASS.call_once(|| {
                extern "C" {
                    fn objc_allocateClassPair(superclass: *const AnyClass, name: *const std::ffi::c_char, extraBytes: usize) -> *mut AnyClass;
                    fn objc_registerClassPair(cls: *mut AnyClass);
                    fn class_addMethod(
                        cls: *mut AnyClass,
                        name: *const std::ffi::c_void,
                        imp: extern "C" fn(*mut AnyObject, *const std::ffi::c_void) -> bool,
                        types: *const std::ffi::c_char,
                    ) -> bool;
                    fn sel_registerName(str: *const std::ffi::c_char) -> *const std::ffi::c_void;
                }

                if let Some(panel_class) = AnyClass::get(c"NSPanel") {
                    let new_cls = objc_allocateClassPair(panel_class as *const AnyClass, b"TurboBarPanel\0".as_ptr() as *const _, 0);
                    if !new_cls.is_null() {
                        let sel_key = sel_registerName(b"canBecomeKeyWindow\0".as_ptr() as *const _);
                        let sel_main = sel_registerName(b"canBecomeMainWindow\0".as_ptr() as *const _);
                        let types = b"c@:\0".as_ptr() as *const _;
                        class_addMethod(new_cls, sel_key, yes_getter, types);
                        class_addMethod(new_cls, sel_main, yes_getter, types);
                        objc_registerClassPair(new_cls);
                    }
                }
            });

            // Dynamically transform this NSWindow into TurboBarPanel (subclass of NSPanel)
            if let Some(turbo_panel_class) = AnyClass::get(c"TurboBarPanel") {
                extern "C" {
                    fn object_setClass(obj: *mut AnyObject, cls: *const AnyClass) -> *const AnyClass;
                }
                object_setClass(ns_win, turbo_panel_class);
            }

            // Apply NSNonactivatingPanelMask on the Panel
            let mask: usize = msg_send![ns_win, styleMask];
            let _: () = msg_send![ns_win, setStyleMask: mask | NS_NONACTIVATING_PANEL_MASK];

            // Set floating window level so it hovers above Photoshop canvas
            let _: () = msg_send![ns_win, setLevel: NS_FLOATING_WINDOW_LEVEL];
            // Allow typing when clicking the text input without transferring key app activation
            let _: () = msg_send![ns_win, setBecomesKeyOnlyIfNeeded: true];
            // Do not hide when Photoshop or other app is active — visibility is managed by PS focus tracker
            let _: () = msg_send![ns_win, setHidesOnDeactivate: false];
            let _: () = msg_send![ns_win, setFloatingPanel: true];
            // Accept mouse moved events even when not active so cursor: pointer and hover work
            let _: () = msg_send![ns_win, setAcceptsMouseMovedEvents: true];
            // NSWindowCollectionBehaviorMoveToActiveSpace (2): follows PS when switching Space
            // NSWindowCollectionBehaviorFullScreenAuxiliary (256): visible alongside PS fullscreen
            let _: () = msg_send![ns_win, setCollectionBehavior: 2usize | 256usize];
        }
    }
}

/// Windows: Add WS_EX_NOACTIVATE to extended style so clicking the bar
/// does not transfer activation away from Photoshop.
#[cfg(target_os = "windows")]
fn apply_non_activating_windows(win: &tauri::WebviewWindow) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE,
    };

    let handle = match win.window_handle() {
        Ok(h) => h,
        Err(_) => return,
    };
    let hwnd = match handle.as_raw() {
        RawWindowHandle::Win32(h) => h.hwnd.get() as isize,
        _ => return,
    };

    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE as isize);
    }
}

fn main() {
    // Shared flag: true when Turbo Bar should be tracking and visible with PS
    let turbo_active: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    tauri::Builder::default()
        .setup(move |app| {
            // Force macOS Accessory policy: strictly no Dock icon
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Make Turbo Bar non-activating so it never steals focus from Photoshop
            if let Some(win) = app.get_webview_window("turbo") {
                #[cfg(target_os = "macos")]
                apply_non_activating_macos(&win);

                #[cfg(target_os = "windows")]
                apply_non_activating_windows(&win);
            }

            // Unified single-port server with lifecycle auto-exit
            tauri::async_runtime::spawn(serve_unified(app.handle().clone(), Arc::clone(&turbo_active)));

            let turbo_active_tray = Arc::clone(&turbo_active);

            // Menu bar tray icon — Show/Hide Turbo Bar + Quit
            let toggle = MenuItem::with_id(app, "toggle", "Show / Hide Turbo Bar", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit ComfyPanel Turbo Bar", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "toggle" => {
                            // Tray toggle: force-show the bar regardless of PS focus (one-shot override)
                            if let Some(win) = app.get_webview_window("turbo") {
                                if let Ok(visible) = win.is_visible() {
                                    if visible {
                                        turbo_active_tray.store(false, Ordering::Relaxed);
                                        let _ = win.hide();
                                    } else {
                                        // Re-activate tracking by sending state via WS (UXP will call sync)
                                        let _ = win.show();
                                        let _ = win.set_always_on_top(true);
                                    }
                                }
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // Intercept close button → hide instead of quit
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("run Turbo Bar");
}
