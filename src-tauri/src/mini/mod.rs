mod lifecycle;
mod position;
#[cfg(target_os = "windows")]
mod windows;

use crate::{db::DbState, settings, tray::TrayState};
use lifecycle::{CloseReason, Lifecycle, Phase};
use position::{Point, WorkArea, SIZE};
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::Duration;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub struct MiniState {
    gate: tokio::sync::Mutex<()>,
    lifecycle: Mutex<Lifecycle>,
    position_pending: AtomicBool,
    dragging: AtomicBool,
    drag_entered: AtomicBool,
    position_locked: AtomicBool,
    native_ready: AtomicBool,
    drag_done: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl Default for MiniState {
    fn default() -> Self {
        Self {
            gate: tokio::sync::Mutex::new(()),
            lifecycle: Mutex::new(Lifecycle::default()),
            position_pending: AtomicBool::new(false),
            dragging: AtomicBool::new(false),
            drag_entered: AtomicBool::new(false),
            position_locked: AtomicBool::new(false),
            native_ready: AtomicBool::new(false),
            drag_done: Mutex::new(None),
        }
    }
}

#[derive(Clone, Serialize)]
pub struct MiniInfo {
    pub always_on_top: bool,
    pub tray_available: bool,
    pub snap_enabled: bool,
    pub position_locked: bool,
    pub menu_lines: Vec<String>,
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn preferences(conn: &rusqlite::Connection) -> Result<(Option<Point>, bool), String> {
    let read = |key: &str| {
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get::<_, String>(0)
        })
        .optional()
        .map_err(err)
    };
    let position = read("mini_position")?.and_then(|value| serde_json::from_str(&value).ok());
    let top = read("mini_always_on_top")?
        .map(|v| v != "false")
        .unwrap_or(true);
    Ok((position, top))
}

fn tray_available(app: &AppHandle) -> bool {
    let state = app.state::<std::sync::Arc<TrayState>>();
    state.visible.load(Ordering::Acquire)
        && state
            .icon
            .lock()
            .map(|icon| icon.is_some())
            .unwrap_or(false)
}

pub fn is_active(app: &AppHandle) -> bool {
    app.state::<MiniState>()
        .lifecycle
        .lock()
        .map(|state| state.token().is_some())
        .unwrap_or(false)
}

fn current(app: &AppHandle, token: u64) -> Result<(), String> {
    if app
        .state::<MiniState>()
        .lifecycle
        .lock()
        .map_err(err)?
        .is_current(token)
    {
        Ok(())
    } else {
        Err("stale mini window operation".into())
    }
}

pub fn info(app: &AppHandle, token: u64) -> Result<MiniInfo, String> {
    current(app, token)?;
    let db = app.state::<DbState>();
    let conn = db.lock().map_err(err)?;
    let (_, always_on_top) = preferences(&conn)?;
    Ok(MiniInfo {
        always_on_top,
        menu_lines: app
            .state::<std::sync::Arc<TrayState>>()
            .runtime
            .latest
            .lock()
            .unwrap()
            .as_ref()
            .map(|(_, p)| p.mini_lines.clone())
            .unwrap_or_default(),
        tray_available: tray_available(app),
        snap_enabled: settings::get_setting(&conn, "mini_snap_enabled").as_deref() != Some("false"),
        position_locked: settings::get_setting(&conn, "mini_position_locked").as_deref()
            == Some("true"),
    })
}

fn work_area(monitor: &tauri::Monitor) -> WorkArea {
    let area = monitor.work_area();
    WorkArea {
        x: area.position.x,
        y: area.position.y,
        width: area.size.width,
        height: area.size.height,
        scale: monitor.scale_factor(),
    }
}

fn save_position(app: &AppHandle, window: &WebviewWindow) -> Result<(), String> {
    let pos = window.outer_position().map_err(err)?;
    let db = app.state::<DbState>();
    let conn = db.lock().map_err(err)?;
    let value = serde_json::to_string(&Point { x: pos.x, y: pos.y }).map_err(err)?;
    if settings::get_setting(&conn, "mini_position").as_deref() != Some(value.as_str()) {
        settings::save_setting(&conn, "mini_position", &value).map_err(err)?;
    }
    Ok(())
}

