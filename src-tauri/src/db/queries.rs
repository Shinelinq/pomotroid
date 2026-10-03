use rusqlite::{params, Connection, Result};
use serde::Serialize;

// ---------------------------------------------------------------------------
// Session CRUD (DATA-03)
// ---------------------------------------------------------------------------

/// Inserts a new session row when a round begins.
/// Returns the row ID so it can be passed to `complete_session` later.
pub fn insert_session(
    conn: &Connection,
    round_type: &str,
    duration_secs: u32,
) -> Result<i64> {
    let started_at = unix_now();
    conn.execute(
        "INSERT INTO sessions (started_at, round_type, duration_secs, completed)
         VALUES (?1, ?2, ?3, 0)",
        params![started_at, round_type, duration_secs],
    )?;
    let id = conn.last_insert_rowid();
    log::debug!("[db] session started: id={id} type={round_type} duration={duration_secs}s");
    Ok(id)
}

/// Updates a session when the round ends (by completion or skip).
pub fn complete_session(
    conn: &Connection,
    session_id: i64,
    completed: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET ended_at = ?1, completed = ?2 WHERE id = ?3",
        params![unix_now(), completed as i64, session_id],
    )?;
    log::debug!("[db] session ended: id={session_id} completed={completed}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Stats queries
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct SessionStats {
    pub total_work_sessions: i64,
    pub completed_work_sessions: i64,
    /// Sum of duration_secs for all *completed* work sessions.
    pub total_work_secs: u64,
}

pub fn get_all_time_stats(conn: &Connection) -> Result<SessionStats> {
    let total_work_sessions: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE round_type = 'work'",
        [],
        |r| r.get(0),
    )?;

    let completed_work_sessions: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE round_type = 'work' AND completed = 1",
        [],
        |r| r.get(0),
    )?;

    let total_work_secs = conn.query_row(
        "SELECT COALESCE(SUM(duration_secs), 0)
         FROM sessions WHERE round_type = 'work' AND completed = 1",
        [],
        |r| read_focus_secs(r, 0),
    )?;

    Ok(SessionStats {
        total_work_sessions,
        completed_work_sessions,
        total_work_secs,
    })
}

/// SQLite sums are signed integers; reject invalid values instead of wrapping.
fn read_focus_secs(row: &rusqlite::Row<'_>, index: usize) -> Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

// ---------------------------------------------------------------------------
// Detailed stats queries (DATA-04)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct DailyStats {
    pub rounds: u32,
    pub focus_mins: u32,
    /// None when no work sessions were started today (avoids 0/0).
    pub completion_rate: Option<f32>,
    /// Completed work rounds per hour of the day (index 0 = midnight).
    pub by_hour: Vec<u32>,
    /// Exact completed-work duration attributed to each session's local starting hour.
    pub by_hour_focus_secs: Vec<u64>,
}

#[derive(Debug, Serialize)]
pub struct DayStat {
    /// Local calendar date in "YYYY-MM-DD" format.
    pub date: String,
    pub rounds: u32,
    /// All recorded work sessions, including incomplete sessions.
    pub started_rounds: u32,
    /// Sum of duration_secs for completed work sessions on this local date.
    pub focus_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct HeatmapEntry {
    /// Local calendar date in "YYYY-MM-DD" format.
    pub date: String,
    pub count: u32,
    /// Sum of duration_secs for completed work sessions on this local date.
    pub focus_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct StreakInfo {
    pub current: u32,
    pub longest: u32,
}

/// Completed work rounds and focus time for today (local calendar date).
pub fn get_daily_stats(conn: &Connection) -> Result<DailyStats> {
    let today: String = conn.query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))?;

    get_daily_stats_for_date(conn, &today)
}

