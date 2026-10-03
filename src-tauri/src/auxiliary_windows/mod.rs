mod geometry;

use crate::{db::DbState, settings};
use geometry::{Area, Geometry, Spec};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{
    AppHandle, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Settings,
    Stats,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Stats => "stats",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Settings => "settings_window_geometry",
            Self::Stats => "stats_window_geometry",
        }
    }
    fn spec(self) -> Spec {
        match self {
            Self::Settings => Spec {
                width: 780.0,
                height: 620.0,
                min_width: 640.0,
                min_height: 480.0,
            },
            Self::Stats => Spec {
                width: 900.0,
                height: 600.0,
                min_width: 720.0,
                min_height: 480.0,
            },
        }
    }
    fn from_label(label: &str) -> Result<Self, String> {
        match label {
            "settings" => Ok(Self::Settings),
            "stats" => Ok(Self::Stats),
            _ => Err("auxiliary window required".into()),
        }
    }
}

struct Session {
    token: u64,
    ready: bool,
    normal: Option<Geometry>,
    pending: Option<tauri::async_runtime::JoinHandle<()>>,
}

#[derive(Default)]
pub struct AuxiliaryWindows {
    gate: tokio::sync::Mutex<()>,
    next: AtomicU64,
    sessions: Mutex<HashMap<Kind, Session>>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn area(monitor: &tauri::Monitor) -> Area {
    let work = monitor.work_area();
    Area {
        name: monitor.name().cloned(),
        x: work.position.x,
        y: work.position.y,
        width: work.size.width,
        height: work.size.height,
        scale: monitor.scale_factor(),
    }
}
fn normal_geometry(window: &WebviewWindow) -> Result<Option<Geometry>, String> {
    if window.is_minimized().map_err(err)?
        || window.is_maximized().map_err(err)?
        || window.is_fullscreen().map_err(err)?
    {
        return Ok(None);
    }
    let size = window.inner_size().map_err(err)?;
    if size.width == 0 || size.height == 0 {
        return Ok(None);
    }
    let scale = window.scale_factor().map_err(err)?;
    let monitor = window
        .current_monitor()
        .map_err(err)?
        .ok_or("monitor unavailable")?;
    let pos = window.outer_position().map_err(err)?;
    Ok(Some(Geometry {
        x: pos.x,
        y: pos.y,
        width: f64::from(size.width) / scale,
        height: f64::from(size.height) / scale,
        monitor: monitor.name().cloned(),
        monitor_x: monitor.work_area().position.x,
        monitor_y: monitor.work_area().position.y,
    }))
}
fn frame(window: &WebviewWindow) -> Result<(f64, f64), String> {
    let outer = window.outer_size().map_err(err)?;
    let inner = window.inner_size().map_err(err)?;
    let scale = window.scale_factor().map_err(err)?;
    Ok((
        f64::from(outer.width.saturating_sub(inner.width)) / scale,
        f64::from(outer.height.saturating_sub(inner.height)) / scale,
    ))
}
fn preferred(app: &AppHandle) -> Result<Area, String> {
    let main = app.get_webview_window("main");
    if main
        .as_ref()
        .is_some_and(|w| !w.is_visible().unwrap_or(false))
    {
        if let Some(mini) = app
            .get_webview_window("mini")
            .filter(|w| w.is_visible().unwrap_or(false))
        {
            if let Ok(Some(monitor)) = mini.current_monitor() {
                return Ok(area(&monitor));
            }
        }
    }
    if let Some(main) = main {
        if let Ok(Some(monitor)) = main.current_monitor() {
            return Ok(area(&monitor));
        }
    }
    app.primary_monitor()
        .map_err(err)?
        .map(|m| area(&m))
        .ok_or("no monitor available".into())
}
fn load(app: &AppHandle, kind: Kind) -> Result<Option<Geometry>, String> {
    let db = app.state::<DbState>();
    let conn = db.lock().map_err(err)?;
    let json: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [kind.key()],
            |row| row.get(0),
        )
        .optional()
        .map_err(err)?;
    json.map(|json| serde_json::from_str(&json).map_err(err))
        .transpose()
}
fn save(app: &AppHandle, kind: Kind, geometry: &Geometry) {
    let result = (|| -> Result<(), String> {
        let db = app.state::<DbState>();
        let conn = db.lock().map_err(err)?;
        settings::save_setting(
            &conn,
            kind.key(),
            &serde_json::to_string(geometry).map_err(err)?,
        )
        .map_err(err)
    })();
    if let Err(error) = result {
        log::warn!("[{}] geometry save failed: {error}", kind.label());
    }
}
fn position(
    app: &AppHandle,
    window: &WebviewWindow,
    kind: Kind,
    saved: Option<&Geometry>,
) -> Result<(), String> {
    let areas: Vec<_> = app
        .available_monitors()
        .map_err(err)?
        .iter()
        .map(area)
        .collect();
    let plan = geometry::restore(kind.spec(), saved, &areas, &preferred(app)?, frame(window)?);
    window
        .set_min_size(Some(LogicalSize::new(plan.min_width, plan.min_height)))
        .map_err(err)?;
    window
        .set_position(PhysicalPosition::new(plan.x, plan.y))
        .map_err(err)?;
    window
        .set_size(LogicalSize::new(plan.width, plan.height))
        .map_err(err)
}

