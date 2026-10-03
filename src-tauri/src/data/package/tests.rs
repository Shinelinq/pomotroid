use super::*;
fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    crate::db::migrations::run(&conn).unwrap();
    settings::seed_defaults(&conn).unwrap();
    PlanBook::load(&conn, &settings::load(&conn).unwrap()).unwrap();
    conn
}
fn fixture() -> (Connection, Package) {
    let conn = db();
    categories::mutate(
        &conn,
        &categories::CategoryAction::Create {
            name: "阅读".into(),
        },
    )
    .unwrap();
    conn.execute("UPDATE categories SET archived=1", [])
        .unwrap();
    for (kind, done, category, duration) in [
        ("work", true, Some(1), 7201),
        ("work", false, None, 6000),
        ("short-break", true, None, 30),
        ("long-break", false, None, 900),
    ] {
        conn.execute("INSERT INTO sessions(stable_id,started_at,ended_at,round_type,duration_secs,completed,category_id) VALUES (?1,1700000000,?2,?3,?4,?5,?6)",
            params![uuid::Uuid::new_v4().to_string(),if done {Some(1700000010i64)} else {None},kind,duration,done,category]).unwrap();
    }
    let p = snapshot(&conn, true, true).unwrap();
    (conn, p)
}
fn options() -> Options {
    Options {
        history: true,
        profiles: true,
        preferences: false,
    }
}
fn import(conn: &Connection, p: &Package) -> ImportPlan {
    let tx = conn.unchecked_transaction().unwrap();
    let plan = plan(&tx, p, &options(), "zh").unwrap();
    apply(&tx, p, &plan).unwrap();
    tx.commit().unwrap();
    plan
}
#[test]
fn roundtrip_overlap_and_third_database_keep_identities() {
    let (_, mut p) = fixture();
    validate(&mut p).unwrap();
    let target = db();
    assert_eq!(import(&target, &p).summary.sessions.added, 4);
    let again = import(&target, &p);
    assert_eq!(again.summary.sessions.existing, 4);
    assert_eq!(again.summary.sessions.added, 0);
    p.sessions.pop();
    assert_eq!(import(&target, &p).summary.sessions.existing, 3);
    let exported = snapshot(&target, true, true).unwrap();
    let third = db();
    import(&third, &exported);
    assert_eq!(
        snapshot(&third, true, true).unwrap().sessions,
        exported.sessions
    );
    assert_eq!(
        target
            .query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM sessions WHERE category_id IS NOT NULL",
                [],
                |r| r.get(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(
        crate::db::queries::get_all_time_stats(&target)
            .unwrap()
            .total_work_secs,
        7201
    );
}
#[test]
fn rename_mapping_and_user_edits_are_distinct() {
    let (_, p) = fixture();
    let conn = db();
    categories::mutate(
        &conn,
        &categories::CategoryAction::Create {
            name: "阅读".into(),
        },
    )
    .unwrap();
    let first = import(&conn, &p);
    assert!(first.renames.iter().any(|r| r.kind == "category"));
    let again = import(&conn, &p);
    assert_eq!(again.summary.categories.existing, 1);
    assert_eq!(again.summary.categories.conflicts, 0);
    assert!(again.renames.is_empty());
    conn.execute(
        "UPDATE categories SET name='我的新名称',name_key='new' WHERE stable_id=?1",
        [&p.categories[0].stable_id],
    )
    .unwrap();
    let changed = import(&conn, &p);
    assert_eq!(changed.summary.categories.conflicts, 1);
    assert_eq!(
        conn.query_row::<String, _, _>(
            "SELECT name FROM categories WHERE stable_id=?1",
            [&p.categories[0].stable_id],
            |r| r.get(0)
        )
        .unwrap(),
        "我的新名称"
    );
}
#[test]
fn session_conflicts_preserve_local_and_category_relationship() {
    let (_, mut p) = fixture();
    let conn = db();
    import(&conn, &p);
    p.sessions[0].completed = !p.sessions[0].completed;
    p.categories[0].archived = false;
    let id = uuid::Uuid::new_v4().to_string();
    let mut extra = p.sessions[0].clone();
    extra.stable_id = id.clone();
    extra.category_stable_id = Some(p.categories[0].stable_id.clone());
    p.sessions.push(extra);
    let result = import(&conn, &p);
    assert_eq!(result.summary.sessions.conflicts, 1);
    assert_eq!(result.summary.categories.conflicts, 1);
    assert_eq!(result.summary.sessions.added, 1);
    assert!(conn.query_row::<bool,_,_>("SELECT c.archived FROM sessions s JOIN categories c ON s.category_id=c.id WHERE s.stable_id=?1",[id],|r|r.get(0)).unwrap());
}
#[test]
fn duplicates_corruption_references_versions_and_limits() {
    let (_, mut p) = fixture();
    p.sessions.push(p.sessions[0].clone());
    assert_eq!(validate(&mut p).unwrap(), 1);
    assert_eq!(p.sessions.len(), 4);
    p.sessions.push(p.sessions[0].clone());
    p.sessions.last_mut().unwrap().duration_secs += 1;
    assert_eq!(validate(&mut p).unwrap_err(), "data_internal_conflict");
    p.sessions.pop();
    p.sessions[0].category_stable_id = Some(uuid::Uuid::new_v4().to_string());
    assert_eq!(validate(&mut p).unwrap_err(), "data_reference");
    assert_eq!(decode(b"broken").unwrap_err(), "data_corrupt");
    assert_eq!(
        decode(br#"{"format":"pomotroid-personal-data","format_version":2}"#).unwrap_err(),
        "data_version"
    );
    assert_eq!(decode(br#"{"format":"other"}"#).unwrap_err(), "data_format");
    assert!(decode(&vec![b' '; MAX_BYTES as usize + 1]).is_err());
    assert!(decode(&[vec![b'['; 20], vec![b']'; 20]].concat()).is_err());
}
#[test]
fn late_failure_rolls_back_every_entity_and_preferences() {
    let (_, p) = fixture();
    let conn = db();
    let before = snapshot(&conn, true, true).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_import BEFORE INSERT ON sessions BEGIN SELECT RAISE(ABORT,'test failure'); END").unwrap();
    let tx = conn.unchecked_transaction().unwrap();
    let opts = Options {
        preferences: true,
        ..options()
    };
    let plan = plan(&tx, &p, &opts, "zh").unwrap();
    assert!(apply(&tx, &p, &plan).is_err());
    tx.rollback().unwrap();
    let after = snapshot(&conn, true, true).unwrap();
    assert_eq!(before.categories, after.categories);
    assert_eq!(before.timer_profiles, after.timer_profiles);
    assert_eq!(before.portable_preferences, after.portable_preferences);
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM import_mappings", [], |r| r.get(0))
            .unwrap(),
        0
    );
}
#[test]
fn preferences_opt_in_and_plans_never_activate() {
    let (_, mut p) = fixture();
    let conn = db();
    settings::save_setting(&conn, "language", "zh").unwrap();
    let before = read_book(&conn).unwrap().unwrap();
    p.portable_preferences.as_mut().unwrap().language = "ja".into();
    import(&conn, &p);
    assert_eq!(settings::load(&conn).unwrap().language, "zh");
    let after = read_book(&conn).unwrap().unwrap();
    assert_eq!(before.selected_id, after.selected_id);
    assert_eq!(before.working, after.working);
    let tx = conn.unchecked_transaction().unwrap();
    let opts = Options {
        preferences: true,
        ..options()
    };
    let plan = plan(&tx, &p, &opts, "zh").unwrap();
    apply(&tx, &p, &plan).unwrap();
    tx.commit().unwrap();
    assert_eq!(settings::load(&conn).unwrap().language, "ja");
}
#[test]
fn stale_plan_changes_with_local_entities_and_renames() {
    let (_, p) = fixture();
    let conn = db();
    let before = plan(&conn, &p, &options(), "zh").unwrap();
    categories::mutate(
        &conn,
        &categories::CategoryAction::Create {
            name: "阅读".into(),
        },
    )
    .unwrap();
    let after = plan(&conn, &p, &options(), "zh").unwrap();
    assert_ne!(before, after);
    import(&conn, &p);
    assert_ne!(after, plan(&conn, &p, &options(), "zh").unwrap());
}
#[test]
fn immutable_decoded_snapshot_survives_external_file_changes() {
    let (_, p) = fixture();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.pomotroid.json");
    std::fs::write(&path, serde_json::to_vec(&p).unwrap()).unwrap();
    let (saved, _) = decode(&std::fs::read(&path).unwrap()).unwrap();
    std::fs::write(&path, "corrupt").unwrap();
    let conn = db();
    import(&conn, &saved);
    assert_eq!(snapshot(&conn, false, false).unwrap().sessions, p.sessions);
}
#[test]
fn empty_database_exports_readable_package() {
    let p = snapshot(&db(), true, true).unwrap();
    let (p, n) = decode(&serde_json::to_vec(&p).unwrap()).unwrap();
    assert!(p.sessions.is_empty());
    assert!(p.categories.is_empty());
    assert_eq!(n, 0);
}
