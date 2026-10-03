use super::*;
use crate::db::{
    categories::{self, CategoryAction},
    migrations, queries,
};
use rusqlite::params;

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    migrations::run(&conn).unwrap();
    conn
}
fn request(date: &str) -> SessionQuery {
    SessionQuery {
        date: date.into(),
        hour: None,
        filter: CategoryFilter::All,
        cursor: None,
        limit: None,
    }
}
fn insert(
    conn: &Connection,
    start: &str,
    end: Option<&str>,
    duration: u32,
    completed: bool,
    category: Option<i64>,
    round: &str,
) -> i64 {
    conn.execute(
        "INSERT INTO sessions(started_at,ended_at,duration_secs,completed,category_id,round_type)
        VALUES(strftime('%s',?1,'utc'),strftime('%s',?2,'utc'),?3,?4,?5,?6)",
        params![start, end, duration, completed, category, round],
    )
    .unwrap();
    conn.last_insert_rowid()
}

#[test]
fn detail_keeps_exact_records_null_end_and_archived_categories_without_writing_history() {
    let conn = db();
    let category = categories::mutate(
        &conn,
        &CategoryAction::Create {
            name: "Reading".into(),
        },
    )
    .unwrap()
    .items[0]
        .id;
    let first = insert(
        &conn,
        "2025-12-31 23:50:00",
        Some("2026-01-01 00:25:00"),
        1801,
        true,
        Some(category),
        "work",
    );
    let unfinished = insert(
        &conn,
        "2025-12-31 08:00:00",
        None,
        5400,
        false,
        None,
        "work",
    );
    insert(
        &conn,
        "2025-12-31 09:00:00",
        Some("2025-12-31 09:10:00"),
        1200,
        false,
        Some(category),
        "work",
    );
    insert(
        &conn,
        "2025-12-31 10:00:00",
        Some("2025-12-31 10:05:00"),
        300,
        true,
        None,
        "short-break",
    );
    insert(&conn, "2026-01-01 00:00:00", None, 600, false, None, "work");
    categories::mutate(&conn, &CategoryAction::Archive { id: category }).unwrap();
    let before = conn.total_changes();
    let page = query(&conn, &request("2025-12-31")).unwrap();
    assert_eq!(page.records.len(), 3);
    assert_eq!(
        (
            page.summary.recorded,
            page.summary.completed,
            page.summary.focus_secs
        ),
        (3, 1, 1801)
    );
    assert_eq!(page.records[0].id, unfinished);
    assert!(page.records[0].ended_at.is_none());
    let completed = page.records.iter().find(|row| row.id == first).unwrap();
    assert_eq!(completed.category_name.as_deref(), Some("Reading"));
    assert!(completed.category_archived);
    assert!(completed.ended_at.unwrap() > completed.started_at);
    assert_eq!(conn.total_changes(), before);
    let mut hour = request("2025-12-31");
    hour.hour = Some(23);
    hour.filter = CategoryFilter::Category {
        category_id: category,
    };
    assert_eq!(query(&conn, &hour).unwrap().records[0].id, first);
    hour.hour = Some(0);
    assert_eq!(query(&conn, &hour).unwrap().summary.recorded, 0);
    let mut unclassified = request("2025-12-31");
    unclassified.filter = CategoryFilter::Uncategorized;
    assert_eq!(
        query(&conn, &unclassified).unwrap().records[0].id,
        unfinished
    );
    categories::mutate(
        &conn,
        &CategoryAction::Rename {
            id: category,
            name: "Books".into(),
        },
    )
    .unwrap();
    let fresh = query(&conn, &request("2025-12-31")).unwrap();
    assert_eq!(fresh.records[2].category_name.as_deref(), Some("Books"));
    assert_eq!(fresh.records[2].started_at, completed.started_at);
}

#[test]
fn detail_summary_matches_existing_daily_hourly_weekly_and_heatmap_seconds() {
    let conn = db();
    let today: String = conn
        .query_row("SELECT date('now','localtime')", [], |r| r.get(0))
        .unwrap();
    let category = categories::mutate(
        &conn,
        &CategoryAction::Create {
            name: "Work".into(),
        },
    )
    .unwrap()
    .items[0]
        .id;
    for (hour, duration, completed, id) in [
        (8, 1801, true, Some(category)),
        (8, 5399, true, None),
        (8, 3000, false, Some(category)),
        (9, 61, true, Some(category)),
    ] {
        insert(
            &conn,
            &format!("{today} {hour:02}:20:00"),
            None,
            duration,
            completed,
            id,
            "work",
        );
    }
    for filter in [
        CategoryFilter::All,
        CategoryFilter::Uncategorized,
        CategoryFilter::Category {
            category_id: category,
        },
    ] {
        let daily = queries::get_daily_stats_filtered(&conn, &filter).unwrap();
        let mut q = request(&today);
        q.filter = filter.clone();
        let page = query(&conn, &q).unwrap();
        assert_eq!(page.summary.completed, u64::from(daily.rounds));
        assert_eq!(
            page.summary.focus_secs,
            daily.by_hour_focus_secs.iter().sum::<u64>()
        );
        let week = queries::get_weekly_stats_filtered(&conn, &filter).unwrap();
        let day = week.iter().find(|d| d.date == today).unwrap();
        assert_eq!(page.summary.recorded, u64::from(day.started_rounds));
        assert_eq!(page.summary.focus_secs, day.focus_secs);
        assert_eq!(
            page.summary.focus_secs,
            queries::get_heatmap_data_filtered(&conn, &filter).unwrap()[0].focus_secs
        );
        q.hour = Some(8);
        let page = query(&conn, &q).unwrap();
        assert_eq!(page.summary.completed, u64::from(daily.by_hour[8]));
        assert_eq!(page.summary.focus_secs, daily.by_hour_focus_secs[8]);
    }
}