fn get_daily_stats_for_date(conn: &Connection, today: &str) -> Result<DailyStats> {
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sessions
         WHERE round_type = 'work'
         AND date(started_at, 'unixepoch', 'localtime') = ?1",
        [&today],
        |r| r.get(0),
    )?;

    let completed: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sessions
         WHERE round_type = 'work' AND completed = 1
         AND date(started_at, 'unixepoch', 'localtime') = ?1",
        [&today],
        |r| r.get(0),
    )?;

    let focus_secs: i64 = conn.query_row(
        "SELECT COALESCE(SUM(duration_secs), 0) FROM sessions
         WHERE round_type = 'work' AND completed = 1
         AND date(started_at, 'unixepoch', 'localtime') = ?1",
        [&today],
        |r| r.get(0),
    )?;

    let mut by_hour = vec![0u32; 24];
    let mut by_hour_focus_secs = vec![0u64; 24];
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', datetime(started_at, 'unixepoch', 'localtime')) AS INTEGER) as h,
                COUNT(*) as cnt,
                COALESCE(SUM(duration_secs), 0) as focus_secs
         FROM sessions
         WHERE round_type = 'work' AND completed = 1
         AND date(started_at, 'unixepoch', 'localtime') = ?1
         GROUP BY h",
    )?;
    let rows = stmt.query_map([today], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, u32>(1)?,
            read_focus_secs(r, 2)?,
        ))
    })?;
    for row in rows {
        let (h, cnt, seconds) = row?;
        if (0..24).contains(&h) {
            by_hour[h as usize] = cnt;
            by_hour_focus_secs[h as usize] = seconds;
        } else {
            return Err(rusqlite::Error::IntegralValueOutOfRange(0, h));
        }
    }

    Ok(DailyStats {
        rounds: completed as u32,
        focus_mins: ((focus_secs + 30) / 60) as u32,
        completion_rate: if total > 0 {
            Some(completed as f32 / total as f32)
        } else {
            None
        },
        by_hour,
        by_hour_focus_secs,
    })
}

/// Recorded work sessions, completed rounds and focus seconds for the last 7 local days.
pub fn get_weekly_stats(conn: &Connection) -> Result<Vec<DayStat>> {
    let today: String = conn.query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))?;
    get_weekly_stats_for_date(conn, &today)
}

/// The reference date is local, so calendar arithmetic also handles non-24-hour days.
fn get_weekly_stats_for_date(conn: &Connection, today: &str) -> Result<Vec<DayStat>> {
    let mut stmt = conn.prepare(
        "SELECT date(started_at, 'unixepoch', 'localtime') as day,
                SUM(CASE WHEN completed = 1 THEN 1 ELSE 0 END) as rounds,
                COUNT(*) as started_rounds,
                SUM(CASE WHEN completed = 1 THEN duration_secs ELSE 0 END) as focus_secs
         FROM sessions
         WHERE round_type = 'work'
         AND date(started_at, 'unixepoch', 'localtime') >= date(?1, '-6 days')
         AND date(started_at, 'unixepoch', 'localtime') < date(?1, '+1 day')
         GROUP BY day
         ORDER BY day",
    )?;
    let rows = stmt.query_map([today], |r| {
        Ok(DayStat {
            date: r.get(0)?,
            rounds: r.get(1)?,
            started_rounds: r.get(2)?,
            focus_secs: read_focus_secs(r, 3)?,
        })
    })?;
    rows.collect()
}

/// Completed work rounds per local calendar day, all time (no date limit).
/// The frontend slices this into per-year views for navigation.
pub fn get_heatmap_data(conn: &Connection) -> Result<Vec<HeatmapEntry>> {
    let mut stmt = conn.prepare(
        "SELECT date(started_at, 'unixepoch', 'localtime') as day,
                COUNT(*) as cnt,
                SUM(duration_secs) as focus_secs
         FROM sessions
         WHERE round_type = 'work' AND completed = 1
         GROUP BY day
         ORDER BY day",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(HeatmapEntry {
            date: r.get(0)?,
            count: r.get(1)?,
            focus_secs: read_focus_secs(r, 2)?,
        })
    })?;
    rows.collect()
}