fn observe(app: &AppHandle, window: &WebviewWindow, kind: Kind, token: u64, closing: bool) {
    let manager = app.state::<AuxiliaryWindows>();
    let measured = normal_geometry(window).ok().flatten();
    let mut sessions = match manager.sessions.lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    let Some(session) = sessions
        .get_mut(&kind)
        .filter(|s| s.token == token && s.ready)
    else {
        return;
    };
    let changed = measured
        .as_ref()
        .is_some_and(|g| session.normal.as_ref() != Some(g));
    if let Some(geometry) = measured {
        session.normal = Some(geometry);
    }
    if !closing && !changed {
        return;
    }
    if let Some(pending) = session.pending.take() {
        pending.abort();
    }
    if closing {
        let geometry = session.normal.clone();
        session.ready = false;
        drop(sessions);
        if let Some(geometry) = geometry {
            save(app, kind, &geometry);
        }
        return;
    }
    let app = app.clone();
    session.pending = Some(tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let manager = app.state::<AuxiliaryWindows>();
        // Serialize the write with close/new observations so an old debounce cannot
        // overwrite a newer final geometry after the session lock is released.
        if let Ok(sessions) = manager.sessions.lock() {
            if let Some(geometry) = sessions
                .get(&kind)
                .filter(|s| s.token == token && s.ready)
                .and_then(|s| s.normal.as_ref())
            {
                save(&app, kind, geometry);
            }
        };
    }));
}

