use super::*;
use crate::db::{migrations, queries};

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    migrations::run(&conn).unwrap();
    conn
}
fn create(conn: &Connection, name: &str) -> i64 {
    mutate(conn, &CategoryAction::Create { name: name.into() })
        .unwrap()
        .items
        .last()
        .unwrap()
        .id
}
fn record(
    conn: &Connection,
    days: i32,
    secs: u32,
    completed: bool,
    category: Option<i64>,
    round: &str,
) {
    conn.execute(
        "INSERT INTO sessions(started_at,ended_at,round_type,duration_secs,completed,category_id)
         VALUES (strftime('%s',date('now','localtime',?1)||' 09:15:00','utc'),123,?2,?3,?4,?5)",
        params![format!("{days} days"), round, secs, completed, category],
    )
    .unwrap();
}

#[test]
fn names_archive_restore_and_default_are_persistent_without_changing_history() {
    let conn = db();
    assert!(load(&conn).unwrap().items.is_empty());
    let id = create(&conn, "  Reading  ");
    assert!(mutate(
        &conn,
        &CategoryAction::Create {
            name: " reading ".into()
        }
    )
    .is_err());
    assert!(mutate(&conn, &CategoryAction::Create { name: " ".into() }).is_err());
    assert!(mutate(
        &conn,
        &CategoryAction::Create {
            name: "字".repeat(25)
        }
    )
    .is_err());
    assert!(mutate(
        &conn,
        &CategoryAction::Create {
            name: "未分类".into()
        }
    )
    .is_err());
    mutate(&conn, &CategoryAction::Select { id: Some(id) }).unwrap();
    record(&conn, 0, 1801, true, Some(id), "work");
    let before: (i64, i64, i64, u32, bool, Option<i64>) = conn
        .query_row(
            "SELECT id,started_at,ended_at,duration_secs,completed,category_id FROM sessions",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .unwrap();
    let archived = mutate(&conn, &CategoryAction::Archive { id }).unwrap();
    assert_eq!(archived.selected_id, None);
    assert!(archived.items[0].archived);
    assert!(mutate(&conn, &CategoryAction::Select { id: Some(id) }).is_err());
    assert!(mutate(
        &conn,
        &CategoryAction::Create {
            name: "READING".into()
        }
    )
    .is_err());
    mutate(
        &conn,
        &CategoryAction::Rename {
            id,
            name: "Books".into(),
        },
    )
    .unwrap();
    let restored = mutate(&conn, &CategoryAction::Restore { id }).unwrap();
    assert_eq!(restored.items[0].id, id);
    assert_eq!(restored.items[0].name, "Books");
    assert!(!restored.items[0].archived);
    assert_eq!(restored.selected_id, None);
    let after = conn
        .query_row(
            "SELECT id,started_at,ended_at,duration_secs,completed,category_id FROM sessions",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, u32>(3)?,
                    r.get::<_, bool>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(before, after);
    mutate(&conn, &CategoryAction::Select { id: Some(id) }).unwrap();
    assert_eq!(load(&conn).unwrap().selected_id, Some(id));
    assert!(conn
        .execute("DELETE FROM categories WHERE id=?1", [id])
        .is_err());
}

#[test]
fn archive_fallback_rolls_back_with_the_category_if_persistence_fails() {
    let conn = db();
    let id = create(&conn, "Atomic");
    mutate(&conn, &CategoryAction::Select { id: Some(id) }).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_selection BEFORE INSERT ON settings WHEN NEW.key='focus_category_id' BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
    assert!(mutate(&conn, &CategoryAction::Archive { id }).is_err());
    let saved = load(&conn).unwrap();
    assert!(!saved.items[0].archived);
    assert_eq!(saved.selected_id, Some(id));
}

#[test]
fn all_uncategorized_and_category_scopes_feed_every_aggregation_consistently() {
    let conn = db();
    let a = create(&conn, "A");
    let b = create(&conn, "B");
    let empty = create(&conn, "Empty");
    for (days, secs, completed) in [(0, 900, true), (0, 600, false), (-1, 1200, true)] {
        record(&conn, days, secs, completed, None, "work");
    }
    for (days, secs, completed) in [
        (0, 1801, true),
        (0, 700, false),
        (-1, 1200, true),
        (-2, 600, true),
        (-8, 300, true),
        (-400, 400, true),
    ] {
        record(&conn, days, secs, completed, Some(a), "work");
    }
    for (days, secs, completed) in [
        (0, 600, true),
        (-1, 500, false),
        (-2, 300, true),
        (-3, 60, true),
    ] {
        record(&conn, days, secs, completed, Some(b), "work");
    }
    record(&conn, 0, 900, true, None, "short-break");
    for (scope, started, rounds, seconds, current, longest, week_rounds, week_started) in [
        (CategoryFilter::All, 13, 10, 7361, 4, 4, 8, 11),
        (CategoryFilter::Uncategorized, 3, 2, 2100, 2, 2, 2, 3),
        (
            CategoryFilter::Category { category_id: a },
            6,
            5,
            4301,
            3,
            3,
            3,
            4,
        ),
        (
            CategoryFilter::Category { category_id: b },
            4,
            3,
            960,
            1,
            2,
            3,
            4,
        ),
        (
            CategoryFilter::Category { category_id: empty },
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ),
    ] {
        let totals = queries::get_all_time_stats_filtered(&conn, &scope).unwrap();
        assert_eq!(
            (
                totals.total_work_sessions,
                totals.completed_work_sessions,
                totals.total_work_secs
            ),
            (started, rounds, seconds)
        );
        let heat = queries::get_heatmap_data_filtered(&conn, &scope).unwrap();
        assert_eq!(
            heat.iter().map(|d| u64::from(d.count)).sum::<u64>(),
            rounds as u64
        );
        assert_eq!(heat.iter().map(|d| d.focus_secs).sum::<u64>(), seconds);
        let week = queries::get_weekly_stats_filtered(&conn, &scope).unwrap();
        assert_eq!(week.iter().map(|d| d.rounds).sum::<u32>(), week_rounds);
        assert_eq!(
            week.iter().map(|d| d.started_rounds).sum::<u32>(),
            week_started
        );
        let today = queries::get_daily_stats_filtered(&conn, &scope).unwrap();
        assert_eq!(today.by_hour.iter().sum::<u32>(), today.rounds);
        assert_eq!(
            today.focus_mins,
            ((today.by_hour_focus_secs.iter().sum::<u64>() + 30) / 60) as u32
        );
        let today_key: String = conn
            .query_row("SELECT date('now','localtime')", [], |r| r.get(0))
            .unwrap();
        let day = week.iter().find(|d| d.date == today_key);
        assert_eq!(today.rounds, day.map_or(0, |d| d.rounds));
        assert_eq!(
            today.by_hour_focus_secs.iter().sum::<u64>(),
            day.map_or(0, |d| d.focus_secs)
        );
        assert_eq!(
            today.completion_rate,
            day.map(|d| d.rounds as f32 / d.started_rounds as f32)
        );
        let streak = queries::get_streak_filtered(&conn, &scope).unwrap();
        assert_eq!((streak.current, streak.longest), (current, longest));
    }
    let before = serde_json::to_value(
        queries::get_all_time_stats_filtered(&conn, &CategoryFilter::Category { category_id: a })
            .unwrap(),
    )
    .unwrap();
    mutate(&conn, &CategoryAction::Archive { id: a }).unwrap();
    let after = serde_json::to_value(
        queries::get_all_time_stats_filtered(&conn, &CategoryFilter::Category { category_id: a })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        serde_json::to_value(queries::get_all_time_stats(&conn).unwrap()).unwrap(),
        serde_json::to_value(
            queries::get_all_time_stats_filtered(&conn, &CategoryFilter::All).unwrap()
        )
        .unwrap()
    );
}

#[test]
fn scope_contract_rejects_invalid_ids_and_propagates_query_failures() {
    let conn = db();
    assert_ne!(
        serde_json::from_str::<CategoryFilter>(r#"{"kind":"all"}"#).unwrap(),
        serde_json::from_str::<CategoryFilter>(r#"{"kind":"uncategorized"}"#).unwrap()
    );
    for json in [
        r#"{"kind":"category","category_id":null}"#,
        r#"{"kind":"all","category_id":1}"#,
        r#"{"kind":"wrong"}"#,
    ] {
        assert!(serde_json::from_str::<CategoryFilter>(json).is_err());
    }
    for id in [0, -1, 999, 9_007_199_254_740_992] {
        let scope = CategoryFilter::Category { category_id: id };
        assert!(queries::get_daily_stats_filtered(&conn, &scope).is_err());
        assert!(queries::get_weekly_stats_filtered(&conn, &scope).is_err());
        assert!(queries::get_heatmap_data_filtered(&conn, &scope).is_err());
        assert!(queries::get_all_time_stats_filtered(&conn, &scope).is_err());
        assert!(queries::get_streak_filtered(&conn, &scope).is_err());
    }
    assert!(queries::insert_session_with_category(&conn, "work", 60, Some(999)).is_err());
    let id = create(&conn, "Focus");
    assert!(queries::insert_session_with_category(&conn, "short-break", 60, Some(id)).is_err());
    conn.execute_batch("DROP TABLE sessions").unwrap();
    assert!(queries::get_streak_filtered(&conn, &CategoryFilter::All).is_err());
    conn.execute_batch("DROP TABLE settings").unwrap();
    assert!(load(&conn).is_err());
}
