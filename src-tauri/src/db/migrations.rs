use rusqlite::{Connection, Result};

/// Full schema for version 1. Tables use IF NOT EXISTS so the batch is
/// idempotent, but the schema_version check in `run()` prevents re-execution.
const MIGRATION_1: &str = "
    CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS settings (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS sessions (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        started_at    INTEGER NOT NULL,
        ended_at      INTEGER,
        round_type    TEXT NOT NULL CHECK(round_type IN ('work', 'short-break', 'long-break')),
        duration_secs INTEGER NOT NULL CHECK(duration_secs > 0),
        completed     INTEGER NOT NULL DEFAULT 0 CHECK(completed IN (0, 1))
    );

    CREATE TABLE IF NOT EXISTS custom_themes (
        id     INTEGER PRIMARY KEY AUTOINCREMENT,
        name   TEXT NOT NULL UNIQUE,
        colors TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_sessions_started_at ON sessions(started_at);
    CREATE INDEX IF NOT EXISTS idx_sessions_round_type ON sessions(round_type);

    INSERT INTO schema_version VALUES (1);
";

/// Migrates timer duration storage from minute-resolution keys to second-resolution keys.
/// Reads existing `time_*_mins` rows, multiplies by 60, writes `time_*_secs`, then deletes
/// the old keys so key names align with the Settings struct field names.
const MIGRATION_2: &str = "
    INSERT OR IGNORE INTO settings (key, value)
        SELECT 'time_work_secs', CAST(CAST(value AS INTEGER) * 60 AS TEXT)
          FROM settings WHERE key = 'time_work_mins';
    INSERT OR IGNORE INTO settings (key, value)
        SELECT 'time_short_break_secs', CAST(CAST(value AS INTEGER) * 60 AS TEXT)
          FROM settings WHERE key = 'time_short_break_mins';
    INSERT OR IGNORE INTO settings (key, value)
        SELECT 'time_long_break_secs', CAST(CAST(value AS INTEGER) * 60 AS TEXT)
          FROM settings WHERE key = 'time_long_break_mins';
    DELETE FROM settings WHERE key IN
        ('time_work_mins', 'time_short_break_mins', 'time_long_break_mins');
    INSERT INTO schema_version VALUES (2);
";

/// Seeds the `check_for_updates` setting for users upgrading from a version
/// that did not have this setting. Fresh installs get it via seed_defaults.
const MIGRATION_3: &str = "
    INSERT OR IGNORE INTO settings (key, value) VALUES ('check_for_updates', 'true');
    INSERT INTO schema_version VALUES (3);
";

/// Seeds the `global_shortcuts_enabled` setting for all installs. Defaults to
/// 'false' — global shortcuts are now opt-in. This is a breaking change for
/// existing users who relied on shortcuts being active by default; they must
/// re-enable them in Settings → Shortcuts.
const MIGRATION_4: &str = "
    INSERT OR IGNORE INTO settings (key, value) VALUES ('global_shortcuts_enabled', 'false');
    INSERT INTO schema_version VALUES (4);
";

/// Seeds the `short_breaks_enabled` and `long_breaks_enabled` settings for
/// users upgrading from a version that did not have these settings.
/// Both default to 'true' — existing behaviour is preserved.
const MIGRATION_5: &str = "
    INSERT OR IGNORE INTO settings (key, value) VALUES ('short_breaks_enabled', 'true');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('long_breaks_enabled', 'true');
    INSERT INTO schema_version VALUES (5);
";

/// Seeds the seven local shortcut key bindings for users upgrading from a version
/// that did not have this feature. These shortcuts are handled entirely by the frontend
/// (keydown listeners) and require no Rust-side dispatch logic.
const MIGRATION_6: &str = "
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_toggle', ' ');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_reset', 'ArrowLeft');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_skip', 'ArrowRight');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_volume_down', 'ArrowDown');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_volume_up', 'ArrowUp');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_mute', 'm');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_fullscreen', 'F11');
    INSERT INTO schema_version VALUES (6);
";

// Classification is additive: existing rows keep their IDs and all recorded values.
const MIGRATION_7: &str = "
    CREATE TABLE categories (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 24),
        name_key TEXT NOT NULL UNIQUE,
        archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0, 1))
    );
    ALTER TABLE sessions ADD COLUMN category_id INTEGER REFERENCES categories(id) ON DELETE RESTRICT;
    CREATE INDEX idx_sessions_category_started ON sessions(category_id, started_at);
    INSERT INTO schema_version VALUES (7);
