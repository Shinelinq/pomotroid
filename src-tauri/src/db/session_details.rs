use super::categories::CategoryFilter;
use rusqlite::{named_params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionCursor {
    pub date: String,
    pub hour: Option<u8>,
    pub filter: CategoryFilter,
    pub started_at: i64,
    pub id: i64,
    /// New rows belong to the next refresh, even if the system clock moved back.
    pub max_id: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionQuery {
    pub date: String,
    #[serde(default)]
    pub hour: Option<u8>,
    #[serde(default)]
    pub filter: CategoryFilter,
    #[serde(default)]
    pub cursor: Option<SessionCursor>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRecord {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub duration_secs: u64,
    pub completed: bool,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub category_archived: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub recorded: u64,
    pub completed: u64,
    pub focus_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionPage {
    pub records: Vec<SessionRecord>,
    pub summary: SessionSummary,
    pub next_cursor: Option<SessionCursor>,
}

fn unsigned(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

/// Calendar validation is explicit: SQLite accepts some overflowing day values.
pub fn valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return false;
    }
    let year: u32 = value[0..4].parse().unwrap();
    let month: u32 = value[5..7].parse().unwrap();
    let day: u32 = value[8..10].parse().unwrap();
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=days).contains(&day)
}

pub fn query(conn: &Connection, input: &SessionQuery) -> Result<SessionPage, String> {
    if !valid_date(&input.date) {
        return Err("detail_invalid_date".into());
    }
    if input.hour.is_some_and(|hour| hour > 23) {
        return Err("detail_invalid_hour".into());
    }
    let limit = input.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err("detail_invalid_limit".into());
    }
    input.filter.validate(conn).map_err(|e| e.to_string())?;
    if let Some(cursor) = &input.cursor {
        if cursor.date != input.date
            || cursor.hour != input.hour
            || cursor.filter != input.filter
            || cursor.id <= 0
            || cursor.id > cursor.max_id
            || cursor.max_id > 9_007_199_254_740_991
        {
            return Err("detail_invalid_cursor".into());
        }
    }

    // A read transaction keeps the ceiling, summary and page mutually consistent.
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let max_id = if let Some(cursor) = &input.cursor {
        cursor.max_id
    } else {
        tx.query_row("SELECT COALESCE(MAX(id),0) FROM sessions", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|e| e.to_string())?
    };
    let (since, until): (i64, Option<i64>) = tx
        .query_row(
            "SELECT CAST(strftime('%s',?1||' 00:00:00','utc') AS INTEGER),
                CAST(strftime('%s',date(?1,'+1 day')||' 00:00:00','utc') AS INTEGER)",
            [&input.date],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    // Static category clauses only. Values, dates, hours and cursor keys are bound.
    // The date predicate preserves the statistics definition; range bounds let
    // the existing started_at/category indexes narrow the scan.
    let scope = format!("s.round_type='work' AND s.id<=:max_id
        AND s.started_at>=:since AND (:until IS NULL OR s.started_at<:until)
        AND date(s.started_at,'unixepoch','localtime')=:date
        AND (:hour IS NULL OR CAST(strftime('%H',s.started_at,'unixepoch','localtime') AS INTEGER)=:hour)
        AND ({})", input.filter.condition());
    let scope_params = named_params! {
        ":max_id": max_id, ":since": since, ":until": until,
        ":date": input.date, ":hour": input.hour, ":category_id": input.filter.id(),
    };
    if let Some(cursor) = &input.cursor {
        let valid = tx.query_row(
            &format!("SELECT 1 FROM sessions s WHERE {scope} AND s.id=:id AND s.started_at=:started"),
            named_params! { ":max_id": max_id, ":since": since, ":until": until, ":date": input.date,
                ":hour": input.hour, ":category_id": input.filter.id(), ":id":cursor.id, ":started":cursor.started_at },
            |_| Ok(()),
        ).optional().map_err(|e| e.to_string())?;
        if valid.is_none() {
            return Err("detail_invalid_cursor".into());
        }
    }
    let summary = tx
        .query_row(
            &format!(
                "SELECT COUNT(*), COALESCE(SUM(CASE WHEN s.completed=1 THEN 1 ELSE 0 END),0),
            COALESCE(SUM(CASE WHEN s.completed=1 THEN s.duration_secs ELSE 0 END),0)
            FROM sessions s WHERE {scope}"
            ),
            scope_params,
            |r| {
                Ok(SessionSummary {
                    recorded: unsigned(r, 0)?,
                    completed: unsigned(r, 1)?,
                    focus_secs: unsigned(r, 2)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;
    let mut records = {
        let mut stmt = tx
            .prepare(&format!(
                "SELECT s.id,s.started_at,s.ended_at,s.duration_secs,s.completed,
                s.category_id,c.name,COALESCE(c.archived,0)
            FROM sessions s LEFT JOIN categories c ON c.id=s.category_id
            WHERE {scope} AND (:after_time IS NULL OR s.started_at>:after_time
                OR (s.started_at=:after_time AND s.id>:after_id))
            ORDER BY s.started_at ASC,s.id ASC LIMIT :limit"
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                named_params! {
                    ":max_id":max_id, ":since":since, ":until":until, ":date":input.date,
                    ":hour":input.hour, ":category_id":input.filter.id(),
                    ":after_time":input.cursor.as_ref().map(|c|c.started_at),
                    ":after_id":input.cursor.as_ref().map(|c|c.id), ":limit":limit+1,
                },
                |r| {
                    Ok(SessionRecord {
                        id: r.get(0)?,
                        started_at: r.get(1)?,
                        ended_at: r.get(2)?,
                        duration_secs: unsigned(r, 3)?,
                        completed: r.get(4)?,
                        category_id: r.get(5)?,
                        category_name: r.get(6)?,
                        category_archived: r.get(7)?,
                    })
                },
            )
            .map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?
    };
    let more = records.len() > limit as usize;
    records.truncate(limit as usize);
    let next_cursor = if more {
        records.last().map(|last| SessionCursor {
            date: input.date.clone(),
            hour: input.hour,
            filter: input.filter.clone(),
            started_at: last.started_at,
            id: last.id,
            max_id,
        })
    } else {
        None
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(SessionPage {
        records,
        summary,
        next_cursor,
    })
}

#[cfg(test)]
mod tests;
