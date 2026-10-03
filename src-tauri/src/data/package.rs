use super::{err, Result};
use crate::{
    db::categories,
    settings,
    timer::plans::{Plan, PlanBook, TimerConfig, STORAGE_KEY},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTITIES: usize = 250_000;
const MAX_CATEGORIES: usize = 4096;
const MAX_PROFILES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub stable_id: String,
    pub started_at_utc_secs: i64,
    pub ended_at_utc_secs: Option<i64>,
    pub round_type: String,
    pub duration_secs: i64,
    pub completed: bool,
    pub category_stable_id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub stable_id: String,
    pub name: String,
    pub archived: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub stable_id: String,
    pub name: String,
    pub initial_name: bool,
    pub config: TimerConfig,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub language: String,
    pub dial_countdown: bool,
    pub tray_display_mode: String,
    pub theme_mode: String,
    pub theme_light: Option<String>,
    pub theme_dark: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub format: String,
    pub format_version: u32,
    pub export_id: String,
    pub exported_at: String,
    pub app_version: String,
    pub source_timezone: Option<String>,
    pub sections: Vec<String>,
    pub sessions: Vec<Session>,
    pub categories: Vec<Category>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timer_profiles: Option<Vec<Profile>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portable_preferences: Option<Preferences>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub history: bool,
    pub profiles: bool,
    pub preferences: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub added: usize,
    pub existing: usize,
    pub conflicts: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Difference {
    pub field: String,
    pub local: String,
    pub file: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conflict {
    pub kind: String,
    pub label: String,
    pub differences: Vec<Difference>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rename {
    pub kind: String,
    pub from: String,
    pub to: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub sessions: Counts,
    pub categories: Counts,
    pub profiles: Counts,
    pub preferences: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    pub summary: Summary,
    pub conflicts: Vec<Conflict>,
    pub renames: Vec<Rename>,
    pub add_sessions: BTreeSet<String>,
    pub add_categories: BTreeMap<String, String>,
    pub add_profiles: BTreeMap<String, String>,
    pub preferences_before: Option<Preferences>,
}

pub fn hash(value: &impl Serialize) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("serializable data"))
    )
}
pub fn read_book(conn: &Connection) -> Result<Option<PlanBook>> {
    settings::get_setting(conn, STORAGE_KEY)
        .map(|json| serde_json::from_str(&json).map_err(err))
        .transpose()
}
fn preferences(conn: &Connection) -> Result<Preferences> {
    let s = settings::load(conn).map_err(err)?;
    let themes = crate::themes::load_bundled();
    let builtin = |name: String| themes.iter().any(|t| t.name == name).then_some(name);
    Ok(Preferences {
        language: s.language,
        dial_countdown: s.dial_countdown,
        tray_display_mode: s.tray_display_mode,
        theme_mode: s.theme_mode,
        theme_light: builtin(s.theme_light),
        theme_dark: builtin(s.theme_dark),
    })
}
/// Caller holds the connection and a read/write transaction. No runtime state is serialized.
pub fn snapshot(conn: &Connection, profiles: bool, prefs: bool) -> Result<Package> {
    let categories = conn
        .prepare("SELECT stable_id,name,archived FROM categories ORDER BY stable_id")
        .map_err(err)?
        .query_map([], |r| {
            Ok(Category {
                stable_id: r.get(0)?,
                name: r.get(1)?,
                archived: r.get(2)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    let sessions = conn.prepare("SELECT s.stable_id,s.started_at,s.ended_at,s.round_type,s.duration_secs,s.completed,c.stable_id
        FROM sessions s LEFT JOIN categories c ON c.id=s.category_id ORDER BY s.stable_id").map_err(err)?
        .query_map([], |r| Ok(Session { stable_id:r.get(0)?, started_at_utc_secs:r.get(1)?, ended_at_utc_secs:r.get(2)?,
            round_type:r.get(3)?, duration_secs:r.get(4)?, completed:r.get(5)?, category_stable_id:r.get(6)? })).map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(err)?;
    let timer_profiles = if profiles {
        Some(
            read_book(conn)?
                .map(|b| {
                    b.plans
                        .into_iter()
                        .map(|p| Profile {
                            stable_id: p.stable_id,
                            name: p.name,
                            initial_name: p.initial_name,
                            config: p.config,
                        })
                        .collect()
                })
                .unwrap_or_default(),
        )
    } else {
        None
    };
    let mut sections = vec!["sessions".into(), "categories".into()];
    if profiles {
        sections.push("timer_profiles".into());
    }
    if prefs {
        sections.push("portable_preferences".into());
    }
    Ok(Package {
        format: "pomotroid-personal-data".into(),
        format_version: 1,
        export_id: uuid::Uuid::new_v4().to_string(),
        exported_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        app_version: env!("CARGO_PKG_VERSION").into(),
        source_timezone: iana_time_zone::get_timezone().ok(),
        sections,
        sessions,
        categories,
        timer_profiles,
        portable_preferences: if prefs {
            Some(preferences(conn)?)
        } else {
            None
        },
    })
}
pub fn encode(p: &Package) -> Result<Vec<u8>> {
    struct Limited(Vec<u8>);
    impl std::io::Write for Limited {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len() as u64 + bytes.len() as u64 > MAX_BYTES {
                return Err(std::io::Error::other("data_too_large"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Limited(Vec::new());
    serde_json::to_writer_pretty(&mut writer, p).map_err(|_| "data_too_large")?;
    Ok(writer.0)
}
fn valid_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|u| u.to_string() == id && !u.is_nil())
}
fn valid_name(s: &str) -> bool {
    (1..=24).contains(&s.chars().count()) && !s.trim().is_empty()
}
fn timestamp(v: i64) -> bool {
    (-62_167_219_200..=253_402_300_799).contains(&v)
}
fn dedup<T: Serialize + PartialEq>(values: &mut Vec<T>, id: impl Fn(&T) -> &str) -> Result<usize> {
    let mut seen = BTreeMap::new();
    let mut duplicates = 0;
    for value in values.iter() {
        if let Some(previous) = seen.insert(id(value).to_string(), value) {
            if previous != value {
                return Err("data_internal_conflict".into());
            }
            duplicates += 1;
        }
    }
    let mut seen = BTreeSet::new();
    values.retain(|v| seen.insert(id(v).to_string()));
    Ok(duplicates)
}
pub fn validate(p: &mut Package) -> Result<usize> {
    if p.format != "pomotroid-personal-data" {
        return Err("data_format".into());
    }
    if p.format_version != 1 {
        return Err("data_version".into());
    }
    let mut sections = vec!["sessions", "categories"];
    if p.timer_profiles.is_some() {
        sections.push("timer_profiles");
    }
    if p.portable_preferences.is_some() {
        sections.push("portable_preferences");
    }
    sections.sort();
    let mut actual = p.sections.iter().map(String::as_str).collect::<Vec<_>>();
    actual.sort();
    if sections != actual
        || !valid_id(&p.export_id)
        || p.app_version.len() > 128
        || p.exported_at.len() > 64
        || p.source_timezone.as_ref().is_some_and(|s| s.len() > 128)
        || chrono::DateTime::parse_from_rfc3339(&p.exported_at)
            .map_or(true, |d| d.offset().local_minus_utc() != 0)
        || p.sessions.len() + p.categories.len() + p.timer_profiles.as_ref().map_or(0, Vec::len)
            > MAX_ENTITIES
    {
        return Err("data_invalid".into());
    }
    if p.categories.len() > MAX_CATEGORIES
        || p.timer_profiles
            .as_ref()
            .is_some_and(|p| p.len() > MAX_PROFILES)
    {
        return Err("data_invalid".into());
    }
    for c in &p.categories {
        if !valid_id(&c.stable_id) || !valid_name(&c.name) {
            return Err("data_invalid".into());
        }
    }
    let categories: BTreeSet<_> = p.categories.iter().map(|c| c.stable_id.as_str()).collect();
    for s in &p.sessions {
        if !valid_id(&s.stable_id)
            || !timestamp(s.started_at_utc_secs)
            || s.ended_at_utc_secs.is_some_and(|v| !timestamp(v))
            || !["work", "short-break", "long-break"].contains(&s.round_type.as_str())
            || s.duration_secs <= 0
        {
            return Err("data_invalid".into());
        }
        if s.category_stable_id
            .as_ref()
            .is_some_and(|id| !categories.contains(id.as_str()))
        {
            return Err("data_reference".into());
        }
    }
    if let Some(profiles) = &p.timer_profiles {
        for v in profiles {
            if !valid_id(&v.stable_id)
                || !valid_name(&v.name)
                || [
                    v.config.time_work_secs,
                    v.config.time_short_break_secs,
                    v.config.time_long_break_secs,
                ]
                .contains(&0)
                || v.config.long_break_interval == 0
            {
                return Err("data_invalid".into());
            }
        }
    }
    if let Some(v) = &p.portable_preferences {
        let themes = crate::themes::load_bundled();
        if !["auto", "en", "zh", "de", "es", "fr", "ja", "pt", "tr"].contains(&v.language.as_str())
            || !["auto", "light", "dark"].contains(&v.theme_mode.as_str())
            || !["progress", "minutes"].contains(&v.tray_display_mode.as_str())
            || [&v.theme_light, &v.theme_dark].iter().any(|n| {
                n.as_ref()
                    .is_some_and(|s| !themes.iter().any(|t| t.name == *s))
            })
        {
            return Err("data_preferences_invalid".into());
        }
    }
    let mut n =
        dedup(&mut p.sessions, |s| &s.stable_id)? + dedup(&mut p.categories, |c| &c.stable_id)?;
    if let Some(profiles) = &mut p.timer_profiles {
        n += dedup(profiles, |p| &p.stable_id)?;
    }
    Ok(n)
}
pub fn decode(bytes: &[u8]) -> Result<(Package, usize)> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err("data_too_large".into());
    }
    // Bound nesting before deserialization, including unknown fields. Strings are escaped JSON.
    let (mut depth, mut string, mut escaped) = (0u32, false, false);
    for b in bytes {
        if string {
            if escaped {
                escaped = false;
            } else if *b == b'\\' {
                escaped = true;
            } else if *b == b'"' {
                string = false;
            }
        } else {
            match b {
                b'"' => string = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > 12 {
                        return Err("data_invalid".into());
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    // Inspect the format header first so future versions report the right error.
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| "data_corrupt")?;
    if value["format"] != "pomotroid-personal-data" {
        return Err("data_format".into());
    }
    if value["format_version"].as_u64() != Some(1) {
        return Err("data_version".into());
    }
    let mut p: Package = serde_json::from_value(value).map_err(|_| "data_invalid")?;
    let n = validate(&mut p)?;
    Ok((p, n))
}
fn same(
    conn: &Connection,
    kind: &str,
    id: &str,
    source: &impl Serialize,
    local: &impl Serialize,
) -> Result<bool> {
    let (source, local) = (hash(source), hash(local));
    if source == local {
        return Ok(true);
    }
    let mapping = conn
        .query_row(
            "SELECT source_hash,applied_hash FROM import_mappings WHERE kind=?1 AND stable_id=?2",
            params![kind, id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(err)?;
    Ok(mapping.is_some_and(|(s, a)| s == source && a == local))
}
fn conflict(kind: &str, label: String, local: &impl Serialize, file: &impl Serialize) -> Conflict {
    let a = serde_json::to_value(local).unwrap();
    let b = serde_json::to_value(file).unwrap();
    let mut differences = Vec::new();
    for (key, value) in b.as_object().unwrap() {
        if a[key] == *value {
            continue;
        }
        if let Some(fields) = value.as_object() {
            for (field, value) in fields {
                if a[key][field] != *value {
                    differences.push(Difference {
                        field: field.clone(),
                        local: a[key][field].to_string(),
                        file: value.to_string(),
                    });
                }
            }
        } else {
            differences.push(Difference {
                field: key.clone(),
                local: a[key].to_string(),
                file: value.to_string(),
            });
        }
    }
    Conflict {
        kind: kind.into(),
        label,
        differences,
    }
}
fn renamed(name: &str, n: usize, locale: &str) -> String {
    let suffix = super::message(locale, "data_import_suffix");
    let suffix = if n == 1 {
        format!(" ({suffix})")
    } else {
        format!(" ({suffix}{n})")
    };
    format!(
        "{}{}",
        name.chars()
            .take(24 - suffix.chars().count())
            .collect::<String>(),
        suffix
    )
}
pub fn plan(conn: &Connection, p: &Package, o: &Options, locale: &str) -> Result<ImportPlan> {
    if (o.profiles && p.timer_profiles.is_none())
        || (o.preferences && p.portable_preferences.is_none())
    {
        return Err("data_invalid".into());
    }
    let local = snapshot(conn, true, o.preferences)?;
    let mut result = ImportPlan {
        summary: Summary::default(),
        conflicts: vec![],
        renames: vec![],
        add_sessions: BTreeSet::new(),
        add_categories: BTreeMap::new(),
        add_profiles: BTreeMap::new(),
        preferences_before: local.portable_preferences,
    };
    if o.history {
        let by_id: BTreeMap<_, _> = local.categories.iter().map(|c| (&c.stable_id, c)).collect();
        let mut names: BTreeSet<_> = local
            .categories
            .iter()
            .map(|c| categories::normalized(&c.name))
            .collect();
        let mut suffixes: BTreeMap<String, usize> = BTreeMap::new();
        for c in &p.categories {
            if let Some(old) = by_id.get(&c.stable_id) {
                if same(conn, "category", &c.stable_id, c, old)? {
                    result.summary.categories.existing += 1;
                } else {
                    result.summary.categories.conflicts += 1;
                    result
                        .conflicts
                        .push(conflict("category", c.name.clone(), old, c));
                }
            } else {
                let mut name = c.name.clone();
                let mut n = *suffixes.get(&categories::normalized(&c.name)).unwrap_or(&0);
                loop {
                    let checked = categories::checked_name(conn, &name, None);
                    if let Err(error) = &checked {
                        if ![
                            "category_duplicate_name",
                            "category_reserved_name",
                            "category_invalid_name",
                        ]
                        .contains(&error.as_str())
                        {
                            return Err(error.clone());
                        }
                    }
                    if !names.contains(&categories::normalized(&name)) && checked.is_ok() {
                        break;
                    }
                    n += 1;
                    name = renamed(&c.name, n, locale);
                }
                names.insert(categories::normalized(&name));
                suffixes.insert(categories::normalized(&c.name), n);
                if name != c.name {
                    result.renames.push(Rename {
                        kind: "category".into(),
                        from: c.name.clone(),
                        to: name.clone(),
                    });
                }
                result.add_categories.insert(c.stable_id.clone(), name);
                result.summary.categories.added += 1;
            }
        }
        let by_id: BTreeMap<_, _> = local.sessions.iter().map(|s| (&s.stable_id, s)).collect();
        for s in &p.sessions {
            if let Some(old) = by_id.get(&s.stable_id) {
                if *old == s {
                    result.summary.sessions.existing += 1;
                } else {
                    result.summary.sessions.conflicts += 1;
                    result.conflicts.push(conflict(
                        "session",
                        s.started_at_utc_secs.to_string(),
                        old,
                        s,
                    ));
                }
            } else {
                result.add_sessions.insert(s.stable_id.clone());
                result.summary.sessions.added += 1;
            }
        }
    }
    if o.profiles {
        let old = local.timer_profiles.unwrap_or_default();
        let by_id: BTreeMap<_, _> = old.iter().map(|p| (&p.stable_id, p)).collect();
        let book = read_book(conn)?;
        let mut names: BTreeSet<_> = old
            .iter()
            .map(|p| categories::normalized(&p.name))
            .collect();
        let mut suffixes: BTreeMap<String, usize> = BTreeMap::new();
        for p in p.timer_profiles.as_ref().unwrap() {
            if let Some(old) = by_id.get(&p.stable_id) {
                if same(conn, "profile", &p.stable_id, p, old)? {
                    result.summary.profiles.existing += 1;
                } else {
                    result.summary.profiles.conflicts += 1;
                    result
                        .conflicts
                        .push(conflict("profile", p.name.clone(), old, p));
                }
            } else {
                let mut name = p.name.clone();
                let mut n = *suffixes.get(&categories::normalized(&p.name)).unwrap_or(&0);
                while names.contains(&categories::normalized(&name))
                    || book.as_ref().is_some_and(|b| b.name(&name, None).is_err())
                {
                    n += 1;
                    name = renamed(&p.name, n, locale);
                }
                names.insert(categories::normalized(&name));
                suffixes.insert(categories::normalized(&p.name), n);
                if name != p.name {
                    result.renames.push(Rename {
                        kind: "profile".into(),
                        from: p.name.clone(),
                        to: name.clone(),
                    });
                }
                result.add_profiles.insert(p.stable_id.clone(), name);
                result.summary.profiles.added += 1;
            }
        }
    }
    result.summary.preferences =
        o.preferences && result.preferences_before != p.portable_preferences;
    Ok(result)
}
fn mapping(
    conn: &Connection,
    kind: &str,
    id: &str,
    source: &impl Serialize,
    applied: &impl Serialize,
) -> Result<()> {
    conn.execute("INSERT OR REPLACE INTO import_mappings(kind,stable_id,source_hash,applied_hash) VALUES (?1,?2,?3,?4)",
        params![kind,id,hash(source),hash(applied)]).map_err(err)?;
    Ok(())
}
/// Runs only inside the caller's transaction; any error drops/rolls back the entire transaction.
pub fn apply(conn: &Connection, p: &Package, plan: &ImportPlan) -> Result<()> {
    for c in &p.categories {
        if let Some(name) = plan.add_categories.get(&c.stable_id) {
            conn.execute(
                "INSERT INTO categories(stable_id,name,name_key,archived) VALUES (?1,?2,?3,?4)",
                params![c.stable_id, name, categories::normalized(name), c.archived],
            )
            .map_err(err)?;
            let mut applied = c.clone();
            applied.name = name.clone();
            mapping(conn, "category", &c.stable_id, c, &applied)?;
        }
    }
    if !plan.add_profiles.is_empty() {
        let mut book = read_book(conn)?.ok_or("data_profile_storage")?;
        for p in p.timer_profiles.as_ref().unwrap() {
            if let Some(name) = plan.add_profiles.get(&p.stable_id) {
                book.plans.push(Plan {
                    id: format!("import-{}", p.stable_id),
                    stable_id: p.stable_id.clone(),
                    name: name.clone(),
                    initial_name: false,
                    config: p.config.clone(),
                });
                let mut applied = p.clone();
                applied.name = name.clone();
                applied.initial_name = false;
                mapping(conn, "profile", &p.stable_id, p, &applied)?;
            }
        }
        // Deliberately leave selection, drafts, timer keys and pending round configuration alone.
        settings::save_setting(
            conn,
            STORAGE_KEY,
            &serde_json::to_string(&book).map_err(err)?,
        )
        .map_err(err)?;
    }
    let ids: BTreeMap<String, i64> = conn
        .prepare("SELECT stable_id,id FROM categories")
        .map_err(err)?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    for s in &p.sessions {
        if plan.add_sessions.contains(&s.stable_id) {
            let category = s
                .category_stable_id
                .as_ref()
                .map(|id| ids.get(id).copied().ok_or("data_reference"))
                .transpose()?;
            conn.execute("INSERT INTO sessions(stable_id,started_at,ended_at,round_type,duration_secs,completed,category_id) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![s.stable_id,s.started_at_utc_secs,s.ended_at_utc_secs,s.round_type,s.duration_secs,s.completed,category]).map_err(err)?;
        }
    }
    if plan.summary.preferences {
        let v = p.portable_preferences.as_ref().unwrap();
        for (key, value) in [
            ("language", v.language.clone()),
            ("dial_countdown", v.dial_countdown.to_string()),
            ("tray_display_mode", v.tray_display_mode.clone()),
            ("theme_mode", v.theme_mode.clone()),
        ] {
            settings::save_setting(conn, key, &value).map_err(err)?;
        }
        for (key, value) in [
            ("theme_light", &v.theme_light),
            ("theme_dark", &v.theme_dark),
        ] {
            if let Some(value) = value {
                settings::save_setting(conn, key, value).map_err(err)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
