use super::*;
use rusqlite::params;
fn db() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    crate::db::migrations::run(&c).unwrap();
    c
}
fn add(c: &Connection, date: &str, secs: i64, done: bool, category: Option<i64>) {
    c.execute("INSERT INTO sessions(stable_id,started_at,round_type,duration_secs,completed,category_id) VALUES (?1,strftime('%s',?2,'utc'),'work',?3,?4,?5)",
        params![uuid::Uuid::new_v4().to_string(),date,secs,done,category]).unwrap();
}
fn scope() -> ReportScope {
    ReportScope {
        kind: ReportKind::Daily,
        start: Some("2024-02-29".into()),
        end: Some("2024-02-29".into()),
        filter: CategoryFilter::All,
        hour: None,
    }
}
#[test]
fn daily_detail_distribution_agree_and_do_not_truncate_pages() {
    let c = db();
    for i in 0..125 {
        add(&c, "2024-02-29 08:00:00", 61, i % 2 == 0, None);
    }
    add(&c, "2024-03-01 00:00:00", 100, true, None);
    add(&c, "2024-02-28 23:59:59", 100, true, None);
    let daily = build(&c, scope(), "zh").unwrap();
    assert_eq!(daily.info.rows, 1);
    assert_eq!(daily.info.focus_secs, 63 * 61);
    let detail = build(
        &c,
        ReportScope {
            kind: ReportKind::Sessions,
            hour: Some(8),
            ..scope()
        },
        "en",
    )
    .unwrap();
    assert_eq!(detail.info.rows, 125);
    assert_eq!(detail.info.focus_secs, daily.info.focus_secs);
    let d = distribution(&c, "2024-02-29", "2024-02-29").unwrap();
    assert_eq!(d.total_focus_secs, daily.info.focus_secs as u64);
    let reader = csv::Reader::from_reader(&daily.bytes[3..]);
    let row = reader.into_records().next().unwrap().unwrap();
    assert_eq!(&row[5], "3843");
    assert_eq!(&row[6], "64.050000");
    assert_eq!(&row[7], "50.40");
    assert!(String::from_utf8(detail.bytes).unwrap().contains(":00"));
}
#[test]
fn incomplete_only_and_empty_ranges() {
    let c = db();
    add(&c, "2024-02-29 00:00:00", 999, false, None);
    let r = build(&c, scope(), "en").unwrap();
    assert_eq!(r.info.rows, 1);
    assert_eq!(r.info.focus_secs, 0);
    assert_eq!(
        distribution(&c, "2024-02-29", "2024-02-29")
            .unwrap()
            .total_focus_secs,
        0
    );
    let r = build(
        &c,
        ReportScope {
            start: Some("2025-01-01".into()),
            end: Some("2025-12-31".into()),
            ..scope()
        },
        "en",
    )
    .unwrap();
    assert_eq!(r.info.rows, 0);
    assert_eq!(csv::Reader::from_reader(&r.bytes[3..]).records().count(), 0);
}
#[test]
fn excel_formula_examples_unicode_and_csv_roundtrip() {
    for input in [
        "=1+1",
        " +SUM(A1)",
        "\r\n@SUM(1)",
        "\u{feff}－1",
        "\u{200b}＝cmd",
        "\t-2",
    ] {
        assert_eq!(protect(input), format!("\t{input}"));
    }
    assert_eq!(
        protect("中文,\"阅读\"\r\n第二行"),
        "中文,\"阅读\"\r\n第二行"
    );
    let c = db();
    let name = " =1,\"2\"\n中文";
    c.execute(
        "INSERT INTO categories(name,name_key,stable_id) VALUES (?1,'test',?2)",
        params![name, uuid::Uuid::new_v4().to_string()],
    )
    .unwrap();
    add(&c, "2024-02-29 08:00:00", 60, true, Some(1));
    let r = build(&c, scope(), "zh").unwrap();
    assert_eq!(&r.bytes[..3], &[0xef, 0xbb, 0xbf]);
    let row = csv::Reader::from_reader(&r.bytes[3..])
        .records()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(&row[1], format!("\t{name}"));
    assert_eq!(&row[5], "60");
}
#[test]
fn distribution_includes_archived_uncategorized_and_is_sorted() {
    let c = db();
    for n in 0..6 {
        c.execute(
            "INSERT INTO categories(name,name_key,archived,stable_id) VALUES ('same',?1,?2,?3)",
            params![n.to_string(), n == 1, uuid::Uuid::new_v4().to_string()],
        )
        .unwrap();
    }
    for (id, secs) in [(Some(1), 21600), (Some(2), 10800), (None, 3600)] {
        add(&c, "2024-02-29 09:00:00", secs, true, id);
    }
    let d = distribution(&c, "2024-02-29", "2024-02-29").unwrap();
    assert_eq!(d.total_focus_secs, 36000);
    assert_eq!(d.rows.len(), 3);
    assert_eq!(
        d.rows
            .iter()
            .map(|r| r.focus_secs * 100 / d.total_focus_secs)
            .collect::<Vec<_>>(),
        vec![60, 30, 10]
    );
    assert!(d.rows[1].archived);
    for id in 3..=6 {
        add(&c, "2024-02-29 09:00:00", 1, true, Some(id));
    }
    assert_eq!(
        distribution(&c, "2024-02-29", "2024-02-29")
            .unwrap()
            .rows
            .len(),
        7
    );
}
#[test]
fn local_calendar_year_and_hour_validation() {
    let c = db();
    add(&c, "2023-12-31 23:59:59", 1, true, None);
    add(&c, "2024-01-01 00:00:00", 2, true, None);
    add(&c, "2024-12-31 23:59:59", 4, true, None);
    add(&c, "2025-01-01 00:00:00", 8, true, None);
    assert_eq!(
        distribution(&c, "2024-01-01", "2024-12-31")
            .unwrap()
            .total_focus_secs,
        6
    );
    assert!(build(
        &c,
        ReportScope {
            hour: Some(8),
            ..scope()
        },
        "en"
    )
    .is_err());
    assert!(build(
        &c,
        ReportScope {
            start: Some("2023-02-29".into()),
            ..scope()
        },
        "en"
    )
    .is_err());
    for date in ["2024-03-10", "2024-11-03", "2024-02-29"] {
        let s = ReportScope {
            start: Some(date.into()),
            end: Some(date.into()),
            ..scope()
        };
        let (a, b) = bounds(&c, &s).unwrap();
        let expected: (i64, i64) = c
            .query_row(
                "SELECT strftime('%s',?1,'utc')+0,strftime('%s',date(?1,'+1 day'),'utc')+0",
                [date],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((a.unwrap(), b.unwrap()), expected);
    }
}
