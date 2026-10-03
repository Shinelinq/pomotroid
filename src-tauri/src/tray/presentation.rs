use crate::{settings::Settings, timer::TimerSnapshot};
use rusqlite::Connection;
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Clone)]
pub struct TrayView {
    pub timer: TimerSnapshot,
    pub settings: Settings,
    pub category_name: Option<String>,
    pub next_category_name: Option<String>,
    pub pending_plan: Option<(String, bool)>,
}

#[derive(Clone, Default)]
pub struct Presentation {
    pub tooltip: String,
    pub mini_lines: Vec<String>,
    pub locale: String,
}

fn catalogs() -> &'static Vec<(&'static str, Value)> {
    static CATALOGS: OnceLock<Vec<(&str, Value)>> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        [
            ("en", include_str!("../../../src/messages/en.json")),
            ("zh", include_str!("../../../src/messages/zh.json")),
            ("de", include_str!("../../../src/messages/de.json")),
            ("es", include_str!("../../../src/messages/es.json")),
            ("fr", include_str!("../../../src/messages/fr.json")),
            ("ja", include_str!("../../../src/messages/ja.json")),
            ("pt", include_str!("../../../src/messages/pt.json")),
            ("tr", include_str!("../../../src/messages/tr.json")),
        ]
        .into_iter()
        .map(|(locale, json)| (locale, serde_json::from_str(json).expect("message catalog")))
        .collect()
    })
}
pub fn text(locale: &str, key: &str, args: &[(&str, String)]) -> String {
    let books = catalogs();
    let mut value = books
        .iter()
        .find(|(id, _)| *id == locale)
        .and_then(|(_, v)| v[key].as_str())
        .or_else(|| books[0].1[key].as_str())
        .unwrap_or(key)
        .to_string();
    for (key, replacement) in args {
        value = value.replace(&format!("{{{key}}}"), replacement);
    }
    value
}
pub fn locale(language: &str) -> String {
    let language = if language == "auto" {
        system_locale()
    } else {
        language.to_string()
    };
    let base = language
        .split(['-', '_'])
        .next()
        .unwrap_or("en")
        .to_lowercase();
    if catalogs().iter().any(|(name, _)| *name == base) {
        base
    } else {
        "en".into()
    }
}
#[cfg(target_os = "windows")]
fn system_locale() -> String {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultLocaleName(buffer: *mut u16, length: i32) -> i32;
    }
    let mut buffer = [0u16; 85];
    // SAFETY: the API receives a valid UTF-16 buffer and its exact capacity.
    let count = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
    if count > 1 {
        String::from_utf16_lossy(&buffer[..count as usize - 1])
    } else {
        "en".into()
    }
}
#[cfg(not(target_os = "windows"))]
fn system_locale() -> String {
    std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_else(|_| "en".into())
}

pub fn remaining(timer: &TimerSnapshot) -> u32 {
    timer.total_secs.saturating_sub(timer.elapsed_secs)
}
pub fn minutes(timer: &TimerSnapshot) -> u32 {
    remaining(timer).div_ceil(60)
}
pub fn minute_label(timer: &TimerSnapshot) -> String {
    let n = minutes(timer);
    if n > 99 {
        "99+".into()
    } else {
        n.to_string()
    }
}
fn duration(secs: u32) -> String {
    let h = secs / 3600;
    let m = secs % 3600 / 60;
    let s = secs % 60;
    let mut parts = Vec::new();
    if h > 0 {
        parts.push(format!("{h}h"));
    }
    if m > 0 {
        parts.push(format!("{m}m"));
    }
    if s > 0 {
        parts.push(format!("{s}s"));
    }
    if parts.is_empty() {
        "0m".into()
    } else {
        parts.join(" ")
    }
}
fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.into()
    } else if max == 0 {
        String::new()
    } else {
        format!("{}…", value.chars().take(max - 1).collect::<String>())
    }
}