fn schedule_position(app: &AppHandle, token: u64) {
    let state = app.state::<MiniState>();
    if state.position_pending.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let state = app.state::<MiniState>();
        let _gate = state.gate.lock().await;
        if current(&app, token).is_ok() && !state.dragging.load(Ordering::Acquire) {
            if let Some(window) = app.get_webview_window("mini") {
                if let Err(error) = settle_window(&window, false) {
                    log::warn!("[mini] visibility repair failed: {error}");
                }
                if let Err(error) = save_position(&app, &window) {
                    log::warn!("[mini] position save failed: {error}");
                }
            }
        }
        state.position_pending.store(false, Ordering::Release);
    });
}

fn settle_window(window: &WebviewWindow, snap: bool) -> Result<(), String> {
    let pos = window.outer_position().map_err(err)?;
    let size = window.outer_size().map_err(err)?;
    let center = (
        i64::from(pos.x) + i64::from(size.width) / 2,
        i64::from(pos.y) + i64::from(size.height) / 2,
    );
    let monitors = window.available_monitors().map_err(err)?;
    let selected = monitors
        .iter()
        .find(|m| {
            let p = m.position();
            let s = m.size();
            center.0 >= i64::from(p.x)
                && center.1 >= i64::from(p.y)
                && center.0 < i64::from(p.x) + i64::from(s.width)
                && center.1 < i64::from(p.y) + i64::from(s.height)
        })
        .cloned()
        .or(window.current_monitor().map_err(err)?)
        .or(window.primary_monitor().map_err(err)?)
        .ok_or("no available monitor")?;
    let point = position::settle(
        work_area(&selected),
        Point { x: pos.x, y: pos.y },
        size.width,
        size.height,
        snap,
    );
    if point.x != pos.x || point.y != pos.y {
        window
            .set_position(PhysicalPosition::new(point.x, point.y))
            .map_err(err)?;
        // A native dispatcher barrier: programmatic Moved events are processed
        // before the drag flag is released and before final persistence.
        let _ = window.outer_position().map_err(err)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn drag_released(app: &AppHandle) {
    if let Some(done) = app.state::<MiniState>().drag_done.lock().unwrap().take() {
        let _ = done.send(());
    }
}

pub async fn drag(app: AppHandle, token: u64) -> Result<(), String> {
    let state = app.state::<MiniState>();
    let gate = state.gate.lock().await;
    current(&app, token)?;
    if info(&app, token)?.position_locked {
        return Ok(());
    }
    let window = app
        .get_webview_window("mini")
        .ok_or("mini window not found")?;
    #[cfg(not(target_os = "windows"))]
    {
        drop(gate);
        return window.start_dragging().map_err(err);
    }
    #[cfg(target_os = "windows")]
    {
        if !state.native_ready.load(Ordering::Acquire) {
            return Err("native drag boundary unavailable".into());
        }
        if state.dragging.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let original = match window.outer_position() {
            Ok(value) => value,
            Err(error) => {
                state.dragging.store(false, Ordering::Release);
                return Err(err(error));
            }
        };
        let (done, wait) = tokio::sync::oneshot::channel();
        *state.drag_done.lock().map_err(err)? = Some(done);
        state.drag_entered.store(false, Ordering::Release);
        if let Err(error) = window.start_dragging() {
            state.dragging.store(false, Ordering::Release);
            state.drag_done.lock().unwrap().take();
            return Err(err(error));
        }
        drop(gate);
        let result = wait.await.map_err(err);
        let _gate = state.gate.lock().await;
        let result = result.and_then(|()| {
            current(&app, token)?;
            let landed = window.outer_position().map_err(err)?;
            let snap = info(&app, token)?.snap_enabled
                && state.drag_entered.load(Ordering::Acquire)
                && landed != original;
            settle_window(&window, snap)
        });
        state.dragging.store(false, Ordering::Release);
        if result.is_ok() {
            schedule_position(&app, token);
        }
        result
    }
}

pub async fn set_behavior(
    app: AppHandle,
    token: u64,
    key: String,
    value: bool,
) -> Result<MiniInfo, String> {
    let state = app.state::<MiniState>();
    let _gate = state.gate.lock().await;
    current(&app, token)?;
    let stored = match key.as_str() {
        "snap" => "mini_snap_enabled",
        "locked" => "mini_position_locked",
        _ => return Err("invalid mini preference".into()),
    };
    {
        let db = app.state::<DbState>();
        settings::save_setting(
            &*db.lock().map_err(err)?,
            stored,
            if value { "true" } else { "false" },
        )
        .map_err(err)?;
    }
    if key == "locked" {
        state.position_locked.store(value, Ordering::Release);
    }
    let value = info(&app, token)?;
    app.emit_to("mini", "mini:preferences", &value)
        .map_err(err)?;
    Ok(value)
}

async fn close_inner(app: &AppHandle, reason: CloseReason) -> Result<(), String> {
    let state = app.state::<MiniState>();
    if reason == CloseReason::Tray && !tray_available(app) {
        return Err("no visible tray recovery entry".into());
    }
    let Some((previous, token)) = state.lifecycle.lock().map_err(err)?.begin_close(reason) else {
        return Ok(());
    };
    let main = app
        .get_webview_window("main")
        .ok_or("main window not found")?;
    let visibility = if reason == CloseReason::Restore {
        main.show().and_then(|()| {
            if main.is_minimized()? {
                main.unminimize()?;
            }
            Ok(())
        })
    } else {
        main.hide()
    };
    if let Err(error) = visibility {
        state
            .lifecycle
            .lock()
            .map_err(err)?
            .abort_close(previous, token, reason);
        return Err(err(error));
    }
    if reason == CloseReason::Restore {
        if let Err(error) = main.set_focus() {
            log::warn!("[mini] main focus request failed: {error}");
        }
    }
    if let Some(window) = app.get_webview_window("mini") {
        if let Err(error) = save_position(app, &window) {
            log::warn!("[mini] final position save failed: {error}");
        }
        // destroy bypasses CloseRequested; the explicit reason also guards late Destroyed events.
        if reason == CloseReason::Restore {
            window.hide().map_err(err)?;
        }
        window.destroy().map_err(err)?;
        tokio::time::timeout(Duration::from_secs(3), async {
            while app.get_webview_window("mini").is_some() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(|_| "mini window close timed out")?;
    }
    // The tray may have been disabled while this asynchronous close was pending.
    if state.lifecycle.lock().map_err(err)?.phase == Phase::Exiting {
        return Ok(());
    }
    if reason == CloseReason::Tray && !tray_available(app) {
        main.show().map_err(err)?;
        if main.is_minimized().map_err(err)? {
            main.unminimize().map_err(err)?;
        }
        if let Err(error) = main.set_focus() {
            log::warn!("[mini] main focus request failed: {error}");
        }
        let mut lifecycle = state.lifecycle.lock().map_err(err)?;
        if lifecycle.phase != Phase::Exiting {
            lifecycle.phase = Phase::Main;
        }
        return Ok(());
    }
    let mut lifecycle = state.lifecycle.lock().map_err(err)?;
    if let Some(id) = token {
        lifecycle.finish_close(id);
    } else if reason == CloseReason::Tray && lifecycle.phase != Phase::Exiting {
        lifecycle.phase = Phase::Hidden;
    }
    Ok(())
}

pub async fn restore(app: AppHandle) -> Result<(), String> {
    let state = app.state::<MiniState>();
    let _gate = state.gate.lock().await;
    close_inner(&app, CloseReason::Restore).await
}

pub fn request_restore(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = restore(app.clone()).await {
            report(&app, &error);
        }
    });
}

pub fn recover_if_hidden(app: &AppHandle) {
    let hidden = app
        .state::<MiniState>()
        .lifecycle
        .lock()
        .map(|state| state.needs_tray_recovery())
        .unwrap_or(false);
    if hidden {
        request_restore(app);
    }
}

fn request_restore_for_token(app: &AppHandle, token: u64) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Validate after acquiring the gate; an old native callback must not close a new mini.
        let _ = restore_from_mini(app, token).await;
    });
}

