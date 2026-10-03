mod control;
pub mod engine;
pub mod plans;
pub mod sequence;

use crate::audio::{AudioCue, AudioManager};
use crate::db::{
    categories::{self, CategoryAction},
    DbState,
};
use crate::settings::Settings;
use crate::tray::{self, TrayState};
use crate::websocket::{self, WsState};
pub use control::{CategoryState, PlanState};
use control::{Control, ResetKind};
use engine::{EngineHandle, TimerCommand, TimerEvent};
use plans::{PlanAction, PlanBook};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Default, Serialize)]
pub struct TimerSnapshot {
    /// "work" | "short-break" | "long-break"
    pub round_type: String,
    /// Round type that was active before this one. Empty string on the first round of a session.
    pub previous_round_type: String,
    pub elapsed_secs: u32,
    pub total_secs: u32,
    pub is_running: bool,
    /// Started but not running, including a pause before the first tick.
    pub is_paused: bool,
    pub work_round_number: u32,
    pub work_rounds_total: u32,
    /// Monotonically-increasing focus round count since last reset. Used as a
    /// session counter when long breaks are disabled.
    pub session_work_count: u32,
    pub round_id: u64,
    pub revision: u64,
    pub captured_at_ms: u64,
    pub has_started: bool,
    pub stop_after_round: bool,
    pub stopped_after_round: bool,
    pub category_id: Option<i64>,
    pub next_category_id: Option<i64>,
    pub category_pending: bool,
    pub category_notice_id: Option<i64>,
    /// Read-only identity of the row already written by this runtime.
    pub session_id: Option<i64>,
}

enum Action {
    Toggle,
    Reset(ResetKind),
    Skip,
    Suspend,
    WakeResume,
    Settings(Box<Settings>),
    Plan(PlanAction),
    Category(CategoryAction),
}
struct Request {
    action: Action,
    result: Option<mpsc::Sender<Result<PlanState, String>>>,
    category_result: Option<mpsc::Sender<Result<CategoryState, String>>>,
}
type Requests = Arc<Mutex<HashMap<u64, Request>>>;

pub struct TimerController {
    engine: EngineHandle,
    control: Arc<Mutex<Control>>,
    requests: Requests,
    next: AtomicU64,
    pub plans_focus: AtomicBool,
    pub categories_focus: AtomicBool,
}

impl TimerController {
    /// The same control -> database lock order as timer events. Parsing and preview never
    /// enter this section; starts from every window/tray wait only for the final transaction.
    pub fn import_data(
        &self,
        app: &AppHandle,
        db: &DbState,
        package: &crate::data::package::Package,
        options: &crate::data::package::Options,
        confirmed: &crate::data::package::ImportPlan,
        locale: &str,
    ) -> Result<(crate::data::package::ImportPlan, bool, bool), String> {
        let mut state = self.control.lock().map_err(crate::data::err)?;
        let conn = db.lock().map_err(crate::data::err)?;
        let (plan, committed) = state.merge_import(&conn, package, options, confirmed, locale)?;
        if !committed {
            return Ok((plan, false, false));
        }
        let preferences = state.settings.clone();
        let (plans, categories, view) = (state.view(), state.category_view(), state.tray_view());
        drop(conn);
        drop(state);
        let mut refresh_failed = app.emit("data:changed", ()).is_err();
        refresh_failed |= app.emit("plans:changed", plans).is_err();
        refresh_failed |= app.emit("categories:changed", categories).is_err();
        if options.preferences {
            refresh_failed |= app.emit("settings:changed", preferences).is_err();
        }
        if let Some(tray) = app.try_state::<Arc<TrayState>>() {
            tray::present(app, &tray, view);
        }
        Ok((plan, true, refresh_failed))
    }
    pub fn new(app: AppHandle, settings: Settings, tray: Arc<TrayState>, db: DbState) -> Self {
        let book =
            PlanBook::load(&db.lock().unwrap(), &settings).expect("failed to load timer plans");
        let control = Arc::new(Mutex::new(Control::new(settings, book)));
        control.lock().unwrap().categories =
            categories::load(&db.lock().unwrap()).expect("failed to load categories");
        tray::present(&app, &tray, control.lock().unwrap().tray_view());
        let (engine, events) =
            engine::spawn_controlled(control.lock().unwrap().duration(), Duration::from_secs(1));
        let requests = Arc::new(Mutex::new(HashMap::new()));
        let state = Arc::clone(&control);
        let jobs = Arc::clone(&requests);
        std::thread::Builder::new()
            .name("timer-events".into())
            .spawn(move || {
                listen_events(app, events, state, jobs, tray, db);
            })
            .expect("failed to spawn timer event listener");
        Self {
            engine,
            control,
            requests,
            next: AtomicU64::new(1),
            plans_focus: AtomicBool::new(false),
            categories_focus: AtomicBool::new(false),
        }
    }