pub fn build(view: &TrayView, conn: &Connection) -> Result<Presentation, String> {
    let locale = locale(&view.settings.language);
    let timer = &view.timer;
    let round = text(
        &locale,
        match timer.round_type.as_str() {
            "short-break" => "round_label_short_break",
            "long-break" => "round_label_long_break",
            _ => "round_label_work",
        },
        &[],
    );
    let category = view
        .category_name
        .clone()
        .unwrap_or_else(|| text(&locale, "category_uncategorized", &[]));
    let state = if timer.is_paused {
        Some(text(&locale, "detail_paused", &[]))
    } else if !timer.has_started {
        Some(text(&locale, "round_waiting", &[]))
    } else {
        None
    };
    let eta = if timer.is_running {
        let now = (timer.captured_at_ms / 1000) as i64;
        let end = now + i64::from(remaining(timer));
        let (day,next,ends,time,date):(String,String,String,String,String)=conn.query_row(
            "SELECT date(?1,'unixepoch','localtime'),date(?1,'unixepoch','localtime','+1 day'),date(?2,'unixepoch','localtime'),strftime('%H:%M',?2,'unixepoch','localtime'),strftime('%m/%d',?2,'unixepoch','localtime')",
            [now,end],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(|e|e.to_string())?;
        Some(text(
            &locale,
            if ends == day {
                "tray_eta"
            } else if ends == next {
                "tray_eta_tomorrow"
            } else {
                "tray_eta_date"
            },
            &[("time", time), ("date", date)],
        ))
    } else {
        None
    };
    let time = if !timer.has_started {
        text(
            &locale,
            "detail_planned",
            &[("duration", duration(timer.total_secs))],
        )
    } else {
        let seconds = remaining(timer);
        let mut line = text(
            &locale,
            "tray_remaining",
            &[("time", format!("{:02}:{:02}", seconds / 60, seconds % 60))],
        );
        if let Some(eta) = &eta {
            line.push_str(" · ");
            line.push_str(eta);
        }
        line
    };
    let header = |max: usize| {
        let mut line = round.clone();
        if timer.round_type == "work" {
            let name = truncate(&category, max);
            if !name.is_empty() {
                line.push_str(" · ");
                line.push_str(&name);
            }
        }
        if let Some(state) = &state {
            line.push_str(" · ");
            line.push_str(state);
        }
        line
    };
    let stop = timer
        .stop_after_round
        .then(|| text(&locale, "round_stop_arranged", &[]));
    let mut max_name = 24;
    let tooltip = loop {
        let mut lines = vec![header(max_name), time.clone()];
        if let Some(stop) = &stop {
            lines.push(stop.clone());
        }
        let value = lines.join("\n");
        if value.encode_utf16().count() <= 127 || max_name == 0 {
            break value;
        }
        max_name -= 1;
    };
    let mut mini_lines = vec![header(16)];
    if let Some(eta) = eta {
        mini_lines.push(eta);
    }
    if let Some((name, initial)) = &view.pending_plan {
        let name = if *initial {
            text(&locale, "plan_current", &[])
        } else {
            truncate(name, 16)
        };
        mini_lines.push(text(&locale, "plan_pending", &[("name", name)]));
    }
    if timer.category_pending || timer.round_type != "work" {
        let name = truncate(
            &view
                .next_category_name
                .clone()
                .unwrap_or_else(|| text(&locale, "category_uncategorized", &[])),
            16,
        );
        mini_lines.push(text(
            &locale,
            if timer.category_pending {
                "category_pending"
            } else {
                "category_next_focus"
            },
            &[("name", name)],
        ));
    }
    Ok(Presentation {
        tooltip,
        mini_lines,
        locale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> TrayView {
        TrayView {
            timer: TimerSnapshot {
                round_type: "work".into(),
                total_secs: 5400,
                ..Default::default()
            },
            settings: Settings {
                language: "zh".into(),
                ..Default::default()
            },
            category_name: Some("编程".into()),
            next_category_name: Some("阅读".into()),
            pending_plan: None,
        }
    }
    #[test]
    fn minute_boundaries_and_idle_plan_are_exact() {
        let mut v = view();
        for (seconds, label) in [
            (0, "0"),
            (1, "1"),
            (59, "1"),
            (60, "1"),
            (61, "2"),
            (5940, "99"),
            (5941, "99+"),
            (7200, "99+"),
        ] {
            v.timer.total_secs = seconds;
            assert_eq!(minute_label(&v.timer), label);
        }
        v.timer.total_secs = 5400;
        let p = build(&v, &Connection::open_in_memory().unwrap()).unwrap();
        assert!(p.tooltip.contains("等待开始"));
        assert!(p.tooltip.contains("计划 1h 30m"));
        assert!(!p.tooltip.contains("预计"));
    }
    #[test]
    fn tooltip_preserves_locked_category_pause_stop_and_local_dates() {
        let conn = Connection::open_in_memory().unwrap();
        let mut v = view();
        v.timer.has_started = true;
        v.timer.is_running = true;
        v.timer.stop_after_round = true;
        v.timer.category_pending = true;
        v.timer.captured_at_ms = conn
            .query_row::<i64, _, _>(
                "SELECT unixepoch('2026-12-31 23:30:00','utc')*1000",
                [],
                |r| r.get(0),
            )
            .unwrap() as u64;
        let p = build(&v, &conn).unwrap();
        assert!(p.tooltip.contains("专注 · 编程"));
        assert!(p.tooltip.contains("剩余 90:00"));
        assert!(p.tooltip.contains("预计明日 01:00 结束"));
        assert!(!p.tooltip.contains("阅读"));
        assert!(p.mini_lines.iter().any(|s| s.contains("阅读")));
        assert!(p.tooltip.contains("本轮完成后停止"));
        v.timer.is_running = false;
        v.timer.is_paused = true;
        let p = build(&v, &conn).unwrap();
        assert!(p.tooltip.contains("已暂停"));
        assert!(!p.tooltip.contains("预计"));
        v.timer.is_running = true;
        v.timer.is_paused = false;
        v.timer.total_secs = 172800;
        assert!(build(&v, &conn).unwrap().tooltip.contains("01/02 23:30"));
        v.category_name = Some("长名称😀".repeat(20));
        v.settings.language = "en".into();
        let p = build(&v, &conn).unwrap();
        assert!(p.tooltip.encode_utf16().count() <= 127);
        assert!(p.tooltip.contains("Remaining"));
        assert!(p.tooltip.contains("Stop"));
        v.timer.round_type = "short-break".into();
        assert!(!build(&v, &conn).unwrap().tooltip.contains("长名称"));
    }
}