fn report(app: &AppHandle, message: &str) {
    log::error!("[mini] {message}");
    let _ = app.emit_to("main", "mini:error", ());
}

pub async fn failed(app: AppHandle, token: u64, message: String) -> Result<(), String> {
    let state = app.state::<MiniState>();
    let _gate = state.gate.lock().await;
    if !state.lifecycle.lock().map_err(err)?.accepts_ready(token) {
        return Ok(());
    }
    let result = close_inner(&app, CloseReason::Restore).await;
    report(&app, &message);
    result
}

pub async fn open(app: AppHandle) -> Result<(), String> {
    let state = app.state::<MiniState>();
    let _gate = state.gate.lock().await;
    if matches!(
        state.lifecycle.lock().map_err(err)?.phase,
        Phase::Closing(_, _) | Phase::Exiting
    ) {
        return Err("mini window is closing or application is exiting".into());
    }
    let Some(token) = state.lifecycle.lock().map_err(err)?.begin() else {
        return Ok(());
    };
    state.native_ready.store(false, Ordering::Release);
    let result = (|| -> Result<(), String> {
        let main = app
            .get_webview_window("main")
            .ok_or("main window not found")?;
        let (saved, top) = {
            let db = app.state::<DbState>();
            let conn = db.lock().map_err(err)?;
            state.position_locked.store(
                settings::get_setting(&conn, "mini_position_locked").as_deref() == Some("true"),
                Ordering::Release,
            );
            preferences(&conn)?
        };
        let monitors = main.available_monitors().map_err(err)?;
        let monitor = main
            .current_monitor()
            .map_err(err)?
            .or(main.primary_monitor().map_err(err)?)
            .ok_or("no available monitor")?;
        let area = work_area(&monitor);
        let origin = main.outer_position().map_err(err)?;
        let size = main.outer_size().map_err(err)?;
        let initial = Point {
            x: (f64::from(origin.x) + f64::from(size.width) - (SIZE + 12.0) * area.scale).round()
                as i32,
            y: (f64::from(origin.y) + 12.0 * area.scale).round() as i32,
        };
        let point = position::place(
            &monitors.iter().map(work_area).collect::<Vec<_>>(),
            saved,
            area,
            initial,
        );
        // Runs in an async command, not a synchronous Windows event handler (WebView2 requirement).
        let window = WebviewWindowBuilder::new(
            &app,
            "mini",
            WebviewUrl::App(format!("/mini?instance={token}").into()),
        )
        .title("Pomotroid")
        .inner_size(SIZE, SIZE)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(false)
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(top)
        .visible(false)
        .focused(false)
        .build()
        .map_err(err)?;
        window
            .set_position(PhysicalPosition::new(point.x, point.y))
            .map_err(err)?;
        window.set_size(LogicalSize::new(SIZE, SIZE)).map_err(err)?;
        #[cfg(target_os = "windows")]
        windows::install(&window, token)?;
        let app_event = app.clone();
        let window_event = window.clone();
        window.on_window_event(move |event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                if current(&app_event, token).is_ok() {
                    api.prevent_close();
                    request_restore_for_token(&app_event, token);
                }
            }
            tauri::WindowEvent::Destroyed => {
                let restore = app_event
                    .state::<MiniState>()
                    .lifecycle
                    .lock()
                    .map(|mut lifecycle| {
                        let restore = lifecycle.is_current(token);
                        lifecycle.finish_close(token);
                        restore
                    })
                    .unwrap_or(false);
                if restore {
                    request_restore_for_token(&app_event, token);
                }
            }
            tauri::WindowEvent::Moved(_) => {
                if !app_event
                    .state::<MiniState>()
                    .dragging
                    .load(Ordering::Acquire)
                {
                    schedule_position(&app_event, token);
                }
            }
            tauri::WindowEvent::ScaleFactorChanged { .. } => {
                let _ = window_event.set_size(LogicalSize::new(SIZE, SIZE));
                if app_event
                    .state::<MiniState>()
                    .dragging
                    .load(Ordering::Acquire)
                {
                    return;
                }
                if let (Ok(Some(monitor)), Ok(pos)) = (
                    window_event.current_monitor(),
                    window_event.outer_position(),
                ) {
                    let point = work_area(&monitor).clamp(Point { x: pos.x, y: pos.y });
                    let _ = window_event.set_position(PhysicalPosition::new(point.x, point.y));
                }
                schedule_position(&app_event, token);
            }
            _ => {}
        });
        Ok(())
    })();
    if let Err(error) = result {
        let _ = close_inner(&app, CloseReason::Restore).await;
        report(&app, &error);
        return Err(error);
    }
    let timeout_app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(15)).await;
        let _ = failed(timeout_app, token, "mini initialization timed out".into()).await;
    });
    Ok(())
}