    fn send(&self, action: Action, result: Option<mpsc::Sender<Result<PlanState, String>>>) {
        self.enqueue(Request {
            action,
            result,
            category_result: None,
        });
    }
    fn enqueue(&self, request: Request) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.requests.lock().unwrap().insert(id, request);
        self.engine.send(TimerCommand::Dispatch(id));
    }
    pub fn categories(&self) -> CategoryState {
        self.control.lock().unwrap().category_view()
    }
    pub fn category_action(&self, action: CategoryAction) -> Result<CategoryState, String> {
        let (tx, rx) = mpsc::channel();
        self.enqueue(Request {
            action: Action::Category(action),
            result: None,
            category_result: Some(tx),
        });
        rx.recv()
            .map_err(|_| "timer controller disconnected".to_string())?
    }
    pub fn toggle(&self) {
        self.send(Action::Toggle, None);
    }
    pub fn reset(&self) {
        self.send(Action::Reset(ResetKind::Cycle), None);
    }
    pub fn restart_round(&self) {
        self.send(Action::Reset(ResetKind::Round), None);
    }
    pub fn skip(&self) {
        self.send(Action::Skip, None);
    }
    pub fn suspend(&self) {
        self.send(Action::Suspend, None);
    }
    pub fn wake_resume(&self) {
        self.send(Action::WakeResume, None);
    }
    pub fn apply_settings(&self, settings: Settings) {
        self.send(Action::Settings(Box::new(settings)), None);
    }
    pub fn get_snapshot(&self) -> TimerSnapshot {
        self.control.lock().unwrap().snapshot()
    }
    pub fn plans(&self) -> PlanState {
        self.control.lock().unwrap().view()
    }
    pub fn plan_action(&self, action: PlanAction) -> Result<PlanState, String> {
        let (tx, rx) = mpsc::channel();
        self.send(Action::Plan(action), Some(tx));
        rx.recv()
            .map_err(|_| "timer controller disconnected".to_string())?
    }
}

fn apply_action(
    state: &mut Control,
    action: Action,
    db: &DbState,
) -> Result<Vec<TimerCommand>, String> {
    Ok(match action {
        Action::Toggle => {
            if state.is_running {
                vec![TimerCommand::Pause]
            } else if state.started {
                vec![TimerCommand::Resume]
            } else {
                vec![state.start()]
            }
        }
        Action::Reset(kind) => {
            state.reset_kind = kind;
            state.stop_after = false;
            vec![TimerCommand::Reset]
        }
        Action::Skip => {
            state.stop_after = false;
            vec![TimerCommand::Skip]
        }
        Action::Suspend => vec![TimerCommand::Suspend],
        Action::WakeResume => vec![TimerCommand::WakeResume],
        Action::Plan(action) => {
            state.plan_action(action, &*db.lock().map_err(|e| e.to_string())?)?
        }
        Action::Category(action) => {
            state.category_action(action, &*db.lock().map_err(|e| e.to_string())?)?;
            state.category_revision += 1;
            vec![]
        }
        Action::Settings(mut settings) => {
            // Non-timer preferences cannot replace a concurrently selected working plan.
            state.book.working.apply(&mut settings);
            state.settings = *settings;
            vec![]
        }
    })
}

fn publish(app: &AppHandle, state: &mut Control, plans: bool) {
    state.touch();
    let _ = app.emit("timer:state", state.snapshot());
    if plans {
        let _ = app.emit("plans:changed", state.view());
    }
}

