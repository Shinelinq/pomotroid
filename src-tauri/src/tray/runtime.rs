use super::{
    digits,
    presentation::{self, Presentation, TrayView},
    TrayColors, TrayState, SIZE,
};
use crate::{db::DbState, themes};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{image::Image, AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct Runtime {
    pub latest: Mutex<Option<(TrayView, Presentation)>>,
    queued: AtomicBool,
    applied: Mutex<Applied>,
}
#[derive(Default)]
struct Applied {
    tooltip: String,
    icon: Option<IconKey>,
    menu: String,
    theme: String,
}
#[derive(Clone, PartialEq)]
struct IconKey {
    mode: String,
    label: String,
    round: String,
    paused: bool,
    running: bool,
    progress: u32,
    countdown: bool,
    colors: TrayColors,
    size: u32,
}

pub fn present(app: &AppHandle, state: &Arc<TrayState>, view: TrayView) {
    let built = {
        let db = app.state::<DbState>();
        let conn = db.lock().unwrap();
        presentation::build(&view, &conn)
    };
    match built {
        Ok(presentation) => {
            let mut latest = state.runtime.latest.lock().unwrap();
            if latest
                .as_ref()
                .is_none_or(|(_, old)| old.mini_lines != presentation.mini_lines)
            {
                let _ = app.emit_to("mini", "mini:status", &presentation.mini_lines);
            }
            *latest = Some((view, presentation));
            drop(latest);
            schedule(app, state);
        }
        Err(error) => log::warn!("[tray] presentation failed: {error}"),
    }
}

pub fn refresh(app: &AppHandle) {
    if let Some(state) = app.try_state::<Arc<TrayState>>() {
        *state.runtime.applied.lock().unwrap() = Applied::default();
        schedule(app, &state);
    }
}
fn schedule(app: &AppHandle, state: &Arc<TrayState>) {
    if state.runtime.queued.swap(true, Ordering::AcqRel) {
        return;
    }
    let app_copy = app.clone();
    let state_copy = Arc::clone(state);
    if let Err(error) = app.run_on_main_thread(move || {
        // Clear before reading latest: a concurrent update queues another flush,
        // so a boundary/idle update cannot be lost even when ticks cease.
        state_copy.runtime.queued.store(false, Ordering::Release);
        flush(&app_copy, &state_copy);
    }) {
        state.runtime.queued.store(false, Ordering::Release);
        log::warn!("[tray] dispatch failed: {error}");
    }
}
fn flush(app: &AppHandle, state: &Arc<TrayState>) {
    let Some((view, text)) = state.runtime.latest.lock().unwrap().clone() else {
        return;
    };
    let dark =
        app.get_webview_window("main").and_then(|w| w.theme().ok()) == Some(tauri::Theme::Dark);
    let settings = &view.settings;
    let name = if settings.theme_mode == "dark" || settings.theme_mode == "auto" && dark {
        &settings.theme_dark
    } else {
        &settings.theme_light
    };
    let mut applied = state.runtime.applied.lock().unwrap();
    if applied.theme != *name {
        if let Ok(dir) = app.path().app_data_dir() {
            if let Some(theme) = themes::find(&dir, name) {
                *state.colors.lock().unwrap() = TrayColors::from_colors_map(&theme.colors);
            }
        }
        applied.theme = name.clone();
    }
    *state.countdown_mode.lock().unwrap() = settings.dial_countdown;
    let Some(tray) = state.icon.lock().unwrap().clone() else {
        return;
    };
    if !state.visible.load(Ordering::Acquire) {
        return;
    }
    if applied.tooltip != text.tooltip {
        match tray.set_tooltip(Some(&text.tooltip)) {
            Ok(()) => applied.tooltip = text.tooltip.clone(),
            Err(error) => log::warn!("[tray] tooltip failed: {error}"),
        }
    }
    let timer = &view.timer;
    let menu_key = format!("{}:{}:{}", text.locale, timer.is_running, timer.has_started);
    if applied.menu != menu_key {
        if let Some(items) = state.menu_items.lock().unwrap().clone() {
            let label = |key| presentation::text(&text.locale, key, &[]);
            let _ = items.toggle.set_text(label(if timer.is_running {
                "mini_pause"
            } else if timer.is_paused {
                "mini_resume"
            } else {
                "mini_start"
            }));
            let _ = items.skip.set_text(label("mini_skip"));
            let _ = items.reset_round.set_text(label("mini_restart"));
            let _ = items.show.set_text(label("mini_restore"));
            let _ = items.exit.set_text(label("mini_exit"));
            let _ = items.skip.set_enabled(timer.has_started);
            let _ = items.reset_round.set_enabled(timer.has_started);
            applied.menu = menu_key;
        }
    }
    let numeric = settings.tray_display_mode == "minutes";
    let size = if numeric {
        tray.rect()
            .ok()
            .flatten()
            .map(|r| r.size.to_physical::<u32>(1.0).height.clamp(16, 64))
            .unwrap_or(SIZE)
    } else {
        SIZE
    };
    let key = icon_key(&view, state.colors.lock().unwrap().clone(), size);
    if applied.icon.as_ref() == Some(&key) {
        return;
    }
    let rendered = if numeric {
        digits::render(&key.colors, &key.label, key.paused, &key.round, size)
    } else {
        Ok(super::render_tray_icon_rgba(
            &key.colors,
            key.paused,
            timer.elapsed_secs as f32 / timer.total_secs.max(1) as f32,
            &key.round,
            key.countdown,
        ))
    };
    match rendered.and_then(|bytes| {
        tray.set_icon(Some(Image::new_owned(bytes, size, size)))
            .map_err(|e| e.to_string())
    }) {
        Ok(()) => applied.icon = Some(key),
        Err(error) => log::warn!("[tray] render/update failed; retaining previous icon: {error}"),
    }
}

fn icon_key(view: &TrayView, colors: TrayColors, size: u32) -> IconKey {
    let settings = &view.settings;
    let timer = &view.timer;
    let numeric = settings.tray_display_mode == "minutes";
    IconKey {
        mode: settings.tray_display_mode.clone(),
        label: if numeric {
            presentation::minute_label(timer)
        } else {
            String::new()
        },
        round: timer.round_type.clone(),
        paused: timer.is_paused,
        running: timer.is_running,
        progress: if numeric {
            0
        } else {
            timer.elapsed_secs.saturating_mul(100) / timer.total_secs.max(1)
        },
        countdown: settings.dial_countdown,
        colors,
        size,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_icon_ignores_ticks_until_minute_changes_but_tracks_state_theme_and_dpi() {
        let mut view = TrayView {
            timer: crate::timer::TimerSnapshot {
                total_secs: 120,
                is_running: true,
                round_type: "work".into(),
                ..Default::default()
            },
            settings: crate::settings::Settings {
                tray_display_mode: "minutes".into(),
                ..Default::default()
            },
            category_name: None,
            next_category_name: None,
            pending_plan: None,
        };
        let key = icon_key(&view, TrayColors::default(), 20);
        view.timer.elapsed_secs = 59;
        view.timer.captured_at_ms = 20000;
        assert!(key == icon_key(&view, TrayColors::default(), 20));
        view.timer.elapsed_secs = 60;
        assert!(key != icon_key(&view, TrayColors::default(), 20));
        view.timer.elapsed_secs = 0;
        view.timer.is_paused = true;
        assert!(key != icon_key(&view, TrayColors::default(), 20));
        view.timer.is_paused = false;
        assert!(key != icon_key(&view, TrayColors::default(), 24));
        let colors = TrayColors {
            focus_round: [1, 2, 3, 255],
            ..TrayColors::default()
        };
        assert!(key != icon_key(&view, colors, 20));
        view.settings.tray_display_mode = "progress".into();
        assert!(key != icon_key(&view, TrayColors::default(), 20));
    }
}