pub async fn ready(app: AppHandle, token: u64) -> Result<(), String> {
    let state = app.state::<MiniState>();
    let _gate = state.gate.lock().await;
    // A reload of the same WebView may acknowledge initialization again; do not refocus it.
    if state.lifecycle.lock().map_err(err)?.phase == Phase::Visible(token) {
        return Ok(());
    }
    if !state.lifecycle.lock().map_err(err)?.accepts_ready(token) {
        return Err("stale mini ready signal".into());
    }
    let result = (|| -> Result<(), String> {
        let window = app
            .get_webview_window("mini")
            .ok_or("mini window not found")?;
        let main = app
            .get_webview_window("main")
            .ok_or("main window not found")?;
        window.show().map_err(err)?;
        main.hide().map_err(err)?;
        window.set_focus().map_err(err)?;
        let mut lifecycle = state.lifecycle.lock().map_err(err)?;
        if !lifecycle.accepts_ready(token) {
            return Err("mini operation cancelled".into());
        }
        lifecycle.phase = Phase::Visible(token);
        Ok(())
    })();
    if let Err(error) = &result {
        let _ = close_inner(&app, CloseReason::Restore).await;
        report(&app, error);
    }
    result
}

pub async fn restore_from_mini(app: AppHandle, token: u64) -> Result<(), String> {
    let state = app.state::<MiniState>();
    let _gate = state.gate.lock().await;
    current(&app, token)?;
    close_inner(&app, CloseReason::Restore).await
}