/// Current and longest work-session streaks (consecutive local calendar days).
/// A streak stays active until midnight: if yesterday had sessions but today does not,
/// the streak is still counted as current.
pub fn get_streak(conn: &Connection) -> Result<StreakInfo> {
    let today: String = conn.query_row(
        "SELECT date('now', 'localtime')",
        [],
        |r| r.get(0),
    )?;

    let mut stmt = conn.prepare(
        "SELECT date(started_at, 'unixepoch', 'localtime') as day
         FROM sessions
         WHERE round_type = 'work' AND completed = 1
         GROUP BY day
         ORDER BY day",
    )?;
    let days: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .flatten()
        .collect();

    Ok(compute_streak(&days, &today))
}

// ---------------------------------------------------------------------------
// Streak helpers
// ---------------------------------------------------------------------------

/// Convert a "YYYY-MM-DD" string to a day number for arithmetic comparison.
/// Uses the proleptic Gregorian calendar; absolute value is arbitrary — only
/// differences between dates matter.
fn date_to_day_num(s: &str) -> Option<i32> {
    let mut parts = s.splitn(3, '-');
    let y: i32 = parts.next()?.parse().ok()?;
    let m: i32 = parts.next()?.parse().ok()?;
    let d: i32 = parts.next()?.parse().ok()?;
    let y = if m <= 2 { y - 1 } else { y };
    let m = if m <= 2 { m + 12 } else { m };
    Some(y * 365 + y / 4 - y / 100 + y / 400 + (153 * m - 457) / 5 + d)
}