";

/// Apply any pending migrations. Each migration is wrapped in a transaction
/// so a partial failure leaves the database unchanged.
pub fn run(conn: &Connection) -> Result<()> {
    let version = current_version(conn)?;

    if version < 1 {
        log::info!("[db/migrations] applying MIGRATION_1: initial schema");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_1} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_1 complete");
    }

    if version < 2 {
        log::info!("[db/migrations] applying MIGRATION_2: timer durations minutes → seconds");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_2} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_2 complete");
    }

    if version < 3 {
        log::info!("[db/migrations] applying MIGRATION_3: seed check_for_updates setting");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_3} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_3 complete");
    }

    if version < 4 {
        log::info!("[db/migrations] applying MIGRATION_4: seed global_shortcuts_enabled setting");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_4} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_4 complete");
    }

    if version < 5 {
        log::info!("[db/migrations] applying MIGRATION_5: seed short_breaks_enabled and long_breaks_enabled");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_5} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_5 complete");
    }

    if version < 6 {
        log::info!("[db/migrations] applying MIGRATION_6: seed local shortcut key bindings");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_6} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_6 complete");
    }

    if version < 7 {
        log::info!("[db/migrations] applying MIGRATION_7: session categories");
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(MIGRATION_7)?;
        tx.commit()?;
    }

    if version < 8 {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(
            "ALTER TABLE sessions ADD COLUMN stable_id TEXT;
            ALTER TABLE categories ADD COLUMN stable_id TEXT;
            CREATE UNIQUE INDEX idx_sessions_stable_id ON sessions(stable_id);
            CREATE UNIQUE INDEX idx_categories_stable_id ON categories(stable_id);
            CREATE TABLE import_mappings (
                kind TEXT NOT NULL, stable_id TEXT NOT NULL,
                source_hash TEXT NOT NULL, applied_hash TEXT NOT NULL,
                PRIMARY KEY(kind,stable_id));",
        )?;
        for table in ["sessions", "categories"] {
            let ids = tx
                .prepare(&format!("SELECT id FROM {table}"))?
                .query_map([], |r| r.get::<_, i64>(0))?
                .collect::<Result<Vec<_>>>()?;
            for id in ids {
                tx.execute(
                    &format!("UPDATE {table} SET stable_id=?1 WHERE id=?2"),
                    rusqlite::params![uuid::Uuid::new_v4().to_string(), id],
                )?;
            }
        }
        if let Some(json) = crate::settings::get_setting(&tx, crate::timer::plans::STORAGE_KEY) {
            let mut book: serde_json::Value = serde_json::from_str(&json)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            if let Some(plans) = book["plans"].as_array_mut() {
                for plan in plans {
                    if !plan["stable_id"]
                        .as_str()
                        .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok_and(|id| !id.is_nil()))
                    {
                        plan["stable_id"] = uuid::Uuid::new_v4().to_string().into();
                    }
                }
            } else {
                return Err(rusqlite::Error::InvalidParameterName(
                    "invalid saved plan collection".into(),
                ));
            }
            crate::settings::save_setting(
                &tx,
                crate::timer::plans::STORAGE_KEY,
                &book.to_string(),
            )?;
        }
        tx.execute_batch("INSERT INTO schema_version VALUES (8)")?;
        tx.commit()?;
    }
    Ok(())
}

