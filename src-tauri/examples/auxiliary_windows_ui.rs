//! Manual native-window acceptance harness. Requires a dedicated temporary directory.
//! Uses the real auxiliary-window manager and stats queries, never the user's database or timer.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use pomotroid_lib::{
    auxiliary_windows, commands,
    db::{self, DbState},
    settings::{self, Settings},
    themes::Theme,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager, State};

#[tauri::command]
fn themes_list() -> Vec<Theme> {
    [
        include_str!("../../static/themes/pomotroid.json"),
        include_str!("../../static/themes/pomotroid-light.json"),
    ]
    .iter()
    .map(|json| serde_json::from_str(json).unwrap())
    .collect()
}

#[tauri::command]
fn settings_set(
    key: String,
    value: String,
    db: State<'_, DbState>,
    app: tauri::AppHandle,
) -> Result<Settings, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    settings::save_setting(&conn, &key, &value).map_err(|e| e.to_string())?;
    let updated = settings::load(&conn).map_err(|e| e.to_string())?;
    app.emit("settings:changed", &updated)
        .map_err(|e| e.to_string())?;
    Ok(updated)
}

#[tauri::command]
fn audio_get_custom_info() -> serde_json::Value {
    serde_json::json!({ "work_alert": null, "short_break_alert": null, "long_break_alert": null })
}
#[tauri::command]
fn check_update() -> Option<String> {
    None
}
#[tauri::command]
fn timer_toggle() {}
#[tauri::command]
fn timer_restart_round() {}
#[tauri::command]
fn timer_skip() {}

fn main() {
    let root = std::env::var_os("POMOTROID_AUX_UI_TEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("pomotroid-aux-ui-validation"));
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("pomotroid-aux-ui-"));
    // This process-local SDK setting also keeps the native WebView cache out of the real profile.
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", root.join("webview"));
    std::fs::create_dir_all(&root).unwrap();
    let conn = rusqlite::Connection::open(root.join("aux-ui-test.sqlite")).unwrap();
    db::migrations::run(&conn).unwrap();
    settings::seed_defaults(&conn).unwrap();
    settings::save_setting(&conn, "language", "zh").unwrap();
    settings::save_setting(&conn, "check_for_updates", "false").unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
        .unwrap();
    if count == 0 {
        for (time, seconds) in [("08:00:00", 1500), ("08:30:00", 3900), ("14:00:00", 900)] {
            conn.execute("INSERT INTO sessions (started_at, ended_at, round_type, duration_secs, completed)
              VALUES (CAST(strftime('%s', date('now','localtime') || ' ' || ?1, 'utc') AS INTEGER),
              CAST(strftime('%s', date('now','localtime') || ' ' || ?1, 'utc') AS INTEGER) + ?2, 'work', ?2, 1)", rusqlite::params![time, seconds]).unwrap();
        }
    }
    let db: DbState = Arc::new(Mutex::new(conn));
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows[0].url = tauri::WebviewUrl::App("/__aux_native_host".into());
    context.config_mut().app.windows[0].title = "Pomotroid — Auxiliary UI Test".into();
    context.config_mut().app.windows[0].visible = true;
    tauri::Builder::default()
        .manage(db)
        .manage(auxiliary_windows::AuxiliaryWindows::default())
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Folder {
                        path: root.join("logs"),
                        file_name: Some("native-ui-test".into()),
                    },
                )])
                .build(),
        )
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Focused(true))
                && ["stats", "settings"].contains(&window.label())
            {
                let _ = window.set_title(&format!("Pomotroid — {} [UI test]", window.label()));
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::aux_window_open,
            commands::aux_window_ready,
            commands::settings_get,
            commands::stats_get_detailed,
            commands::stats_get_heatmap,
            commands::app_version,
            commands::tray_supported,
            commands::accessibility_trusted,
            themes_list,
            settings_set,
            audio_get_custom_info,
            check_update,
            timer_toggle,
            timer_restart_round,
            timer_skip,
        ])
        .setup(|app| {
            let host = app.get_webview_window("main").unwrap();
            let app = app.handle().clone();
            host.on_window_event(move |event| {
                if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                    app.exit(0);
                }
            });
            Ok(())
        })
        .build(context)
        .unwrap()
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
                auxiliary_windows::save_all(app);
            }
        });
}