fn listen_events(
    app: AppHandle,
    events: mpsc::Receiver<TimerEvent>,
    control: Arc<Mutex<Control>>,
    requests: Requests,
    tray: Arc<TrayState>,
    db: DbState,
) {
    let mut boundary_commands = Vec::new();
    let mut reset_result: Option<mpsc::Sender<Result<PlanState, String>>> = None;
    while let Ok(event) = events.recv() {
        if let TimerEvent::Boundary { reply } = event {
            let _ = reply.send(std::mem::take(&mut boundary_commands));
            continue;
        }
        let mut state = control.lock().unwrap();
        if let TimerEvent::Dispatch { id, reply } = event {
            let request = requests.lock().unwrap().remove(&id);
            if let Some(request) = request {
                let changed_categories = matches!(request.action, Action::Category(_));
                let changed_settings = matches!(request.action, Action::Plan(ref a) if !matches!(a, PlanAction::StopAfter { .. } | PlanAction::ApplyNow { .. }));
                let result = apply_action(&mut state, request.action, &db);
                match result {
                    Ok(commands) => {
                        publish(&app, &mut state, !changed_categories);
                        if changed_categories {
                            let _ = app.emit("categories:changed", state.category_view());
                        }
                        if let Some(result) = request.category_result {
                            let _ = result.send(Ok(state.category_view()));
                        }
                        if changed_settings {
                            let _ = app.emit("settings:changed", &state.settings);
                        }
                        if let Some(result) = request.result {
                            if commands
                                .iter()
                                .any(|command| matches!(command, TimerCommand::Reset))
                            {
                                // Apply-now returns the new round after the engine confirms Reset.
                                reset_result = Some(result);
                            } else {
                                let _ = result.send(Ok(state.view()));
                            }
                        }
                        let _ = reply.send(commands);
                    }
                    Err(error) => {
                        if let Some(result) = request.result {
                            let _ = result.send(Err(error));
                        } else if let Some(result) = request.category_result {
                            let _ = result.send(Err(error));
                        } else {
                            log::error!("[timer] control operation failed: {error}");
                        }
                        let _ = reply.send(vec![]);
                    }
                }
            } else {
                let _ = reply.send(vec![]);
            }
            let view = state.tray_view();
            drop(state);
            tray::present(&app, &tray, view);
            continue;
        }
        let mut boundary_event = false;
        let mut top_update = None;
        match event {
            TimerEvent::Started { total_secs } => {
                state.started = true;
                state.is_running = true;
                let _ = app.emit(
                    "timer:started",
                    serde_json::json!({ "total_secs": total_secs }),
                );
                if let Some(ws) = app.try_state::<Arc<WsState>>() {
                    websocket::broadcast_started(&ws, total_secs);
                }
            }
            TimerEvent::Tick {
                elapsed_secs,
                total_secs,
            } => {
                if let Ok(conn) = db.lock() {
                    if let Err(error) = state.record_tick(elapsed_secs, &conn) {
                        log::error!("[timer] session insert failed: {error}");
                    }
                }
                let _ = app.emit(
                    "timer:tick",
                    serde_json::json!({ "elapsed_secs": elapsed_secs, "total_secs": total_secs }),
                );
                let rt = state.sequence.current_round.as_str();
                if let Some(audio) = app.try_state::<Arc<AudioManager>>() {
                    if audio.tick_enabled_for(rt) {
                        audio.play_cue(AudioCue::Tick);
                    }
                }
            }
            TimerEvent::Complete { skipped } => {
                let result = state.complete(skipped, &db.lock().unwrap());
                boundary_commands = result.unwrap_or_else(|error| {
                    log::error!("[timer] completion failed: {error}");
                    vec![]
                });
                state.touch();
                let snapshot = state.snapshot();
                let _ = app.emit("timer:round-change", &snapshot);
                if let Some(ws) = app.try_state::<Arc<WsState>>() {
                    websocket::broadcast_round_change(&ws, snapshot);
                }
                let next = state.sequence.current_round;
                if let Some(audio) = app.try_state::<Arc<AudioManager>>() {
                    audio.play_cue(match next {
                        sequence::RoundType::Work => AudioCue::WorkAlert,
                        sequence::RoundType::ShortBreak => AudioCue::ShortBreakAlert,
                        sequence::RoundType::LongBreak => AudioCue::LongBreakAlert,
                    });
                }
                if state.settings.always_on_top {
                    let is_break = next != sequence::RoundType::Work;
                    top_update = Some(!(state.settings.break_always_on_top && is_break));
                }
                boundary_event = true;
            }
            TimerEvent::Reset => {
                boundary_commands = state.reset(&db.lock().unwrap()).unwrap_or_else(|error| {
                    log::error!("[timer] reset failed: {error}");
                    if let Some(result) = reset_result.take() {
                        let _ = result.send(Err(error));
                    }
                    vec![]
                });
                state.touch();
                let _ = app.emit("timer:reset", state.snapshot());
                if let Some(ws) = app.try_state::<Arc<WsState>>() {
                    websocket::broadcast_reset(&ws);
                }
                boundary_event = true;
            }
            TimerEvent::Paused { elapsed_secs } | TimerEvent::Suspended { elapsed_secs } => {
                state.elapsed_secs = elapsed_secs;
                state.is_running = false;
                let _ = app.emit(
                    "timer:paused",
                    serde_json::json!({ "elapsed_secs": elapsed_secs }),
                );
                if let Some(ws) = app.try_state::<Arc<WsState>>() {
                    websocket::broadcast_paused(&ws, elapsed_secs);
                }
            }
            TimerEvent::Resumed { elapsed_secs } => {
                state.elapsed_secs = elapsed_secs;
                state.is_running = true;
                let _ = app.emit(
                    "timer:resumed",
                    serde_json::json!({ "elapsed_secs": elapsed_secs }),
                );
                if let Some(ws) = app.try_state::<Arc<WsState>>() {
                    websocket::broadcast_resumed(&ws, elapsed_secs);
                }
            }
            TimerEvent::Dispatch { .. } | TimerEvent::Boundary { .. } => unreachable!(),
        }
        publish(&app, &mut state, boundary_event);
        if boundary_event {
            if let Some(result) = reset_result.take() {
                let _ = result.send(Ok(state.view()));
            }
        }
        let view = state.tray_view();
        drop(state);
        tray::present(&app, &tray, view);
        // Native menu/window work may dispatch to the UI thread. Never hold the
        // controller mutex while that thread can request a timer snapshot.
        if let Some(top) = top_update {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(top);
            }
        }
    }
}

#[cfg(test)]
mod tests;
