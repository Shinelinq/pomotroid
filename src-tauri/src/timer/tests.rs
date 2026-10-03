use super::*;
use crate::{
    db::{migrations, queries},
    settings,
};
use plans::TimerConfig;
use rusqlite::Connection;

fn setup(settings: Settings) -> (Control, DbState) {
    let conn = Connection::open_in_memory().unwrap();
    migrations::run(&conn).unwrap();
    settings::seed_defaults(&conn).unwrap();
    let book = PlanBook::load(&conn, &settings).unwrap();
    (Control::new(settings, book), Arc::new(Mutex::new(conn)))
}

#[test]
fn import_preview_is_allowed_during_running_and_paused_rounds_but_commit_is_not() {
    use crate::data::package::{plan, snapshot, Options};
    let (mut state, db) = setup(Settings::default());
    let conn = db.lock().unwrap();
    let package = snapshot(&conn, true, false).unwrap();
    let options = Options {
        history: true,
        profiles: true,
        preferences: false,
    };
    state.start();
    let confirmed = plan(&conn, &package, &options, "en").unwrap();
    assert_eq!(
        state
            .merge_import(&conn, &package, &options, &confirmed, "en")
            .unwrap_err(),
        "data_active_round"
    );
    state.is_running = false;
    assert_eq!(
        state
            .merge_import(&conn, &package, &options, &confirmed, "en")
            .unwrap_err(),
        "data_active_round"
    );
    state.started = false;
    // An old unfinished database record is not an active runtime round.
    crate::db::queries::insert_session(&conn, "work", 60).unwrap();
    assert!(
        state
            .merge_import(&conn, &package, &options, &confirmed, "en")
            .unwrap()
            .1
    );
}

#[test]
fn stale_import_plan_never_writes_and_rollback_preserves_runtime() {
    use crate::data::package::{plan, snapshot, Options};
    let (mut state, db) = setup(Settings::default());
    let (_, source) = setup(Settings::default());
    let source = source.lock().unwrap();
    crate::db::categories::mutate(
        &source,
        &crate::db::categories::CategoryAction::Create {
            name: "Reading".into(),
        },
    )
    .unwrap();
    crate::db::queries::insert_session(&source, "work", 60).unwrap();
    let package = snapshot(&source, true, false).unwrap();
    let options = Options {
        history: true,
        profiles: true,
        preferences: false,
    };
    let conn = db.lock().unwrap();
    let confirmed = plan(&conn, &package, &options, "en").unwrap();
    crate::db::categories::mutate(
        &conn,
        &crate::db::categories::CategoryAction::Create {
            name: "Reading".into(),
        },
    )
    .unwrap();
    let (updated, committed) = state
        .merge_import(&conn, &package, &options, &confirmed, "en")
        .unwrap();
    assert!(!committed);
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap(),
        0
    );
    conn.execute_batch("CREATE TRIGGER fail_import BEFORE INSERT ON sessions BEGIN SELECT RAISE(ABORT,'test'); END").unwrap();
    let before = serde_json::to_string(&state.view()).unwrap();
    assert_eq!(
        state
            .merge_import(&conn, &package, &options, &updated, "en")
            .unwrap_err(),
        "data_rolled_back"
    );
    assert_eq!(serde_json::to_string(&state.view()).unwrap(), before);
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM categories", [], |r| r.get(0))
            .unwrap(),
        1
    );
}
fn action(state: &mut Control, db: &DbState, op: PlanAction) -> Result<Vec<TimerCommand>, String> {
    apply_action(state, Action::Plan(op), db)
}
fn template(state: &mut Control, db: &DbState, name: &str, long: bool) -> String {
    action(
        state,
        db,
        PlanAction::Template {
            name: name.into(),
            long,
        },
    )
    .unwrap();
    state.book.plans.last().unwrap().id.clone()
}
fn select(state: &mut Control, db: &DbState, id: &str) {
    action(state, db, PlanAction::Select { id: id.into() }).unwrap();
}
fn stop(state: &mut Control, db: &DbState, enabled: bool) {
    action(
        state,
        db,
        PlanAction::StopAfter {
            round_id: state.round_id,
            enabled,
        },
    )
    .unwrap();
}