pub fn compute_streak(days: &[String], today: &str) -> StreakInfo {
    let nums: Vec<i32> = days.iter().filter_map(|s| date_to_day_num(s)).collect();
    if nums.is_empty() {
        return StreakInfo { current: 0, longest: 0 };
    }

    let today_n = match date_to_day_num(today) {
        Some(n) => n,
        None => return StreakInfo { current: 0, longest: 0 },
    };

    // Current streak — alive if most recent session day is today or yesterday.
    let last = *nums.last().unwrap();
    let current = if last == today_n || last == today_n - 1 {
        let mut count = 0u32;
        let mut expected = last;
        for &n in nums.iter().rev() {
            if n == expected {
                count += 1;
                expected -= 1;
            } else {
                break;
            }
        }
        count
    } else {
        0
    };

    // Longest streak.
    let mut longest = 1u32;
    let mut run = 1u32;
    for i in 1..nums.len() {
        if nums[i] == nums[i - 1] + 1 {
            run += 1;
            if run > longest { longest = run; }
        } else {
            run = 1;
        }
    }

    StreakInfo { current, longest }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run(&conn).unwrap();
        conn
    }

    fn record_session(
        conn: &Connection,
        local_start: &str,
        round_type: &str,
        duration_secs: i64,
        completed: bool,
    ) {
        conn.execute(
            "INSERT INTO sessions (started_at, ended_at, round_type, duration_secs, completed)
             VALUES (CAST(strftime('%s', ?1, 'utc') AS INTEGER),
                     CAST(strftime('%s', ?1, 'utc') AS INTEGER) + ?3, ?2, ?3, ?4)",
            params![local_start, round_type, duration_secs, completed],
        )
        .unwrap();
    }

    fn session_rows(conn: &Connection) -> Vec<Vec<rusqlite::types::Value>> {
        conn.prepare("SELECT * FROM sessions ORDER BY id")
            .unwrap()
            .query_map([], |row| (0..6).map(|i| row.get(i)).collect())
            .unwrap()
            .collect::<Result<_>>()
            .unwrap()
    }

    #[test]
    fn daily_hours_empty_are_zero_filled() {
        let conn = setup();
        let today = get_daily_stats_for_date(&conn, "2024-03-15").unwrap();
        assert_eq!(today.by_hour, vec![0; 24]);
        assert_eq!(today.by_hour_focus_secs, vec![0; 24]);
    }

    #[test]
    fn daily_hours_use_start_time_and_actual_completed_durations() {
        let conn = setup();
        for (start, round, seconds, complete) in [
            ("2024-03-15 08:00:00", "work", 1500, true),
            ("2024-03-15 08:20:00", "work", 2700, true),
            ("2024-03-15 08:50:00", "work", 5437, true),
            ("2024-03-15 08:10:00", "work", 1200, false),
            ("2024-03-15 08:15:00", "short-break", 300, true),
            ("2024-03-15 08:30:00", "long-break", 900, true),
            ("2024-03-15 23:50:00", "work", 2400, true),
            ("2024-03-14 23:50:00", "work", 3600, true),
            ("2024-03-16 00:00:00", "work", 60, true),
        ] {
            record_session(&conn, start, round, seconds, complete);
        }
        conn.execute(
            "UPDATE sessions SET ended_at = ended_at + 3600 WHERE id = 1",
            [],
        )
        .unwrap();
        let original = session_rows(&conn);
        let today = get_daily_stats_for_date(&conn, "2024-03-15").unwrap();
        assert_eq!(today.by_hour.len(), 24);
        assert_eq!(today.by_hour_focus_secs.len(), 24);
        assert_eq!((today.by_hour[8], today.by_hour_focus_secs[8]), (3, 9637));
        assert_eq!((today.by_hour[23], today.by_hour_focus_secs[23]), (1, 2400));
        assert_eq!(today.by_hour[9], 0);
        assert_eq!(today.by_hour_focus_secs[9], 0);
        assert_eq!(today.by_hour.iter().sum::<u32>(), today.rounds);
        let raw_day_seconds: i64 = conn
            .query_row(
                "SELECT SUM(duration_secs) FROM sessions WHERE round_type='work' AND completed=1
             AND date(started_at, 'unixepoch', 'localtime') = ?1",
                ["2024-03-15"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            today.by_hour_focus_secs.iter().sum::<u64>(),
            u64::try_from(raw_day_seconds).unwrap()
        );
        assert_eq!(raw_day_seconds, 12037);
        assert_ne!(
            u64::from(today.focus_mins) * 60,
            today.by_hour_focus_secs.iter().sum::<u64>()
        );
        assert_eq!(session_rows(&conn), original);
        let previous = get_daily_stats_for_date(&conn, "2024-03-14").unwrap();
        assert_eq!(previous.by_hour_focus_secs[23], 3600);
    }

    #[test]
    fn daily_hour_seconds_remain_wide_and_propagate_invalid_storage() {
        let conn = setup();
        record_session(
            &conn,
            "2024-03-15 08:00:00",
            "work",
            i64::from(u32::MAX),
            true,
        );
        record_session(&conn, "2024-03-15 08:30:00", "work", 3600, true);
        let today = get_daily_stats_for_date(&conn, "2024-03-15").unwrap();
        assert_eq!(today.by_hour_focus_secs[8], u64::from(u32::MAX) + 3600);
        let malformed = setup();
        malformed
            .execute(
                "INSERT INTO sessions (started_at, round_type, duration_secs, completed)
          VALUES (CAST(strftime('%s', '2024-03-15 08:00:00', 'utc') AS INTEGER), 'work', 1.5, 1)",
                [],
            )
            .unwrap();
        assert!(get_daily_stats_for_date(&malformed, "2024-03-15").is_err());
    }

    #[test]
    fn mixed_sessions_use_recorded_durations_without_modifying_history() {
        let conn = setup();
        record_session(&conn, "2024-03-15 08:00:00", "work", 1500, true);
        record_session(&conn, "2024-03-15 09:00:00", "work", 2700, true);
        record_session(&conn, "2024-03-15 10:00:00", "work", 1200, false);
        record_session(&conn, "2024-03-15 11:00:00", "short-break", 300, true);
        // Pauses can make wall-clock duration longer than recorded focus duration.
        conn.execute(
            "UPDATE sessions SET ended_at = ended_at + 3600 WHERE id = 1",
            [],
        )
        .unwrap();
        let before = session_rows(&conn);

        let week = get_weekly_stats_for_date(&conn, "2024-03-15").unwrap();
        assert_eq!(week.len(), 1);
        assert_eq!(week[0].date, "2024-03-15");
        assert_eq!(week[0].rounds, 2);
        assert_eq!(week[0].started_rounds, 3);
        assert_eq!(week[0].focus_secs, 4200);

        let entries = get_heatmap_data(&conn).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].date, "2024-03-15");
        assert_eq!(entries[0].count, 2);
        assert_eq!(entries[0].focus_secs, 4200);

        let totals = get_all_time_stats(&conn).unwrap();
        assert_eq!(totals.total_work_sessions, 3);
        assert_eq!(totals.completed_work_sessions, 2);
        assert_eq!(totals.total_work_secs, 4200);
        assert_eq!(session_rows(&conn), before);
    }

    #[test]
    fn weekly_stats_keep_incomplete_only_days_and_exclude_break_only_days() {
        let conn = setup();
        record_session(&conn, "2024-03-13 08:00:00", "work", 1200, false);
        record_session(&conn, "2024-03-13 09:00:00", "work", 1500, false);
        record_session(&conn, "2024-03-14 08:00:00", "short-break", 300, true);
        record_session(&conn, "2024-03-14 09:00:00", "long-break", 900, true);

        let week = get_weekly_stats_for_date(&conn, "2024-03-15").unwrap();
        assert_eq!(week.len(), 1);
        assert_eq!(week[0].date, "2024-03-13");
        assert_eq!(week[0].rounds, 0);
        assert_eq!(week[0].started_rounds, 2);
        assert_eq!(week[0].focus_secs, 0);
        assert!(get_heatmap_data(&conn).unwrap().is_empty());
        let totals = get_all_time_stats(&conn).unwrap();
        assert_eq!(totals.total_work_sessions, 2);
        assert_eq!(totals.completed_work_sessions, 0);
        assert_eq!(totals.total_work_secs, 0);
    }

    #[test]
    fn weekly_stats_use_complete_local_days_and_exclude_future_dates() {
        let conn = setup();
        // Deliberately insert out of order. Sessions crossing midnight belong to their start date.
        for (date, duration) in [
            ("2024-03-16 00:00:00", 9999),
            ("2024-03-15 23:59:59", 50),
            ("2024-03-09 00:00:00", 10),
            ("2024-03-08 23:59:59", 9999),
            ("2024-03-10 00:00:00", 30),
            ("2024-03-09 23:59:59", 20),
            ("2024-03-15 00:00:00", 40),
            ("2024-03-20 12:00:00", 9999),
        ] {
            record_session(&conn, date, "work", duration, true);
        }

        let week = get_weekly_stats_for_date(&conn, "2024-03-15").unwrap();
        let days: Vec<_> = week
            .iter()
            .map(|d| (d.date.as_str(), d.rounds, d.started_rounds, d.focus_secs))
            .collect();
        assert_eq!(
            days,
            vec![
                ("2024-03-09", 2, 2, 30),
                ("2024-03-10", 1, 1, 30),
                ("2024-03-15", 2, 2, 90)
            ]
        );
    }

    #[test]
    fn heatmap_keeps_all_years_and_matches_lifetime_totals() {
        let conn = setup();
        for (date, round_type, duration, completed) in [
            ("2025-01-01 00:00:00", "work", 3600, true),
            ("2024-12-31 23:50:00", "work", 1500, true),
            ("2023-12-31 09:00:00", "work", 2700, true),
            ("2024-12-31 12:00:00", "work", 900, true),
            ("2024-12-31 13:00:00", "work", 1200, false),
            ("2025-01-02 08:00:00", "work", 1500, false),
            ("2025-01-02 09:00:00", "long-break", 900, true),
        ] {
            record_session(&conn, date, round_type, duration, completed);
        }

        let entries = get_heatmap_data(&conn).unwrap();
        let days: Vec<_> = entries
            .iter()
            .map(|d| (d.date.as_str(), d.count, d.focus_secs))
            .collect();
        assert_eq!(
            days,
            vec![
                ("2023-12-31", 1, 2700),
                ("2024-12-31", 2, 2400),
                ("2025-01-01", 1, 3600)
            ]
        );
        let totals = get_all_time_stats(&conn).unwrap();
        assert_eq!(totals.total_work_sessions, 6);
        assert_eq!(totals.completed_work_sessions, 4);
        assert_eq!(totals.total_work_secs, 8700);
        assert_eq!(entries.iter().map(|d| u64::from(d.count)).sum::<u64>(), 4);
        assert_eq!(
            entries.iter().map(|d| d.focus_secs).sum::<u64>(),
            totals.total_work_secs
        );
    }

    #[test]
    fn focus_seconds_preserve_remainders_and_exceed_u32_without_truncation() {
        for durations in [
            vec![2700],
            vec![3600],
            vec![4200],
            vec![6300],
            vec![1123500],
            vec![3599, 38],
            vec![i64::from(u32::MAX), 123],
            vec![i64::from(u32::MAX) + 456],
        ] {
            let conn = setup();
            for &duration in &durations {
                record_session(&conn, "2024-03-15 08:00:00", "work", duration, true);
            }
            let expected = u64::try_from(durations.iter().sum::<i64>()).unwrap();
            let week = get_weekly_stats_for_date(&conn, "2024-03-15").unwrap();
            let entries = get_heatmap_data(&conn).unwrap();
            assert_eq!(week[0].focus_secs, expected);
            assert_eq!(entries[0].focus_secs, expected);
            assert_eq!(get_all_time_stats(&conn).unwrap().total_work_secs, expected);
        }
    }

    #[test]
    fn stats_queries_propagate_sql_errors() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(get_weekly_stats(&conn).is_err());
        assert!(get_heatmap_data(&conn).is_err());
        assert!(get_all_time_stats(&conn).is_err());
    }

    #[test]
    fn stats_queries_reject_negative_and_non_integer_seconds() {
        for invalid in [
            rusqlite::types::Value::Integer(-1),
            rusqlite::types::Value::Real(1.5),
        ] {
            let conn = setup();
            // Simulate malformed storage only in this in-memory test database.
            conn.execute_batch("PRAGMA ignore_check_constraints = ON")
                .unwrap();
            conn.execute(
                "INSERT INTO sessions (started_at, round_type, duration_secs, completed)
                 VALUES (CAST(strftime('%s', '2024-03-15 08:00:00', 'utc') AS INTEGER),
                         'work', ?1, 1)",
                [invalid],
            )
            .unwrap();
            assert!(get_weekly_stats_for_date(&conn, "2024-03-15").is_err());
            assert!(get_heatmap_data(&conn).is_err());
            assert!(get_all_time_stats(&conn).is_err());
        }
    }

    #[test]
    fn stats_queries_propagate_sqlite_sum_overflow() {
        let conn = setup();
        conn.execute_batch(
            "INSERT INTO sessions (started_at, round_type, duration_secs, completed)
             VALUES (0, 'work', 9223372036854775807, 1), (0, 'work', 1, 1)",
        )
        .unwrap();
        let day: String = conn
            .query_row("SELECT date(0, 'unixepoch', 'localtime')", [], |r| r.get(0))
            .unwrap();
        assert!(get_weekly_stats_for_date(&conn, &day).is_err());
        assert!(get_heatmap_data(&conn).is_err());
        assert!(get_all_time_stats(&conn).is_err());
    }

    #[test]
    fn insert_and_complete_session() {
        let conn = setup();
        let id = insert_session(&conn, "work", 1500).unwrap();
        assert!(id > 0);

        complete_session(&conn, id, true).unwrap();

        let completed: i64 = conn
            .query_row(
                "SELECT completed FROM sessions WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(completed, 1);
    }

    #[test]
    fn stats_empty_db() {
        let conn = setup();
        let stats = get_all_time_stats(&conn).unwrap();
        assert_eq!(stats.total_work_sessions, 0);
        assert_eq!(stats.completed_work_sessions, 0);
        assert_eq!(stats.total_work_secs, 0);
    }

    #[test]
    fn compute_streak_empty() {
        let info = compute_streak(&[], "2024-03-15");
        assert_eq!(info.current, 0);
        assert_eq!(info.longest, 0);
    }

    #[test]
    fn compute_streak_active_today() {
        let days = vec!["2024-03-13".to_string(), "2024-03-14".to_string(), "2024-03-15".to_string()];
        let info = compute_streak(&days, "2024-03-15");
        assert_eq!(info.current, 3);
        assert_eq!(info.longest, 3);
    }

    #[test]
    fn compute_streak_active_until_midnight() {
        // Yesterday had sessions, today does not — streak still live.
        let days = vec!["2024-03-13".to_string(), "2024-03-14".to_string()];
        let info = compute_streak(&days, "2024-03-15");
        assert_eq!(info.current, 2);
    }

    #[test]
    fn compute_streak_broken() {
        // Last session was 2 days ago — streak is broken.
        let days = vec!["2024-03-12".to_string(), "2024-03-13".to_string()];
        let info = compute_streak(&days, "2024-03-15");
        assert_eq!(info.current, 0);
    }

    #[test]
    fn compute_streak_longest_across_break() {
        let days = vec![
            "2024-03-01".to_string(), "2024-03-02".to_string(), "2024-03-03".to_string(),
            "2024-03-10".to_string(), "2024-03-11".to_string(),
        ];
        let info = compute_streak(&days, "2024-03-11");
        assert_eq!(info.current, 2);
        assert_eq!(info.longest, 3);
    }

    #[test]
    fn get_daily_stats_empty() {
        let conn = setup();
        let stats = get_daily_stats(&conn).unwrap();
        assert_eq!(stats.rounds, 0);
        assert_eq!(stats.focus_mins, 0);
        assert!(stats.completion_rate.is_none());
        assert_eq!(stats.by_hour.len(), 24);
    }

    #[test]
    fn get_weekly_stats_empty() {
        let conn = setup();
        let stats = get_weekly_stats(&conn).unwrap();
        assert!(stats.is_empty());
    }

    #[test]
    fn get_heatmap_data_empty() {
        let conn = setup();
        let entries = get_heatmap_data(&conn).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn focus_mins_rounds_to_nearest_minute() {
        let conn = setup();

        // 339 s = 5:39 → rounds up to 6 min (remainder 39 ≥ 30).
        let id1 = insert_session(&conn, "work", 339).unwrap();
        complete_session(&conn, id1, true).unwrap();
        let stats = get_daily_stats(&conn).unwrap();
        assert_eq!(stats.focus_mins, 6, "339 s should round to 6 min");

        // Reset and test round-down: 324 s = 5:24 → rounds down to 5 min (remainder 24 < 30).
        let conn2 = setup();
        let id2 = insert_session(&conn2, "work", 324).unwrap();
        complete_session(&conn2, id2, true).unwrap();
        let stats2 = get_daily_stats(&conn2).unwrap();
        assert_eq!(stats2.focus_mins, 5, "324 s should round to 5 min");

        // Exact minute boundary: 1500 s = 25:00 → stays 25 min.
        let conn3 = setup();
        let id3 = insert_session(&conn3, "work", 1500).unwrap();
        complete_session(&conn3, id3, true).unwrap();
        let stats3 = get_daily_stats(&conn3).unwrap();
        assert_eq!(stats3.focus_mins, 25, "1500 s should be exactly 25 min");
    }

    #[test]
    fn stats_counts_correctly() {
        let conn = setup();

        let id1 = insert_session(&conn, "work", 1500).unwrap();
        complete_session(&conn, id1, true).unwrap();

        let id2 = insert_session(&conn, "work", 1500).unwrap();
        complete_session(&conn, id2, false).unwrap(); // skipped

        let _id3 = insert_session(&conn, "short-break", 300).unwrap();

        let stats = get_all_time_stats(&conn).unwrap();
        assert_eq!(stats.total_work_sessions, 2);
        assert_eq!(stats.completed_work_sessions, 1);
        assert_eq!(stats.total_work_secs, 1500);
    }
}