/// Returns the current schema version, or 0 if the database is fresh.
fn current_version(conn: &Connection) -> Result<i64> {
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='schema_version'",
        [],
        |row| row.get(0),
    )?;

    if !table_exists {
        return Ok(0);
    }

    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        // Second run must not error (version check prevents re-application).
        run(&conn).unwrap();
        let v: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 8);
    }

    #[test]
    fn identity_migration_only_adds_metadata_and_preserves_saved_plan_ids_and_drafts() {
        let conn = version_six();
        conn.execute_batch(MIGRATION_7).unwrap();
        conn.execute(
            "INSERT INTO categories(id,name,name_key,archived) VALUES (17,'Reading','reading',1)",
            [],
        )
        .unwrap();
        conn.execute("UPDATE sessions SET category_id=17 WHERE id=42", [])
            .unwrap();
        let original = serde_json::json!({"plans":[{"id":"plan-7","name":"Saved","initial_name":false,"config":{"time_work_secs":1500}}],"selected_id":"plan-7","working":{"time_work_secs":2345},"next_id":8});
        crate::settings::save_setting(
            &conn,
            crate::timer::plans::STORAGE_KEY,
            &original.to_string(),
        )
        .unwrap();
        run(&conn).unwrap();
        let ids = || {
            conn.prepare("SELECT stable_id FROM sessions ORDER BY id")
                .unwrap()
                .query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>>>()
                .unwrap()
        };
        let first = ids();
        run(&conn).unwrap();
        assert_eq!(ids(), first);
        assert!(first.iter().all(|id| uuid::Uuid::parse_str(id).is_ok()));
        let row: (i64, String, bool, String) = conn
            .query_row(
                "SELECT id,name,archived,stable_id FROM categories",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!((row.0, row.1, row.2), (17, "Reading".into(), true));
        assert!(uuid::Uuid::parse_str(&row.3).is_ok());
        let mut migrated: serde_json::Value = serde_json::from_str(
            &crate::settings::get_setting(&conn, crate::timer::plans::STORAGE_KEY).unwrap(),
        )
        .unwrap();
        assert!(uuid::Uuid::parse_str(migrated["plans"][0]["stable_id"].as_str().unwrap()).is_ok());
        migrated["plans"][0]
            .as_object_mut()
            .unwrap()
            .remove("stable_id");
        assert_eq!(migrated, original);
        assert_eq!(
            conn.query_row::<i64, _, _>("SELECT category_id FROM sessions WHERE id=42", [], |r| r
                .get(0))
                .unwrap(),
            17
        );
    }

    #[test]
    fn all_tables_created() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        for table in &[
            "settings",
            "sessions",
            "custom_themes",
            "schema_version",
            "categories",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "table '{table}' was not created");
        }
    }
    fn version_six() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        for migration in [
            MIGRATION_1,
            MIGRATION_2,
            MIGRATION_3,
            MIGRATION_4,
            MIGRATION_5,
            MIGRATION_6,
        ] {
            conn.execute_batch(migration).unwrap();
        }
        conn.execute_batch("INSERT INTO sessions(id,started_at,ended_at,round_type,duration_secs,completed) VALUES (42,1700000000,1700000901,'work',901,1),(99,1700001000,NULL,'work',5399,0),(120,1700002000,1700002060,'short-break',60,1);").unwrap();
        conn
    }

    #[test]
    fn category_migration_preserves_old_records_and_all_time_totals() {
        let conn = version_six();
        let read = || {
            conn.prepare("SELECT id,started_at,ended_at,round_type,duration_secs,completed FROM sessions ORDER BY id").unwrap()
            .query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,Option<i64>>(2)?,r.get::<_,String>(3)?,r.get::<_,u32>(4)?,r.get::<_,bool>(5)?))).unwrap().collect::<Result<Vec<_>>>().unwrap()
        };
        let before = read();
        run(&conn).unwrap();
        run(&conn).unwrap();
        assert_eq!(read(), before);
        assert_eq!(
            conn.query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM sessions WHERE category_id IS NULL",
                [],
                |r| r.get(0)
            )
            .unwrap(),
            3
        );
        assert_eq!(
            conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM categories", [], |r| r.get(0))
                .unwrap(),
            0
        );
        let totals = super::super::queries::get_all_time_stats(&conn).unwrap();
        assert_eq!(
            (
                totals.total_work_sessions,
                totals.completed_work_sessions,
                totals.total_work_secs
            ),
            (2, 1, 901)
        );
    }

    #[test]
    fn failed_category_migration_rolls_back_table_column_and_version() {
        let conn = version_six();
        conn.execute_batch("CREATE INDEX idx_sessions_category_started ON sessions(started_at)")
            .unwrap();
        assert!(run(&conn).is_err());
        assert_eq!(current_version(&conn).unwrap(), 6);
        assert_eq!(
            conn.query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='categories'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
            0
        );
        assert!(conn.prepare("SELECT category_id FROM sessions").is_err());
        conn.execute_batch("DROP INDEX idx_sessions_category_started")
            .unwrap();
        run(&conn).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 8);
    }
}