#[test]
fn initial_plan_preserves_every_timer_field_once_and_survives_reload() {
    let original = Settings {
        time_work_secs: 5400,
        time_short_break_secs: 339,
        time_long_break_secs: 1799,
        long_break_interval: 7,
        short_breaks_enabled: false,
        long_breaks_enabled: false,
        auto_start_work: true,
        auto_start_break: false,
        ..Settings::default()
    };
    let (state, db) = setup(original.clone());
    let conn = db.lock().unwrap();
    let reloaded = PlanBook::load(&conn, &Settings::default()).unwrap();
    assert_eq!(reloaded.plans.len(), 1);
    assert_eq!(reloaded.working, TimerConfig::from_settings(&original));
    assert_eq!(reloaded.selected_id, state.active_id);
    assert_eq!(
        TimerConfig::from_settings(&settings::load(&conn).unwrap()),
        reloaded.working
    );
    assert_eq!(
        queries::get_all_time_stats(&conn)
            .unwrap()
            .completed_work_sessions,
        0
    );
}

#[test]
fn saving_names_stable_ids_deletion_rules_and_atomic_rollback() {
    let (mut state, db) = setup(Settings::default());
    let active = state.active_id.clone();
    assert_eq!(
        action(&mut state, &db, PlanAction::Delete { id: active.clone() }).unwrap_err(),
        "plan_last"
    );
    action(
        &mut state,
        &db,
        PlanAction::Edit {
            key: "time_work_secs".into(),
            value: "1801".into(),
        },
    )
    .unwrap();
    assert!(state.view().modified);
    assert_eq!(
        state.book.find(&active).unwrap().config.time_work_secs,
        1500
    );
    action(&mut state, &db, PlanAction::Save).unwrap();
    assert!(!state.view().modified);
    action(
        &mut state,
        &db,
        PlanAction::SaveAs {
            name: "  Precise  ".into(),
        },
    )
    .unwrap();
    let copy = state.active_id.clone();
    assert_ne!(copy, active);
    action(
        &mut state,
        &db,
        PlanAction::Rename {
            id: copy.clone(),
            name: "Renamed".into(),
        },
    )
    .unwrap();
    assert_eq!(state.active_id, copy);
    assert!(action(
        &mut state,
        &db,
        PlanAction::SaveAs {
            name: " renamed ".into()
        }
    )
    .is_err());
    assert!(action(&mut state, &db, PlanAction::SaveAs { name: "  ".into() }).is_err());
    assert!(action(
        &mut state,
        &db,
        PlanAction::SaveAs {
            name: "长".repeat(25)
        }
    )
    .is_err());
    assert_eq!(
        action(&mut state, &db, PlanAction::Delete { id: copy.clone() }).unwrap_err(),
        "plan_in_use"
    );
    action(&mut state, &db, PlanAction::Delete { id: active }).unwrap();
    let before = serde_json::to_string(&state.book).unwrap();
    db.lock().unwrap().execute_batch("CREATE TRIGGER fail_config BEFORE INSERT ON settings WHEN NEW.key='auto_start_break' BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
    assert!(action(
        &mut state,
        &db,
        PlanAction::Edit {
            key: "time_work_secs".into(),
            value: "2200".into()
        }
    )
    .is_err());
    assert_eq!(serde_json::to_string(&state.book).unwrap(), before);
    let loaded = PlanBook::load(&db.lock().unwrap(), &Settings::default()).unwrap();
    assert_eq!(serde_json::to_string(&loaded).unwrap(), before);
    assert_eq!(loaded.find(&copy).unwrap().name, "Renamed");
    assert_eq!(loaded.working.time_work_secs, 1801);
}

#[test]
fn templates_do_not_activate_and_idle_selection_preserves_round_type_and_cycle() {
    let (mut state, db) = setup(Settings::default());
    let original = state.active_id.clone();
    let new = template(&mut state, &db, "Long", true);
    assert_eq!(state.active_id, original);
    assert!(!state.book.find(&new).unwrap().config.auto_start_work);
    state.sequence.current_round = sequence::RoundType::ShortBreak;
    state.sequence.work_round_number = 3;
    select(&mut state, &db, &new);
    assert_eq!(state.duration(), 600);
    assert_eq!(state.sequence.work_round_number, 3);
    assert!(!state.started);
}

#[test]
fn start_before_first_tick_locks_duration_and_pending_last_selection_wins() {
    let (mut state, db) = setup(Settings::default());
    let original = state.active_id.clone();
    let a = template(&mut state, &db, "A", true);
    let b = template(&mut state, &db, "B", false);
    state.start();
    select(&mut state, &db, &a);
    assert_eq!(state.duration(), 1500);
    assert_eq!(state.active_id, original);
    state.record_tick(1, &db.lock().unwrap()).unwrap();
    assert_eq!(
        db.lock()
            .unwrap()
            .query_row::<u32, _, _>("SELECT duration_secs FROM sessions", [], |r| r.get(0))
            .unwrap(),
        1500
    );
    state.is_running = false;
    let remaining = state.duration() - state.elapsed_secs;
    select(&mut state, &db, &b);
    assert_eq!(state.pending_id, Some(b));
    select(&mut state, &db, &a);
    assert_eq!(state.pending_id, Some(a.clone()));
    assert_eq!(state.duration() - state.elapsed_secs, remaining);
    assert_eq!(
        action(&mut state, &db, PlanAction::Delete { id: a }).unwrap_err(),
        "plan_in_use"
    );
    select(&mut state, &db, &original);
    assert!(state.pending_id.is_none());
    assert_eq!(state.book.working, state.config);
}

#[test]
fn cancel_arrangement_restores_working_selection_and_restart_has_no_saved_progress() {
    let (mut state, db) = setup(Settings::default());
    let original = state.active_id.clone();
    let next = template(&mut state, &db, "Next", true);
    state.start();
    select(&mut state, &db, &next);
    stop(&mut state, &db, true);
    let loaded = PlanBook::load(&db.lock().unwrap(), &Settings::default()).unwrap();
    let restarted = Control::new(Settings::default(), loaded);
    assert_eq!(restarted.active_id, next);
    assert!(!restarted.started);
    assert!(!restarted.stop_after);
    assert!(restarted.pending_id.is_none());
    action(&mut state, &db, PlanAction::CancelPending).unwrap();
    assert!(state.pending_id.is_none());
    assert_eq!(state.book.selected_id, original);
    assert!(state.stop_after);
}

#[test]
fn restart_current_and_apply_now_are_distinct_and_keep_incomplete_records() {
    let (mut state, db) = setup(Settings::default());
    let next = template(&mut state, &db, "Next", true);
    state.sequence.work_round_number = 3;
    state.start();
    state.record_tick(20, &db.lock().unwrap()).unwrap();
    select(&mut state, &db, &next);
    stop(&mut state, &db, true);
    state.reset_kind = ResetKind::Round;
    let commands = state.reset(&db.lock().unwrap()).unwrap();
    assert_eq!(commands.len(), 1);
    assert_eq!(state.duration(), 1500);
    assert_eq!(state.sequence.work_round_number, 3);
    assert_eq!(state.pending_id, Some(next.clone()));
    assert!(!state.stop_after);
    assert!(!state.started);
    state.start();
    state.record_tick(1, &db.lock().unwrap()).unwrap();
    let request = PlanAction::ApplyNow {
        round_id: state.round_id,
        pending_revision: state.pending_revision,
    };
    action(&mut state, &db, request).unwrap();
    let commands = state.reset(&db.lock().unwrap()).unwrap();
    assert_eq!(commands.len(), 2);
    assert!(state.started);
    assert_eq!(state.duration(), 3000);
    assert_eq!(state.sequence.work_round_number, 1);
    assert_eq!(state.active_id, next);
    assert!(state.pending_id.is_none());
    let conn = db.lock().unwrap();
    assert_eq!(
        conn.query_row::<i64, _, _>(
            "SELECT COUNT(*) FROM sessions WHERE completed=0 AND ended_at IS NOT NULL",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        queries::get_all_time_stats(&conn)
            .unwrap()
            .completed_work_sessions,
        0
    );
}

#[test]
fn boundary_applies_new_break_rules_interval_and_auto_start_together() {
    let (mut state, db) = setup(Settings::default());
    state.sequence.work_round_number = 2;
    state.start();
    state.record_tick(1, &db.lock().unwrap()).unwrap();
    let mut config = TimerConfig::template(true);
    config.long_break_interval = 2;
    config.auto_start_break = true;
    action(
        &mut state,
        &db,
        PlanAction::ReplaceConfig {
            config: config.clone(),
        },
    )
    .unwrap();
    assert_eq!(state.sequence.work_rounds_total, 4);
    let commands = state.complete(false, &db.lock().unwrap()).unwrap();
    assert_eq!(state.sequence.current_round, sequence::RoundType::LongBreak);
    assert_eq!(state.duration(), 1800);
    assert_eq!(commands.len(), 2);
    config.short_breaks_enabled = false;
    config.long_breaks_enabled = false;
    config.auto_start_work = false;
    action(&mut state, &db, PlanAction::ReplaceConfig { config }).unwrap();
    state.complete(true, &db.lock().unwrap()).unwrap();
    assert_eq!(state.sequence.current_round, sequence::RoundType::Work);
    assert!(!state.started);
    state.start();
    state.complete(false, &db.lock().unwrap()).unwrap();
    assert_eq!(state.sequence.current_round, sequence::RoundType::Work);
    assert_eq!(state.sequence.work_round_number, 2);
}

#[test]
fn one_shot_stop_wins_over_new_auto_start_and_stale_requests_cannot_affect_next_round() {
    let (mut state, db) = setup(Settings::default());
    state.start();
    state.record_tick(1, &db.lock().unwrap()).unwrap();
    let old_id = state.round_id;
    stop(&mut state, &db, true);
    stop(&mut state, &db, true);
    state.is_running = false;
    assert!(state.stop_after);
    state.is_running = true;
    let mut config = TimerConfig::template(true);
    config.auto_start_break = true;
    action(&mut state, &db, PlanAction::ReplaceConfig { config }).unwrap();
    let commands = state.complete(false, &db.lock().unwrap()).unwrap();
    assert_eq!(commands.len(), 1);
    assert!(!state.started);
    assert!(state.stopped_after);
    assert!(!state.stop_after);
    assert!(state.config.auto_start_break);
    state.start();
    assert!(action(
        &mut state,
        &db,
        PlanAction::StopAfter {
            round_id: old_id,
            enabled: true
        }
    )
    .is_err());
    assert!(!state.stop_after);
    assert_eq!(
        queries::get_all_time_stats(&db.lock().unwrap())
            .unwrap()
            .completed_work_sessions,
        1
    );
    assert_eq!(
        queries::get_all_time_stats(&db.lock().unwrap())
            .unwrap()
            .total_work_secs,
        1500
    );
}

#[test]
fn cancel_skip_cycle_reset_and_break_stop_are_one_shot() {
    let (mut state, db) = setup(Settings::default());
    state.start();
    stop(&mut state, &db, true);
    stop(&mut state, &db, false);
    assert_eq!(state.complete(false, &db.lock().unwrap()).unwrap().len(), 2);
    assert_eq!(
        state.sequence.current_round,
        sequence::RoundType::ShortBreak
    );
    stop(&mut state, &db, true);
    assert_eq!(state.complete(false, &db.lock().unwrap()).unwrap().len(), 1);
    state.start();
    stop(&mut state, &db, true);
    assert_eq!(state.complete(true, &db.lock().unwrap()).unwrap().len(), 2);
    assert!(!state.stop_after);
    assert!(!state.stopped_after);
    let next = template(&mut state, &db, "Next", true);
    select(&mut state, &db, &next);
    stop(&mut state, &db, true);
    state.reset_kind = ResetKind::Cycle;
    assert_eq!(state.reset(&db.lock().unwrap()).unwrap().len(), 1);
    assert_eq!(state.active_id, next);
    assert_eq!(state.sequence.work_round_number, 1);
    assert_eq!(state.sequence.current_round, sequence::RoundType::Work);
    assert!(!state.started);
    assert!(!state.stop_after);
}

#[test]
fn apply_now_rejects_replaced_or_cancelled_arrangement() {
    let (mut state, db) = setup(Settings::default());
    state.start();
    let next = template(&mut state, &db, "Next", true);
    select(&mut state, &db, &next);
    let revision = state.pending_revision;
    action(&mut state, &db, PlanAction::CancelPending).unwrap();
    let request = PlanAction::ApplyNow {
        round_id: state.round_id,
        pending_revision: revision,
    };
    assert!(action(&mut state, &db, request).is_err());
}

#[test]
fn engine_handshake_orders_controls_and_stop_without_a_next_session() {
    let (mut state, db) = setup(Settings {
        time_work_secs: 60,
        ..Settings::default()
    });
    let next = template(&mut state, &db, "Concurrent selection", true);
    let (engine, events) = engine::spawn_controlled(60, Duration::from_millis(2));
    engine.send(TimerCommand::Dispatch(1));
    let mut complete = false;
    let mut commands = vec![];
    loop {
        match events.recv_timeout(Duration::from_secs(3)).unwrap() {
            TimerEvent::Dispatch { id: 1, reply } => {
                let start = apply_action(&mut state, Action::Toggle, &db).unwrap();
                // Simulate a second WebView's request arriving before Start is processed.
                engine.send(TimerCommand::Dispatch(2));
                reply.send(start).unwrap();
            }
            TimerEvent::Dispatch { id: 2, reply } => {
                assert!(state.started);
                assert!(state.session_id.is_none());
                select(&mut state, &db, &next);
                action(
                    &mut state,
                    &db,
                    PlanAction::Edit {
                        key: "auto_start_break".into(),
                        value: "true".into(),
                    },
                )
                .unwrap();
                assert_eq!(state.duration(), 60);
                stop(&mut state, &db, true);
                reply.send(vec![]).unwrap();
            }
            TimerEvent::Started { .. } => {
                assert!(state.started);
            }
            TimerEvent::Tick {
                elapsed_secs,
                total_secs,
            } => {
                assert_eq!(total_secs, state.duration());
                state
                    .record_tick(elapsed_secs, &db.lock().unwrap())
                    .unwrap();
            }
            TimerEvent::Complete { skipped } => {
                assert!(!skipped);
                commands = state.complete(false, &db.lock().unwrap()).unwrap();
                complete = true;
            }
            TimerEvent::Boundary { reply } => {
                reply.send(std::mem::take(&mut commands)).unwrap();
                if complete {
                    break;
                }
            }
            event => panic!("unexpected event: {event:?}"),
        }
    }
    assert!(events.recv_timeout(Duration::from_millis(30)).is_err());
    assert_eq!(
        db.lock()
            .unwrap()
            .query_row::<i64, _, _>("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap(),
        1
    );
    assert!(!state.started);
    assert!(!state.is_running);
    assert_eq!(state.duration(), 600);
    assert_eq!(
        db.lock()
            .unwrap()
            .query_row::<u32, _, _>("SELECT duration_secs FROM sessions", [], |r| r.get(0))
            .unwrap(),
        60
    );
    engine.send(TimerCommand::Shutdown);
}

fn category(
    state: &mut Control,
    db: &DbState,
    op: crate::db::categories::CategoryAction,
) -> Result<(), String> {
    apply_action(state, Action::Category(op), db).map(|_| ())
}
fn new_category(state: &mut Control, db: &DbState, name: &str) -> i64 {
    category(
        state,
        db,
        crate::db::categories::CategoryAction::Create { name: name.into() },
    )
    .unwrap();
    state.categories.items.last().unwrap().id
}

#[test]
fn work_category_is_locked_before_first_tick_and_breaks_only_change_next_focus() {
    use crate::db::categories::CategoryAction as C;
    let (mut state, db) = setup(Settings::default());
    let a = new_category(&mut state, &db, "A");
    let b = new_category(&mut state, &db, "B");
    category(&mut state, &db, C::Select { id: Some(a) }).unwrap();
    let plan = state.active_id.clone();
    let duration = state.duration();
    state.start();
    category(&mut state, &db, C::Select { id: Some(b) }).unwrap();
    assert_eq!(state.category_id, Some(a));
    assert!(state.snapshot().category_pending);
    assert_eq!(state.active_id, plan);
    assert_eq!(state.duration(), duration);
    state.record_tick(1, &db.lock().unwrap()).unwrap();
    let session = state.session_id;
    state.is_running = false;
    category(&mut state, &db, C::Select { id: None }).unwrap();
    state.is_running = true;
    state.record_tick(2, &db.lock().unwrap()).unwrap();
    assert_eq!(state.session_id, session);
    state.complete(false, &db.lock().unwrap()).unwrap();
    assert_eq!(
        state.sequence.current_round,
        sequence::RoundType::ShortBreak
    );
    assert_eq!(state.category_id, None);
    category(&mut state, &db, C::Select { id: Some(b) }).unwrap();
    state.record_tick(1, &db.lock().unwrap()).unwrap();
    assert_eq!(state.category_id, None);
    state.complete(true, &db.lock().unwrap()).unwrap();
    assert_eq!(state.sequence.current_round, sequence::RoundType::Work);
    assert_eq!(state.category_id, Some(b));
    state.record_tick(1, &db.lock().unwrap()).unwrap();
    let rows: Vec<(String, Option<i64>)> = db
        .lock()
        .unwrap()
        .prepare("SELECT round_type,category_id FROM sessions ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![
            ("work".into(), Some(a)),
            ("short-break".into(), None),
            ("work".into(), Some(b))
        ]
    );
}

#[test]
fn archive_start_order_preserves_legal_locked_history_and_resets_future_default() {
    use crate::db::categories::CategoryAction as C;
    for start_first in [true, false] {
        let (mut state, db) = setup(Settings::default());
        let id = new_category(&mut state, &db, "Archive race");
        category(&mut state, &db, C::Select { id: Some(id) }).unwrap();
        if start_first {
            state.start();
        }
        category(&mut state, &db, C::Archive { id }).unwrap();
        assert_eq!(state.categories.selected_id, None);
        if !start_first {
            state.start();
        }
        state.record_tick(1, &db.lock().unwrap()).unwrap();
        assert_eq!(state.category_id, if start_first { Some(id) } else { None });
        assert!(category(&mut state, &db, C::Select { id: Some(id) }).is_err());
        state.complete(false, &db.lock().unwrap()).unwrap();
        let stored: Option<i64> = db
            .lock()
            .unwrap()
            .query_row("SELECT category_id FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, if start_first { Some(id) } else { None });
        let restored = crate::db::categories::load(&db.lock().unwrap()).unwrap();
        assert_eq!(restored.selected_id, None);
        assert!(restored.items[0].archived);
        state.complete(false, &db.lock().unwrap()).unwrap();
        assert_eq!(state.sequence.current_round, sequence::RoundType::Work);
        assert_eq!(state.category_id, None);
        assert_eq!(state.category_notice_id, None);
    }
}

#[test]
fn categories_are_independent_of_plans_resets_and_statistical_scopes() {
    use crate::db::categories::{CategoryAction as C, CategoryFilter};
    let (mut state, db) = setup(Settings::default());
    let a = new_category(&mut state, &db, "A");
    let b = new_category(&mut state, &db, "B");
    category(&mut state, &db, C::Select { id: Some(a) }).unwrap();
    state.start();
    let next = template(&mut state, &db, "Long", true);
    select(&mut state, &db, &next);
    assert_eq!(state.category_id, Some(a));
    assert_eq!(state.categories.selected_id, Some(a));
    category(&mut state, &db, C::Select { id: Some(b) }).unwrap();
    let current_round = state.round_id;
    category(
        &mut state,
        &db,
        C::CancelPending {
            round_id: current_round,
        },
    )
    .unwrap();
    assert_eq!(state.categories.selected_id, Some(a));
    assert!(!state.snapshot().category_pending);
    let before = serde_json::to_value(state.snapshot()).unwrap();
    queries::get_all_time_stats_filtered(
        &db.lock().unwrap(),
        &CategoryFilter::Category { category_id: b },
    )
    .unwrap();
    assert_eq!(serde_json::to_value(state.snapshot()).unwrap(), before);
    state.reset_kind = ResetKind::Cycle;
    state.reset(&db.lock().unwrap()).unwrap();
    assert_eq!(state.categories.selected_id, Some(a));
    assert_eq!(state.category_id, None);
    assert!(!state.started);
    assert!(category(
        &mut state,
        &db,
        C::CancelPending {
            round_id: current_round
        }
    )
    .is_err());
}

#[test]
fn restoring_a_category_clears_an_obsolete_archive_notice_without_selecting_it() {
    use crate::db::categories::CategoryAction as C;
    let (mut state, db) = setup(Settings::default());
    let id = new_category(&mut state, &db, "Restore notice");
    category(&mut state, &db, C::Select { id: Some(id) }).unwrap();
    state.start();
    category(&mut state, &db, C::Archive { id }).unwrap();
    assert_eq!(state.category_notice_id, Some(id));
    category(&mut state, &db, C::Restore { id }).unwrap();
    assert_eq!(state.category_notice_id, None);
    assert_eq!(state.categories.selected_id, None);
    assert_eq!(state.category_id, Some(id));
}

#[test]
fn snapshot_exposes_only_the_existing_written_session_identity() {
    let (mut state, db) = setup(Settings::default());
    state.start();
    assert_eq!(state.snapshot().session_id, None);
    state.record_tick(1, &db.lock().unwrap()).unwrap();
    let id = state.session_id;
    assert!(id.is_some());
    assert_eq!(state.snapshot().session_id, id);
    state.is_running = false;
    assert_eq!(state.snapshot().session_id, id);
    assert!(state.snapshot().is_paused);
    state.complete(false, &db.lock().unwrap()).unwrap();
    assert_eq!(state.snapshot().session_id, None);
}

#[test]
fn integration_ninety_minutes_plan_category_stop_tray_stats_and_detail_agree() {
    use crate::db::{
        categories::{CategoryAction as C, CategoryFilter},
        session_details::{self, SessionQuery},
    };
    use crate::tray::presentation;
    let (mut state, db) = setup(Settings {
        time_work_secs: 5400,
        language: "zh".into(),
        ..Settings::default()
    });
    let code = new_category(&mut state, &db, "编程");
    let read = new_category(&mut state, &db, "阅读");
    category(&mut state, &db, C::Select { id: Some(code) }).unwrap();
    let next = template(&mut state, &db, "下轮方案", false);
    state.start();
    let old = state.round_id;
    select(&mut state, &db, &next);
    category(&mut state, &db, C::Select { id: Some(read) }).unwrap();
    stop(&mut state, &db, true);
    action(
        &mut state,
        &db,
        PlanAction::Edit {
            key: "auto_start_break".into(),
            value: "true".into(),
        },
    )
    .unwrap();
    state.record_tick(1, &db.lock().unwrap()).unwrap();
    let id = state.session_id.unwrap();
    state.is_running = false;
    let paused = presentation::build(&state.tray_view(), &db.lock().unwrap()).unwrap();
    assert!(paused.tooltip.contains("已暂停"));
    state.is_running = true;
    state.record_tick(5399, &db.lock().unwrap()).unwrap();
    assert_eq!(state.session_id, Some(id));
    let before = presentation::build(&state.tray_view(), &db.lock().unwrap()).unwrap();
    assert!(before.tooltip.contains("编程"));
    assert!(!before.tooltip.contains("阅读"));
    assert!(before.tooltip.contains("剩余 00:01"));
    let commands = state.complete(false, &db.lock().unwrap()).unwrap();
    assert!(!commands.iter().any(|c| matches!(c, TimerCommand::Start)));
    assert_eq!(
        state.sequence.current_round,
        sequence::RoundType::ShortBreak
    );
    assert_eq!(state.duration(), 300);
    assert!(!state.started);
    assert!(!state.stop_after);
    assert!(state.stopped_after);
    assert!(action(
        &mut state,
        &db,
        PlanAction::StopAfter {
            round_id: old,
            enabled: true
        }
    )
    .is_err());
    let after = presentation::build(&state.tray_view(), &db.lock().unwrap()).unwrap();
    assert!(after.tooltip.contains("短休息"));
    assert!(after.tooltip.contains("等待开始"));
    let conn = db.lock().unwrap();
    let (duration, category, completed): (u32, i64, bool) = conn
        .query_row(
            "SELECT duration_secs,category_id,completed FROM sessions WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((duration, category, completed), (5400, code, true));
    let date: String = conn
        .query_row(
            "SELECT date(started_at,'unixepoch','localtime') FROM sessions WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    let filter = CategoryFilter::Category { category_id: code };
    let detail = session_details::query(
        &conn,
        &SessionQuery {
            date,
            hour: None,
            filter: filter.clone(),
            cursor: None,
            limit: Some(50),
        },
    )
    .unwrap();
    assert_eq!(detail.summary.focus_secs, 5400);
    assert_eq!(detail.summary.recorded, 1);
    assert_eq!(detail.records[0].category_name.as_deref(), Some("编程"));
    let stats = queries::get_all_time_stats_filtered(&conn, &filter).unwrap();
    assert_eq!(stats.completed_work_sessions, 1);
    assert_eq!(stats.total_work_secs, 5400);
    drop(conn);
    state.complete(true, &db.lock().unwrap()).unwrap();
    assert!(!state.started);
    assert_eq!(state.category_id, None);
    state.start();
    assert_eq!(state.category_id, Some(read));
    assert!(presentation::build(&state.tray_view(), &db.lock().unwrap())
        .unwrap()
        .tooltip
        .contains("阅读"));
}