pub async fn hide_to_tray(app: AppHandle, token: u64) -> Result<(), String> {
    let state = app.state::<MiniState>();
    let _gate = state.gate.lock().await;
    current(&app, token)?;
    close_inner(&app, CloseReason::Tray).await
}

pub async fn set_top(app: AppHandle, token: u64, value: bool) -> Result<(), String> {
    let state = app.state::<MiniState>();
    let _gate = state.gate.lock().await;
    current(&app, token)?;
    let window = app
        .get_webview_window("mini")
        .ok_or("mini window not found")?;
    let db = app.state::<DbState>();
    let conn = db.lock().map_err(err)?;
    let (_, previous) = preferences(&conn)?;
    window.set_always_on_top(value).map_err(err)?;
    if let Err(error) = settings::save_setting(
        &conn,
        "mini_always_on_top",
        if value { "true" } else { "false" },
    ) {
        let _ = window.set_always_on_top(previous);
        return Err(err(error));
    }
    Ok(())
}

pub fn begin_exit(app: &AppHandle) {
    if let Some(state) = app.try_state::<MiniState>() {
        if let Ok(mut lifecycle) = state.lifecycle.lock() {
            if let Some(token) = lifecycle.token() {
                lifecycle.phase = Phase::Closing(token, CloseReason::Exit);
                lifecycle.finish_close(token);
            } else {
                lifecycle.phase = Phase::Exiting;
            }
        }
        if let Some(window) = app.get_webview_window("mini") {
            if let Err(error) = save_position(app, &window) {
                log::warn!("[mini] exit position save failed: {error}");
            }
        }
    }
}

pub fn exit(app: &AppHandle) {
    begin_exit(app);
    app.exit(0);
}

pub fn exit_from_mini(app: &AppHandle, token: u64) -> Result<(), String> {
    current(app, token)?;
    exit(app);
    Ok(())
}

pub async fn persist_position(app: AppHandle, token: u64) -> Result<(), String> {
    let state = app.state::<MiniState>();
    let _gate = state.gate.lock().await;
    current(&app, token)?;
    if let Some(window) = app.get_webview_window("mini") {
        save_position(&app, &window)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mini_preferences_preserve_main_settings_and_sessions() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run(&conn).unwrap();
        settings::save_setting(&conn, "always_on_top", "false").unwrap();
        settings::save_setting(&conn, "window_x", "42").unwrap();
        assert_eq!(preferences(&conn).unwrap(), (None, true));
        assert_ne!(
            settings::get_setting(&conn, "mini_snap_enabled").as_deref(),
            Some("false")
        );
        assert_ne!(
            settings::get_setting(&conn, "mini_position_locked").as_deref(),
            Some("true")
        );
        settings::save_setting(&conn, "mini_snap_enabled", "false").unwrap();
        settings::save_setting(&conn, "mini_position_locked", "true").unwrap();
        assert_eq!(
            settings::get_setting(&conn, "mini_snap_enabled").as_deref(),
            Some("false")
        );
        assert_eq!(
            settings::get_setting(&conn, "mini_position_locked").as_deref(),
            Some("true")
        );
        settings::save_setting(&conn, "mini_position", r#"{"x":120,"y":240}"#).unwrap();
        settings::save_setting(&conn, "mini_always_on_top", "false").unwrap();
        assert_eq!(
            preferences(&conn).unwrap(),
            (Some(Point { x: 120, y: 240 }), false)
        );
        assert_eq!(
            settings::get_setting(&conn, "window_x").as_deref(),
            Some("42")
        );
        assert_eq!(
            settings::get_setting(&conn, "always_on_top").as_deref(),
            Some("false")
        );
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
