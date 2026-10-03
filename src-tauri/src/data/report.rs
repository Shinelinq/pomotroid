use super::{err, Result};
use crate::db::{categories::CategoryFilter, session_details::valid_date};
use rusqlite::{named_params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Daily,
    Sessions,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportScope {
    pub kind: ReportKind,
    pub start: Option<String>,
    pub end: Option<String>,
    pub filter: CategoryFilter,
    pub hour: Option<u8>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReportInfo {
    pub scope: ReportScope,
    pub rows: usize,
    pub focus_secs: i64,
    pub filename: String,
    pub timezone: Option<String>,
}
pub struct Report {
    pub info: ReportInfo,
    pub bytes: Vec<u8>,
}

fn validate(conn: &Connection, s: &ReportScope) -> Result<()> {
    s.filter.validate(conn).map_err(err)?;
    if s.start.is_some() != s.end.is_some()
        || s.start.as_ref().is_some_and(|v| !valid_date(v))
        || s.end.as_ref().is_some_and(|v| !valid_date(v))
        || s.start > s.end
        || s.hour.is_some_and(|h| {
            h > 23 || s.start.is_none() || s.start != s.end || s.kind != ReportKind::Sessions
        })
    {
        return Err("report_invalid_scope".into());
    }
    Ok(())
}
// Inclusive local calendar dates -> half-open UTC interval. Never add 86400 seconds.
fn bounds(conn: &Connection, s: &ReportScope) -> Result<(Option<i64>, Option<i64>)> {
    conn.query_row(
        "SELECT CAST(strftime('%s',?1||' 00:00:00','utc') AS INTEGER),
        CAST(strftime('%s',date(?2,'+1 day')||' 00:00:00','utc') AS INTEGER)",
        rusqlite::params![s.start, s.end],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .map_err(err)
}
fn condition(filter: &CategoryFilter) -> String {
    format!("s.round_type='work' AND (:since IS NULL OR s.started_at>=:since)
        AND (:until IS NULL OR s.started_at<:until)
        AND (:start IS NULL OR date(s.started_at,'unixepoch','localtime')>=:start)
        AND (:end IS NULL OR date(s.started_at,'unixepoch','localtime')<=:end)
        AND (:hour IS NULL OR CAST(strftime('%H',s.started_at,'unixepoch','localtime') AS INTEGER)=:hour)
        AND ({})",filter.condition())
}
/// Excel-oriented protection. Quoting alone does not prevent formulas.
pub fn protect(value: &str) -> String {
    let first = value
        .trim_start_matches(|c: char| {
            c.is_whitespace()
                || c.is_control()
                || matches!(c, '\u{feff}' | '\u{200b}' | '\u{200e}' | '\u{200f}')
        })
        .chars()
        .next();
    if first.is_some_and(|c| "=+-@＝＋－＠﹦﹢﹣".contains(c)) {
        format!("\t{value}")
    } else {
        value.into()
    }
}
fn local_time(date: String, offset: i64) -> String {
    format!(
        "{date}{}{:02}:{:02}",
        if offset < 0 { '-' } else { '+' },
        offset.abs() / 3600,
        offset.abs() % 3600 / 60
    )
}
pub fn build(conn: &Connection, scope: ReportScope, locale: &str) -> Result<Report> {
    validate(conn, &scope)?;
    let tx = conn.unchecked_transaction().map_err(err)?;
    let (since, until) = bounds(&tx, &scope)?;
    let clause = condition(&scope.filter);
    let params = named_params! {":since":since,":until":until,":start":scope.start,":end":scope.end,":hour":scope.hour,":category_id":scope.filter.id()};
    let msg = |key| super::message(locale, key);
    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .quote_style(csv::QuoteStyle::NonNumeric)
        .from_writer(vec![0xef, 0xbb, 0xbf]);
    let keys = match scope.kind {
        ReportKind::Daily => [
            "csv_date",
            "csv_category",
            "csv_category_id",
            "csv_recorded",
            "csv_completed",
            "csv_focus_secs",
            "csv_focus_mins",
            "csv_rate",
        ],
        ReportKind::Sessions => [
            "csv_session_id",
            "csv_start",
            "csv_end",
            "csv_category",
            "csv_category_id",
            "csv_status",
            "csv_planned",
            "csv_focus_secs",
        ],
    };
    writer.write_record(keys.map(msg)).map_err(err)?;
    let (mut rows, mut focus_secs) = (0, 0i64);
    let uncategorized = msg("category_uncategorized");
    if scope.kind == ReportKind::Daily {
        let mut stmt=tx.prepare(&format!("SELECT date(s.started_at,'unixepoch','localtime'),c.name,c.stable_id,COUNT(*),SUM(s.completed),
            SUM(CASE WHEN s.completed=1 THEN s.duration_secs ELSE 0 END) FROM sessions s LEFT JOIN categories c ON c.id=s.category_id
            WHERE {clause} GROUP BY date(s.started_at,'unixepoch','localtime'),s.category_id ORDER BY 1,COALESCE(c.stable_id,'')")).map_err(err)?;
        let mut query = stmt.query(params).map_err(err)?;
        while let Some(r) = query.next().map_err(err)? {
            let date: String = r.get(0).map_err(err)?;
            let category: Option<String> = r.get(1).map_err(err)?;
            let id: Option<String> = r.get(2).map_err(err)?;
            let recorded: i64 = r.get(3).map_err(err)?;
            let completed: i64 = r.get(4).map_err(err)?;
            let secs: i64 = r.get(5).map_err(err)?;
            focus_secs = focus_secs.checked_add(secs).ok_or("report_overflow")?;
            writer
                .write_record([
                    date,
                    protect(category.as_deref().unwrap_or(&uncategorized)),
                    id.unwrap_or_default(),
                    recorded.to_string(),
                    completed.to_string(),
                    secs.to_string(),
                    format!("{:.6}", secs as f64 / 60.0),
                    if recorded == 0 {
                        String::new()
                    } else {
                        format!("{:.2}", 100.0 * completed as f64 / recorded as f64)
                    },
                ])
                .map_err(err)?;
            rows += 1;
        }
    } else {
        let mut stmt=tx.prepare(&format!("SELECT s.stable_id,
            strftime('%Y-%m-%dT%H:%M:%S',s.started_at,'unixepoch','localtime'),
            strftime('%Y-%m-%dT%H:%M:%S',s.ended_at,'unixepoch','localtime'),
            c.name,c.stable_id,s.completed,s.duration_secs,
            CAST(strftime('%s',s.started_at,'unixepoch','localtime') AS INTEGER)-s.started_at,
            CAST(strftime('%s',s.ended_at,'unixepoch','localtime') AS INTEGER)-s.ended_at
            FROM sessions s LEFT JOIN categories c ON c.id=s.category_id WHERE {clause} ORDER BY s.started_at,s.stable_id")).map_err(err)?;
        let mut query = stmt.query(params).map_err(err)?;
        let completed = msg("detail_completed");
        let incomplete = msg("detail_incomplete");
        while let Some(r) = query.next().map_err(err)? {
            let id: String = r.get(0).map_err(err)?;
            let start: String = r.get(1).map_err(err)?;
            let end: Option<String> = r.get(2).map_err(err)?;
            let start_offset: i64 = r.get(7).map_err(err)?;
            let end_offset: Option<i64> = r.get(8).map_err(err)?;
            let category: Option<String> = r.get(3).map_err(err)?;
            let category_id: Option<String> = r.get(4).map_err(err)?;
            let done: bool = r.get(5).map_err(err)?;
            let duration: i64 = r.get(6).map_err(err)?;
            let secs = if done { duration } else { 0 };
            focus_secs = focus_secs.checked_add(secs).ok_or("report_overflow")?;
            writer
                .write_record([
                    id,
                    local_time(start, start_offset),
                    end.zip(end_offset)
                        .map(|(date, offset)| local_time(date, offset))
                        .unwrap_or_default(),
                    protect(category.as_deref().unwrap_or(&uncategorized)),
                    category_id.unwrap_or_default(),
                    if done {
                        completed.clone()
                    } else {
                        incomplete.clone()
                    },
                    duration.to_string(),
                    secs.to_string(),
                ])
                .map_err(err)?;
            rows += 1;
        }
    }
    let category = match &scope.filter {
        CategoryFilter::All => msg("category_all"),
        CategoryFilter::Uncategorized => uncategorized,
        CategoryFilter::Category { category_id } => tx
            .query_row(
                "SELECT name FROM categories WHERE id=?1",
                [category_id],
                |r| r.get(0),
            )
            .map_err(err)?,
    };
    let filename = format!(
        "Pomotroid-{}-{}-{}-{}{}.csv",
        if scope.kind == ReportKind::Daily {
            "daily"
        } else {
            "sessions"
        },
        scope.start.as_deref().unwrap_or("all"),
        scope.end.as_deref().unwrap_or("all"),
        super::files::safe_filename(&category),
        scope
            .hour
            .map(|h| format!("-{h:02}-{:02}", h + 1))
            .unwrap_or_default()
    );
    tx.commit().map_err(err)?;
    Ok(Report {
        info: ReportInfo {
            scope,
            rows,
            focus_secs,
            filename,
            timezone: iana_time_zone::get_timezone().ok(),
        },
        bytes: writer.into_inner().map_err(err)?,
    })
}
#[derive(Debug, Clone, Serialize)]
pub struct DistributionRow {
    pub category_id: Option<i64>,
    pub name: Option<String>,
    pub archived: bool,
    pub completed: u64,
    pub focus_secs: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct Distribution {
    pub rows: Vec<DistributionRow>,
    pub total_focus_secs: u64,
}
pub fn distribution(conn: &Connection, start: &str, end: &str) -> Result<Distribution> {
    let scope = ReportScope {
        kind: ReportKind::Daily,
        start: Some(start.into()),
        end: Some(end.into()),
        filter: CategoryFilter::All,
        hour: None,
    };
    validate(conn, &scope)?;
    let (since, until) = bounds(conn, &scope)?;
    let mut stmt=conn.prepare(&format!("SELECT s.category_id,c.name,COALESCE(c.archived,0),COUNT(*),SUM(s.duration_secs)
        FROM sessions s LEFT JOIN categories c ON c.id=s.category_id WHERE {} AND s.completed=1
        GROUP BY s.category_id ORDER BY SUM(s.duration_secs) DESC,COALESCE(c.name,'') COLLATE BINARY,COALESCE(c.stable_id,'')",condition(&scope.filter))).map_err(err)?;
    let rows=stmt.query_map(named_params! {":since":since,":until":until,":start":start,":end":end,":hour":Option::<u8>::None,":category_id":Option::<i64>::None},
        |r|Ok(DistributionRow {category_id:r.get(0)?,name:r.get(1)?,archived:r.get(2)?,completed:r.get::<_,i64>(3)? as u64,focus_secs:r.get::<_,i64>(4)? as u64})).map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(err)?;
    let total_focus_secs = rows
        .iter()
        .try_fold(0u64, |sum, r| sum.checked_add(r.focus_secs))
        .ok_or("report_overflow")?;
    Ok(Distribution {
        rows,
        total_focus_secs,
    })
}

#[cfg(test)]
mod tests;
