use super::{
    err, files,
    package::{self, ImportPlan, Options, Package, Summary},
    report::{self, Report, ReportInfo, ReportScope},
    Result,
};
use crate::{db::DbState, settings, timer::TimerController};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    io::Read,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

const TTL: Duration = Duration::from_secs(20 * 60);
struct Preview {
    package: Arc<Package>,
    duplicates: usize,
    options: Options,
    plan: ImportPlan,
    locale: String,
    digest: String,
}
enum Payload {
    Empty,
    Import(Box<Preview>),
    Report(Report),
}
struct Slot {
    owner: String,
    created: Instant,
    cancel: Arc<AtomicBool>,
    path: Option<PathBuf>,
    payload: Payload,
}
impl Drop for Slot {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
#[derive(Default)]
pub struct DataState {
    slots: Arc<Mutex<BTreeMap<String, Slot>>>,
}
impl DataState {
    pub fn release_owner(&self, owner: String) {
        let slots = self.slots.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Ok(mut slots) = slots.lock() {
                slots.retain(|_, s| s.owner != owner);
            }
        });
    }
    pub fn new() -> Self {
        let state = Self::default();
        let weak = Arc::downgrade(&state.slots);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(30));
            let Some(slots) = weak.upgrade() else { break };
            if let Ok(mut slots) = slots.lock() {
                slots.retain(|_, s| s.created.elapsed() < TTL);
            };
        });
        state
    }
}
fn slot<'a>(
    slots: &'a mut BTreeMap<String, Slot>,
    token: &str,
    owner: &str,
) -> Result<&'a mut Slot> {
    slots
        .get_mut(token)
        .filter(|s| s.owner == owner && s.created.elapsed() < TTL)
        .ok_or("data_expired".into())
}
fn phase(window: &WebviewWindow, token: &str, value: &str) {
    let _ = window.emit(
        "data:phase",
        serde_json::json!({"token":token,"phase":value}),
    );
}
#[derive(Serialize)]
pub struct PreviewInfo {
    token: String,
    filename: String,
    exported_at: String,
    app_version: String,
    source_timezone: Option<String>,
    start: Option<i64>,
    end: Option<i64>,
    has_profiles: bool,
    has_preferences: bool,
    duplicates: usize,
    options: Options,
    summary: Summary,
    conflicts: usize,
    renames: usize,
    digest: String,
}
fn info(token: &str, s: &Slot, p: &Preview) -> PreviewInfo {
    PreviewInfo {
        token: token.into(),
        filename: s
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
        exported_at: p.package.exported_at.clone(),
        app_version: p.package.app_version.clone(),
        source_timezone: p.package.source_timezone.clone(),
        start: p
            .package
            .sessions
            .iter()
            .map(|s| s.started_at_utc_secs)
            .min(),
        end: p
            .package
            .sessions
            .iter()
            .map(|s| s.started_at_utc_secs)
            .max(),
        has_profiles: p.package.timer_profiles.is_some(),
        has_preferences: p.package.portable_preferences.is_some(),
        duplicates: p.duplicates,
        options: p.options.clone(),
        summary: p.plan.summary.clone(),
        conflicts: p.plan.conflicts.len(),
        renames: p.plan.renames.len(),
        digest: p.digest.clone(),
    }
}
#[tauri::command]
pub async fn data_begin(app: AppHandle, window: WebviewWindow) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DataState>();
        let mut slots = state.slots.lock().map_err(err)?;
        slots.retain(|_, s| s.owner != window.label() && s.created.elapsed() < TTL);
        let token = uuid::Uuid::new_v4().to_string();
        slots.insert(
            token.clone(),
            Slot {
                owner: window.label().into(),
                created: Instant::now(),
                cancel: Arc::new(AtomicBool::new(false)),
                path: None,
                payload: Payload::Empty,
            },
        );
        Ok(token)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn data_cancel(app: AppHandle, window: WebviewWindow, token: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DataState>();
        let mut slots = state.slots.lock().map_err(err)?;
        if slots.get(&token).is_some_and(|s| s.owner == window.label()) {
            slots.remove(&token);
        }
        Ok(())
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn data_read(
    app: AppHandle,
    window: WebviewWindow,
    token: String,
    locale: String,
) -> Result<Option<PreviewInfo>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DataState>();
        let cancel = slot(
            &mut *state.slots.lock().map_err(err)?,
            &token,
            window.label(),
        )?
        .cancel
        .clone();
        let selected = app
            .dialog()
            .file()
            .set_parent(&window)
            .add_filter("Pomotroid", &["pomotroid.json"])
            .blocking_pick_file();
        let Some(selected) = selected else {
            return Ok(None);
        };
        if cancel.load(Ordering::Acquire) {
            return Err("data_cancelled".into());
        }
        let path = selected.into_path().map_err(err)?;
        slot(
            &mut *state.slots.lock().map_err(err)?,
            &token,
            window.label(),
        )?
        .path = Some(path.clone());
        phase(&window, &token, "reading");
        let mut file = std::fs::File::open(&path).map_err(|_| "data_read_failed")?;
        if !file.metadata().map_err(err)?.is_file() {
            return Err("data_format".into());
        }
        if file.metadata().map_err(err)?.len() > package::MAX_BYTES {
            return Err("data_too_large".into());
        }
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err("data_cancelled".into());
            }
            let n = file.read(&mut buffer).map_err(|_| "data_read_failed")?;
            if n == 0 {
                break;
            }
            if bytes.len() as u64 + n as u64 > package::MAX_BYTES {
                return Err("data_too_large".into());
            }
            bytes.extend_from_slice(&buffer[..n]);
        }
        phase(&window, &token, "validating");
        let (package, duplicates) = package::decode(&bytes)?;
        let digest = package::hash(&package);
        drop(bytes);
        if cancel.load(Ordering::Acquire) {
            return Err("data_cancelled".into());
        }
        let options = Options {
            history: true,
            profiles: package.timer_profiles.is_some(),
            preferences: false,
        };
        let db = app.state::<DbState>();
        let conn = db.lock().map_err(err)?;
        let tx = conn.unchecked_transaction().map_err(err)?;
        let plan = package::plan(&tx, &package, &options, &locale)?;
        tx.commit().map_err(err)?;
        drop(conn);
        let p = Preview {
            package: Arc::new(package),
            duplicates,
            options,
            plan,
            locale,
            digest,
        };
        let mut slots = state.slots.lock().map_err(err)?;
        let s = slot(&mut slots, &token, window.label())?;
        let info = info(&token, s, &p);
        s.payload = Payload::Import(Box::new(p));
        Ok(Some(info))
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn data_replan(
    app: AppHandle,
    window: WebviewWindow,
    token: String,
    options: Options,
) -> Result<PreviewInfo> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DataState>();
        let mut slots = state.slots.lock().map_err(err)?;
        let s = slot(&mut slots, &token, window.label())?;
        let Payload::Import(p) = &mut s.payload else {
            return Err("data_expired".into());
        };
        let db = app.state::<DbState>();
        let conn = db.lock().map_err(err)?;
        let tx = conn.unchecked_transaction().map_err(err)?;
        p.plan = package::plan(&tx, &p.package, &options, &p.locale)?;
        tx.commit().map_err(err)?;
        p.options = options;
        Ok(info(
            &token,
            s,
            match &s.payload {
                Payload::Import(p) => p,
                _ => unreachable!(),
            },
        ))
    })
    .await
    .map_err(err)?
}
#[derive(Serialize)]
pub struct Details {
    conflicts: Vec<package::Conflict>,
    renames: Vec<package::Rename>,
}
#[tauri::command]
pub async fn data_details(
    app: AppHandle,
    window: WebviewWindow,
    token: String,
    offset: usize,
    renames: bool,
) -> Result<Details> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DataState>();
        let mut slots = state.slots.lock().map_err(err)?;
        let s = slot(&mut slots, &token, window.label())?;
        let Payload::Import(p) = &s.payload else {
            return Err("data_expired".into());
        };
        Ok(Details {
            conflicts: if renames {
                vec![]
            } else {
                p.plan
                    .conflicts
                    .iter()
                    .skip(offset)
                    .take(20)
                    .cloned()
                    .collect()
            },
            renames: if renames {
                p.plan
                    .renames
                    .iter()
                    .skip(offset)
                    .take(20)
                    .cloned()
                    .collect()
            } else {
                vec![]
            },
        })
    })
    .await
    .map_err(err)?
}
#[derive(Serialize)]
pub struct CommitResult {
    pub committed: bool,
    pub refresh_failed: bool,
    pub preview: Option<PreviewInfo>,
    pub summary: Summary,
}
#[tauri::command]
pub async fn data_commit(
    app: AppHandle,
    window: WebviewWindow,
    token: String,
) -> Result<CommitResult> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DataState>();
        let mut slots = state.slots.lock().map_err(err)?;
        let s = slot(&mut slots, &token, window.label())?;
        let Payload::Import(p) = &mut s.payload else {
            return Err("data_expired".into());
        };
        phase(&window, &token, "importing");
        let db = app.state::<DbState>();
        let (plan, committed, refresh_failed) = app
            .state::<TimerController>()
            .import_data(&app, &db, &p.package, &p.options, &p.plan, &p.locale)?;
        p.plan = plan;
        let summary = p.plan.summary.clone();
        let preview = if committed {
            None
        } else {
            Some(info(
                &token,
                s,
                match &s.payload {
                    Payload::Import(p) => p,
                    _ => unreachable!(),
                },
            ))
        };
        if committed {
            slots.remove(&token);
        }
        Ok(CommitResult {
            committed,
            refresh_failed,
            preview,
            summary,
        })
    })
    .await
    .map_err(err)?
}
#[derive(Serialize)]
pub struct Saved {
    filename: String,
    rows: usize,
    categories: usize,
    profiles: usize,
    warning: bool,
}
fn save(
    app: &AppHandle,
    window: &WebviewWindow,
    filename: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<Option<String>> {
    let selected = app
        .dialog()
        .file()
        .set_parent(window)
        .set_file_name(filename)
        .add_filter("Pomotroid", &[extension])
        .blocking_save_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(err)?;
    // The native dialog determines the exact name (and confirms overwrites). Never change it afterwards.
    if !path
        .to_string_lossy()
        .to_lowercase()
        .ends_with(&format!(".{extension}"))
    {
        return Err("data_suffix".into());
    }
    let state = app.state::<DataState>();
    let paths = state
        .slots
        .lock()
        .map_err(err)?
        .values()
        .filter_map(|s| s.path.clone())
        .collect::<Vec<_>>();
    files::allowed(&path, &app.path().app_data_dir().map_err(err)?, &paths)?;
    files::atomic_save(&path, bytes).map_err(|e| {
        log::error!("[data] file save failed: {e}");
        "data_save_failed".to_string()
    })?;
    Ok(Some(
        path.file_name()
            .ok_or("data_path")?
            .to_string_lossy()
            .into_owned(),
    ))
}
#[tauri::command]
pub async fn data_export(
    app: AppHandle,
    window: WebviewWindow,
    profiles: bool,
    preferences: bool,
) -> Result<Option<Saved>> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = app.state::<DbState>();
        let mut p = {
            let conn = db.lock().map_err(err)?;
            let tx = conn.unchecked_transaction().map_err(err)?;
            let p = package::snapshot(&tx, profiles, preferences)?;
            tx.commit().map_err(err)?;
            p
        };
        package::validate(&mut p)?;
        let bytes = package::encode(&p)?;
        let name = format!(
            "Pomotroid-data-{}.pomotroid.json",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        let Some(filename) = save(&app, &window, &name, "pomotroid.json", &bytes)? else {
            return Ok(None);
        };
        let warning = settings::save_setting(
            &*db.lock().map_err(err)?,
            "data_last_export",
            &chrono::Utc::now().to_rfc3339(),
        )
        .is_err();
        Ok(Some(Saved {
            filename,
            rows: p.sessions.len(),
            categories: p.categories.len(),
            profiles: p.timer_profiles.map_or(0, |p| p.len()),
            warning,
        }))
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub fn data_last_export(app: AppHandle) -> Result<Option<String>> {
    Ok(settings::get_setting(
        &*app.state::<DbState>().lock().map_err(err)?,
        "data_last_export",
    ))
}
#[tauri::command]
pub async fn report_preview(
    app: AppHandle,
    window: WebviewWindow,
    token: String,
    scope: ReportScope,
    locale: String,
) -> Result<ReportInfo> {
    tauri::async_runtime::spawn_blocking(move || {
        let report = report::build(
            &*app.state::<DbState>().lock().map_err(err)?,
            scope,
            &locale,
        )?;
        let info = report.info.clone();
        let state = app.state::<DataState>();
        let mut slots = state.slots.lock().map_err(err)?;
        slot(&mut slots, &token, window.label())?.payload = Payload::Report(report);
        Ok(info)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn report_save(
    app: AppHandle,
    window: WebviewWindow,
    token: String,
) -> Result<Option<Saved>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DataState>();
        let (bytes, info) = {
            let mut slots = state.slots.lock().map_err(err)?;
            let s = slot(&mut slots, &token, window.label())?;
            let Payload::Report(r) = &s.payload else {
                return Err("data_expired".into());
            };
            (r.bytes.clone(), r.info.clone())
        };
        let Some(filename) = save(&app, &window, &info.filename, "csv", &bytes)? else {
            return Ok(None);
        };
        state.slots.lock().map_err(err)?.remove(&token);
        Ok(Some(Saved {
            filename,
            rows: info.rows,
            categories: 0,
            profiles: 0,
            warning: false,
        }))
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn stats_distribution(
    app: AppHandle,
    start: String,
    end: String,
) -> Result<report::Distribution> {
    tauri::async_runtime::spawn_blocking(move || {
        report::distribution(&*app.state::<DbState>().lock().map_err(err)?, &start, &end)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn data_view_stats(app: AppHandle) -> Result<()> {
    crate::auxiliary_windows::open(app.clone(), crate::auxiliary_windows::Kind::Stats).await?;
    app.emit_to("stats", "stats:all", ()).map_err(err)
}