pub async fn open(app: AppHandle, kind: Kind) -> Result<(), String> {
    let manager = app.state::<AuxiliaryWindows>();
    let _gate = manager.gate.lock().await;
    if let Some(window) = app.get_webview_window(kind.label()) {
        if !manager
            .sessions
            .lock()
            .map_err(err)?
            .get(&kind)
            .is_some_and(|s| s.ready)
        {
            return Ok(());
        }
        if window.is_minimized().map_err(err)? {
            window.unminimize().map_err(err)?;
        }
        if let Some(geometry) = normal_geometry(&window)? {
            let areas: Vec<_> = app
                .available_monitors()
                .map_err(err)?
                .iter()
                .map(area)
                .collect();
            let frame = frame(&window)?;
            if !areas.iter().any(|a| geometry::fits(&geometry, a, frame)) {
                // Off-screen reuse follows the same event suppression as first open.
                if let Some(session) = manager.sessions.lock().map_err(err)?.get_mut(&kind) {
                    session.ready = false;
                    if let Some(pending) = session.pending.take() {
                        pending.abort();
                    }
                }
                let restored = position(&app, &window, kind, Some(&geometry))
                    .and_then(|()| normal_geometry(&window));
                if let Some(session) = manager.sessions.lock().map_err(err)?.get_mut(&kind) {
                    if let Ok(Some(normal)) = &restored {
                        session.normal = Some(normal.clone());
                    }
                    session.ready = true;
                }
                restored?;
            }
        }
        window.show().map_err(err)?;
        return window.set_focus().map_err(err);
    }
    let token = manager.next.fetch_add(1, Ordering::Relaxed) + 1;
    manager.sessions.lock().map_err(err)?.insert(
        kind,
        Session {
            token,
            ready: false,
            normal: None,
            pending: None,
        },
    );
    let spec = kind.spec();
    let builder = WebviewWindowBuilder::new(
        &app,
        kind.label(),
        WebviewUrl::App(format!("/{}?aux={token}", kind.label()).into()),
    )
    .title(match kind {
        Kind::Settings => "Pomotroid — Settings",
        Kind::Stats => "Pomotroid — Statistics",
    })
    .inner_size(spec.width, spec.height)
    .resizable(true)
    .maximizable(true)
    .visible(false)
    .focused(false)
    .decorations(cfg!(target_os = "macos"));
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    let window = match builder.build() {
        Ok(window) => window,
        Err(error) => {
            manager.sessions.lock().map_err(err)?.remove(&kind);
            return Err(err(error));
        }
    };
    let event_app = app.clone();
    let event_window = window.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) => {
            observe(&event_app, &event_window, kind, token, false)
        }
        tauri::WindowEvent::CloseRequested { .. } => {
            observe(&event_app, &event_window, kind, token, true)
        }
        tauri::WindowEvent::Destroyed => {
            if let Ok(mut sessions) = event_app.state::<AuxiliaryWindows>().sessions.lock() {
                if sessions.get(&kind).is_some_and(|s| s.token == token) {
                    if let Some(session) = sessions.remove(&kind) {
                        if let Some(pending) = session.pending {
                            pending.abort();
                        }
                    }
                }
            }
        }
        _ => {}
    });
    Ok(())
}

pub async fn ready(app: AppHandle, window: WebviewWindow, token: u64) -> Result<(), String> {
    let kind = Kind::from_label(window.label())?;
    let manager = app.state::<AuxiliaryWindows>();
    let _gate = manager.gate.lock().await;
    {
        let sessions = manager.sessions.lock().map_err(err)?;
        let session = sessions
            .get(&kind)
            .filter(|s| s.token == token)
            .ok_or("stale auxiliary window")?;
        if session.ready {
            return Ok(());
        }
    }
    let saved = load(&app, kind).unwrap_or_else(|error| {
        log::warn!(
            "[{}] geometry load failed; centering: {error}",
            kind.label()
        );
        None
    });
    position(&app, &window, kind, saved.as_ref())?;
    // Querying after placement acts as a native dispatcher barrier; restoration events stay ignored.
    let normal = normal_geometry(&window)?;
    window.show().map_err(err)?;
    window.set_focus().map_err(err)?;
    if let Some(session) = manager
        .sessions
        .lock()
        .map_err(err)?
        .get_mut(&kind)
        .filter(|s| s.token == token)
    {
        session.normal = normal;
        session.ready = true;
    }
    Ok(())
}

pub fn save_all(app: &AppHandle) {
    let Some(manager) = app.try_state::<AuxiliaryWindows>() else {
        return;
    };
    let windows = manager
        .sessions
        .lock()
        .map(|sessions| {
            sessions
                .iter()
                .map(|(kind, s)| (*kind, s.token))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for (kind, token) in windows {
        if let Some(window) = app.get_webview_window(kind.label()) {
            observe(app, &window, kind, token, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geometry_keys_never_overlap_main_or_mini_and_round_trip_units() {
        assert_ne!(Kind::Settings.key(), Kind::Stats.key());
        for key in [Kind::Settings.key(), Kind::Stats.key()] {
            assert!(![
                "window_x",
                "window_y",
                "window_width",
                "window_height",
                "mini_position"
            ]
            .contains(&key));
        }
        let geometry = Geometry {
            x: -1200,
            y: 20,
            width: 780.0,
            height: 620.0,
            monitor: Some("left".into()),
            monitor_x: -1920,
            monitor_y: 0,
        };
        let json = serde_json::to_string(&geometry).unwrap();
        assert_eq!(serde_json::from_str::<Geometry>(&json).unwrap(), geometry);
        assert!(Kind::from_label("main").is_err());
        assert!(Kind::from_label("mini").is_err());
    }
}
