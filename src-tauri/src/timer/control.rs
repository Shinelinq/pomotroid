use super::{
    engine::TimerCommand,
    plans::{PlanAction, PlanBook, TimerConfig},
    sequence::SequenceState,
    TimerSnapshot,
};
use crate::{
    db::{
        categories::{self, CategoryAction, CategoryData},
        queries,
    },
    settings::Settings,
};
use rusqlite::Connection;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct PlanState {
    pub book: PlanBook,
    pub active_id: String,
    pub pending_id: Option<String>,
    pub pending_revision: u64,
    pub modified: bool,
    pub timer: TimerSnapshot,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryState {
    pub data: CategoryData,
    pub revision: u64,
    pub timer: TimerSnapshot,
}

#[derive(Debug, Clone, Copy)]
pub enum ResetKind {
    Round,
    Cycle,
    ApplyNow,
}

pub struct Control {
    pub settings: Settings,
    pub book: PlanBook,
    pub sequence: SequenceState,
    pub active_id: String,
    pub config: TimerConfig,
    pub pending_id: Option<String>,
    pub pending_revision: u64,
    pub round_id: u64,
    pub revision: u64,
    pub captured_at_ms: u64,
    pub started: bool,
    pub is_running: bool,
    pub elapsed_secs: u32,
    pub stop_after: bool,
    pub stopped_after: bool,
    pub session_id: Option<i64>,
    pub reset_kind: ResetKind,
    pub categories: CategoryData,
    pub category_id: Option<i64>,
    pub category_revision: u64,
    pub category_notice_id: Option<i64>,
}

impl Control {
    pub fn new(mut settings: Settings, book: PlanBook) -> Self {
        book.working.apply(&mut settings);
        Self {
            sequence: SequenceState::new(book.working.long_break_interval),
            active_id: book.selected_id.clone(),
            config: book.working.clone(),
            book,
            settings,
            pending_id: None,
            pending_revision: 0,
            round_id: 1,
            revision: 0,
            captured_at_ms: now_ms(),
            started: false,
            is_running: false,
            elapsed_secs: 0,
            stop_after: false,
            stopped_after: false,
            session_id: None,
            reset_kind: ResetKind::Round,
            categories: CategoryData::default(),
            category_id: None,
            category_revision: 0,
            category_notice_id: None,
        }
    }

    pub fn duration(&self) -> u32 {
        let mut settings = self.settings.clone();
        self.config.apply(&mut settings);
        self.sequence.current_duration_secs(&settings)
    }

    pub fn touch(&mut self) {
        self.revision += 1;
        self.captured_at_ms = now_ms();
    }

    pub fn snapshot(&self) -> TimerSnapshot {
        TimerSnapshot {
            round_type: self.sequence.current_round.as_str().into(),
            previous_round_type: self
                .sequence
                .previous_round
                .map(|r| r.as_str().into())
                .unwrap_or_default(),
            elapsed_secs: self.elapsed_secs,
            total_secs: self.duration(),
            is_running: self.is_running,
            is_paused: self.started && !self.is_running,
            work_round_number: self.sequence.work_round_number,
            work_rounds_total: self.sequence.work_rounds_total,
            session_work_count: self.sequence.session_work_count,
            round_id: self.round_id,
            revision: self.revision,
            captured_at_ms: self.captured_at_ms,
            has_started: self.started,
            stop_after_round: self.stop_after,
            stopped_after_round: self.stopped_after,
            category_id: self.category_id,
            next_category_id: self.categories.selected_id,
            category_pending: self.started
                && self.sequence.current_round == super::sequence::RoundType::Work
                && self.category_id != self.categories.selected_id,
            category_notice_id: self.category_notice_id,
            session_id: self.session_id,
        }
    }

    pub fn view(&self) -> PlanState {
        PlanState {
            modified: self
                .book
                .find(&self.book.selected_id)
                .is_ok_and(|p| p.config != self.book.working),
            book: self.book.clone(),
            active_id: self.active_id.clone(),
            pending_id: self.pending_id.clone(),
            pending_revision: self.pending_revision,
            timer: self.snapshot(),
        }
    }

    pub fn tray_view(&self) -> crate::tray::presentation::TrayView {
        let name = |id| {
            self.categories
                .items
                .iter()
                .find(|c| Some(c.id) == id)
                .map(|c| c.name.clone())
        };
        let id = if self.started {
            self.category_id
        } else {
            self.categories.selected_id
        };
        crate::tray::presentation::TrayView {
            timer: self.snapshot(),
            settings: self.settings.clone(),
            category_name: name(id),
            next_category_name: name(self.categories.selected_id),
            pending_plan: self
                .pending_id
                .as_ref()
                .and_then(|id| self.book.find(id).ok())
                .map(|p| (p.name.clone(), p.initial_name)),
        }
    }

    pub fn category_view(&self) -> CategoryState {
        CategoryState {
            data: self.categories.clone(),
            revision: self.category_revision,
            timer: self.snapshot(),
        }
    }

    pub fn category_action(
        &mut self,
        action: CategoryAction,
        conn: &Connection,
    ) -> Result<(), String> {
        if matches!(action, CategoryAction::DismissNotice) {
            self.category_notice_id = None;
            return Ok(());
        }
        let action = match action {
            CategoryAction::CancelPending { round_id } => {
                if round_id != self.round_id
                    || !self.started
                    || self.sequence.current_round != super::sequence::RoundType::Work
                {
                    return Err("category_stale_round".into());
                }
                CategoryAction::Select {
                    id: self.category_id,
                }
            }
            action => action,
        };
        let fallback = match &action {
            CategoryAction::Archive { id } if self.categories.selected_id == Some(*id) => Some(*id),
            _ => None,
        };
        let data = categories::mutate(conn, &action)?;
        self.categories = data;
        if matches!(action, CategoryAction::Select { .. })
            || matches!(action, CategoryAction::Restore { id } if self.category_notice_id == Some(id))
        {
            self.category_notice_id = None;
        }
        if let Some(id) = fallback {
            self.category_notice_id = Some(id);
        }
        Ok(())
    }

    fn use_working(&mut self) {
        self.config = self.book.working.clone();
        self.active_id = self.book.selected_id.clone();
        self.sequence.work_rounds_total = self.config.long_break_interval;
        self.pending_id = None;
        self.pending_revision += 1;
    }

    fn arrange(&mut self) -> Vec<TimerCommand> {
        self.book.working.apply(&mut self.settings);
        self.pending_revision += 1;
        if self.config == self.book.working {
            self.active_id = self.book.selected_id.clone();
            self.pending_id = None;
        } else if self.started {
            self.pending_id = Some(self.book.selected_id.clone());
        } else {
            self.use_working();
            return vec![TimerCommand::Prime {
                duration_secs: self.duration(),
            }];
        }
        vec![]
    }

    pub fn plan_action(
        &mut self,
        action: PlanAction,
        conn: &Connection,
    ) -> Result<Vec<TimerCommand>, String> {
        // Validate and persist a copy first. A failed transaction leaves runtime state unchanged.
        let mut book = self.book.clone();
        let mut arrange = false;
        match action {
            PlanAction::ReplaceConfig { config } => {
                config.validate()?;
                book.working = config;
                arrange = true;
            }
            PlanAction::Select { id } => {
                book.working = book.find(&id)?.config.clone();
                book.selected_id = id;
                arrange = true;
            }
            PlanAction::Edit { key, value } => {
                book.working.change(&key, &value)?;
                arrange = true;
            }
            PlanAction::Save => {
                let config = book.working.clone();
                book.plans
                    .iter_mut()
                    .find(|p| p.id == book.selected_id)
                    .ok_or("plan_missing")?
                    .config = config;
            }
            PlanAction::SaveAs { name } => {
                book.selected_id = book.create(&name, book.working.clone())?;
                arrange = true;
            }
            PlanAction::Rename { id, name } => {
                let name = book.name(&name, Some(&id))?;
                let plan = book
                    .plans
                    .iter_mut()
                    .find(|p| p.id == id)
                    .ok_or("plan_missing")?;
                plan.name = name;
                plan.initial_name = false;
            }
            PlanAction::Delete { id } => {
                book.find(&id)?;
                if book.plans.len() == 1 {
                    return Err("plan_last".into());
                }
                if id == self.active_id
                    || self.pending_id.as_ref() == Some(&id)
                    || id == book.selected_id
                {
                    return Err("plan_in_use".into());
                }
                book.plans.retain(|p| p.id != id);
            }
            PlanAction::Template { name, long } => {
                book.create(&name, TimerConfig::template(long))?;
            }
            PlanAction::CancelPending => {
                book.selected_id = self.active_id.clone();
                book.working = self.config.clone();
                arrange = true;
            }
            PlanAction::ApplyNow {
                round_id,
                pending_revision,
            } => {
                if round_id != self.round_id
                    || pending_revision != self.pending_revision
                    || self.pending_id.is_none()
                {
                    return Err("plan_stale_round".into());
                }
                self.reset_kind = ResetKind::ApplyNow;
                self.stop_after = false;
                return Ok(vec![TimerCommand::Reset]);
            }
            PlanAction::StopAfter { round_id, enabled } => {
                if round_id != self.round_id || !self.started {
                    return Err("plan_stale_round".into());
                }
                self.stop_after = enabled;
                return Ok(vec![]);
            }
        }
        book.persist(conn)?;
        self.book = book;
        Ok(if arrange { self.arrange() } else { vec![] })
    }

    pub fn start(&mut self) -> TimerCommand {
        // Lock before Start reaches the engine, including the interval before its first tick.
        self.category_id = if self.sequence.current_round == super::sequence::RoundType::Work {
            self.category_notice_id = None;
            self.categories.selected_id
        } else {
            None
        };
        self.started = true;
        self.is_running = true;
        self.stopped_after = false;
        TimerCommand::Start
    }

    pub fn record_tick(&mut self, elapsed: u32, conn: &Connection) -> Result<(), String> {
        self.elapsed_secs = elapsed;
        self.is_running = true;
        if self.started && self.session_id.is_none() {
            self.session_id = Some(
                queries::insert_session_with_category(
                    conn,
                    self.sequence.current_round.as_str(),
                    self.duration(),
                    self.category_id,
                )
                .map_err(|e| e.to_string())?,
            );
        }
        Ok(())
    }

    fn end_record(&mut self, conn: &Connection, completed: bool) -> Result<(), String> {
        if let Some(id) = self.session_id {
            queries::complete_session(conn, id, completed).map_err(|e| e.to_string())?;
            self.session_id = None;
        }
        Ok(())
    }

    pub fn complete(
        &mut self,
        skipped: bool,
        conn: &Connection,
    ) -> Result<Vec<TimerCommand>, String> {
        self.end_record(conn, !skipped)?;
        let stop = self.stop_after && !skipped;
        if self.pending_id.is_some() {
            self.use_working();
        }
        let mut settings = self.settings.clone();
        self.config.apply(&mut settings);
        let (round, duration) = self.sequence.advance(&settings);
        self.new_round();
        self.stopped_after = stop;
        let mut commands = vec![TimerCommand::Prime {
            duration_secs: duration,
        }];
        let auto = if round == super::sequence::RoundType::Work {
            self.config.auto_start_work
        } else {
            self.config.auto_start_break
        };
        if auto && !stop {
            commands.push(self.start());
        }
        Ok(commands)
    }

    fn new_round(&mut self) {
        self.round_id += 1;
        self.category_id = None;
        self.elapsed_secs = 0;
        self.started = false;
        self.is_running = false;
        self.stop_after = false;
        self.stopped_after = false;
    }

    pub fn reset(&mut self, conn: &Connection) -> Result<Vec<TimerCommand>, String> {
        self.end_record(conn, false)?;
        let kind = self.reset_kind;
        self.reset_kind = ResetKind::Round;
        if matches!(kind, ResetKind::Cycle | ResetKind::ApplyNow) {
            self.use_working();
            self.sequence.reset();
        }
        self.new_round();
        let mut commands = vec![TimerCommand::Prime {
            duration_secs: self.duration(),
        }];
        if matches!(kind, ResetKind::ApplyNow) {
            commands.push(self.start());
        }
        Ok(commands)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