#[test]
fn cursor_pages_are_stable_for_identical_timestamps_and_ignore_new_rows_until_refresh() {
    let conn = db();
    let mut ids = Vec::new();
    for n in 0..123 {
        ids.push(insert(
            &conn,
            if n % 2 == 0 {
                "2026-01-01 09:00:00"
            } else {
                "2026-01-01 08:00:00"
            },
            None,
            60 + n,
            n % 3 == 0,
            None,
            "work",
        ));
    }
    let mut q = request("2026-01-01");
    let first = query(&conn, &q).unwrap();
    assert_eq!(first.records.len(), 50);
    assert_eq!(first.summary.recorded, 123);
    let mut got: Vec<_> = first.records.iter().map(|r| (r.started_at, r.id)).collect();
    q.cursor = first.next_cursor;
    let new = insert(&conn, "2026-01-01 07:00:00", None, 99, true, None, "work");
    while q.cursor.is_some() {
        let page = query(&conn, &q).unwrap();
        assert_eq!(page.summary.recorded, 123);
        got.extend(page.records.iter().map(|r| (r.started_at, r.id)));
        q.cursor = page.next_cursor;
    }
    assert_eq!(got.len(), 123);
    assert!(got.windows(2).all(|w| w[0] < w[1]));
    ids.sort();
    let mut fetched: Vec<_> = got.iter().map(|r| r.1).collect();
    fetched.sort();
    assert_eq!(fetched, ids);
    let fresh = query(&conn, &request("2026-01-01")).unwrap();
    assert_eq!(fresh.summary.recorded, 124);
    assert_eq!(fresh.records[0].id, new);
}

#[test]
fn validates_calendar_hours_limits_and_cursor_scope_and_returns_errors_not_empty_pages() {
    let conn = db();
    for date in [
        "2025-02-29",
        "2026-04-31",
        "2026-00-01",
        "2026-1-01",
        "0000-01-01",
        "2026-01-01' OR 1=1",
    ] {
        assert!(query(&conn, &request(date)).is_err(), "{date}");
    }
    for date in ["2024-02-29", "2000-02-29", "0001-01-01", "9999-12-31"] {
        assert!(valid_date(date));
    }
    let mut q = request("2026-01-01");
    q.hour = Some(24);
    assert!(query(&conn, &q).is_err());
    q.hour = None;
    for limit in [0, 101, u32::MAX] {
        q.limit = Some(limit);
        assert!(query(&conn, &q).is_err());
    }
    q.limit = Some(1);
    insert(&conn, "2026-01-01 08:00:00", None, 60, true, None, "work");
    insert(&conn, "2026-01-01 08:00:00", None, 61, false, None, "work");
    q.cursor = query(&conn, &q).unwrap().next_cursor;
    q.hour = Some(8);
    assert!(query(&conn, &q).is_err());
    q.hour = None;
    q.cursor.as_mut().unwrap().started_at += 1;
    assert!(query(&conn, &q).is_err());
    conn.execute_batch("DROP TABLE sessions").unwrap();
    assert!(query(&conn, &request("2026-01-01")).is_err());
}

#[test]
fn date_and_hour_follow_sqlite_local_start_time_including_day_and_year_edges() {
    let conn = db();
    for (start, date, hour) in [
        ("2025-12-31 23:59:59", "2025-12-31", 23),
        ("2026-01-01 00:00:00", "2026-01-01", 0),
        ("2024-02-29 08:59:59", "2024-02-29", 8),
        ("2024-02-29 09:00:00", "2024-02-29", 9),
    ] {
        let id = insert(&conn, start, None, 61, true, None, "work");
        let mut q = request(date);
        q.hour = Some(hour);
        assert!(query(&conn, &q).unwrap().records.iter().any(|r| r.id == id));
        q.hour = Some((hour + 1) % 24);
        assert!(!query(&conn, &q).unwrap().records.iter().any(|r| r.id == id));
    }
}
