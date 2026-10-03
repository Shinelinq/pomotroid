use crate::settings;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const SELECTION_KEY: &str = "focus_category_id";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub archived: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CategoryData {
    pub items: Vec<Category>,
    pub selected_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CategoryAction {
    Create { name: String },
    Rename { id: i64, name: String },
    Archive { id: i64 },
    Restore { id: i64 },
    Select { id: Option<i64> },
    CancelPending { round_id: u64 },
    DismissNotice,
}

/// A query scope is independent of the timer's default/locked classification.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CategoryFilter {
    #[default]
    All,
    Uncategorized,
    Category {
        category_id: i64,
    },
}

impl<'de> Deserialize<'de> for CategoryFilter {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Input {
            All {},
            Uncategorized {},
            Category { category_id: i64 },
        }
        Ok(match Input::deserialize(deserializer)? {
            Input::All {} => Self::All,
            Input::Uncategorized {} => Self::Uncategorized,
            Input::Category { category_id } => Self::Category { category_id },
        })
    }
}

impl CategoryFilter {
    pub fn validate(&self, conn: &Connection) -> rusqlite::Result<()> {
        if let Self::Category { category_id } = self {
            validate_id(*category_id).map_err(rusqlite::Error::InvalidParameterName)?;
            // Archived categories are deliberately valid historical scopes.
            conn.query_row(
                "SELECT id FROM categories WHERE id=?1",
                [category_id],
                |_| Ok(()),
            )?;
        }
        Ok(())
    }
    pub fn id(&self) -> Option<i64> {
        match self {
            Self::Category { category_id } => Some(*category_id),
            _ => None,
        }
    }
    /// Only trusted SQL fragments; all category IDs remain bound parameters.
    /// Each fragment uses the same parameter, including the two non-ID scopes.
    pub fn condition(&self) -> &'static str {
        match self {
            Self::All => ":category_id IS NULL",
            Self::Uncategorized => "category_id IS NULL AND :category_id IS NULL",
            Self::Category { .. } => "category_id = :category_id",
        }
    }
}

pub fn validate_id(id: i64) -> Result<(), String> {
    if !(1..=9_007_199_254_740_991).contains(&id) {
        return Err("category_invalid_id".into());
    }
    Ok(())
}

pub fn list(conn: &Connection) -> Result<Vec<Category>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, archived FROM categories ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                archived: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())
}

fn selection(conn: &Connection) -> Result<Option<i64>, String> {
    let value = conn
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [SELECTION_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    value
        .map(|value| serde_json::from_str::<Option<i64>>(&value).map_err(|e| e.to_string()))
        .transpose()
        .map(Option::flatten)
}

pub fn load(conn: &Connection) -> Result<CategoryData, String> {
    let items = list(conn)?;
    let selected_id =
        selection(conn)?.filter(|id| items.iter().any(|c| c.id == *id && !c.archived));
    Ok(CategoryData { items, selected_id })
}

// Match the existing plan-name rules: trim, collapse whitespace for comparison,
// Unicode lowercase, 1–24 scalar characters. Archived names remain reserved.
fn normalized(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn checked_name(
    conn: &Connection,
    name: &str,
    except: Option<i64>,
) -> Result<(String, String), String> {
    let name = name.trim();
    if !(1..=24).contains(&name.chars().count()) {
        return Err("category_invalid_name".into());
    }
    let key = normalized(name);
    for catalog in [
        include_str!("../../../src/messages/en.json"),
        include_str!("../../../src/messages/zh.json"),
    ] {
        let messages: serde_json::Value =
            serde_json::from_str(catalog).map_err(|e| e.to_string())?;
        if messages["category_uncategorized"]
            .as_str()
            .is_some_and(|name| normalized(name) == key)
        {
            return Err("category_reserved_name".into());
        }
    }
    let duplicate: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM categories WHERE name_key=?1 AND (?2 IS NULL OR id != ?2))",
        params![key, except], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if duplicate {
        return Err("category_duplicate_name".into());
    }
    Ok((name.into(), key))
}

pub fn active(conn: &Connection, id: Option<i64>) -> Result<(), String> {
    if let Some(id) = id {
        validate_id(id)?;
        let archived: bool = conn
            .query_row("SELECT archived FROM categories WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())?
            .ok_or("category_missing")?;
        if archived {
            return Err("category_archived_error".into());
        }
    }
    Ok(())
}

/// Caller serializes this with work-start using the existing controller queue.
/// Archive + fallback selection commit together; sessions are never updated here.
pub fn mutate(conn: &Connection, action: &CategoryAction) -> Result<CategoryData, String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    match action {
        CategoryAction::Create { name } => {
            let (name, key) = checked_name(&tx, name, None)?;
            tx.execute(
                "INSERT INTO categories(name,name_key) VALUES (?1,?2)",
                params![name, key],
            )
            .map_err(|e| e.to_string())?;
        }
        CategoryAction::Rename { id, name } => {
            validate_id(*id)?;
            let (name, key) = checked_name(&tx, name, Some(*id))?;
            if tx
                .execute(
                    "UPDATE categories SET name=?1, name_key=?2 WHERE id=?3",
                    params![name, key, id],
                )
                .map_err(|e| e.to_string())?
                == 0
            {
                return Err("category_missing".into());
            }
        }
        CategoryAction::Archive { id } | CategoryAction::Restore { id } => {
            validate_id(*id)?;
            let archived = matches!(action, CategoryAction::Archive { .. });
            if tx
                .execute(
                    "UPDATE categories SET archived=?1 WHERE id=?2",
                    params![archived, id],
                )
                .map_err(|e| e.to_string())?
                == 0
            {
                return Err("category_missing".into());
            }
            if archived && selection(&tx)? == Some(*id) {
                settings::save_setting(&tx, SELECTION_KEY, "null").map_err(|e| e.to_string())?;
            }
        }
        CategoryAction::Select { id } => {
            active(&tx, *id)?;
            settings::save_setting(
                &tx,
                SELECTION_KEY,
                &serde_json::to_string(id).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        CategoryAction::CancelPending { .. } | CategoryAction::DismissNotice => {
            return Err("category_invalid_action".into())
        }
    }
    let data = load(&tx)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(data)
}

#[cfg(test)]
mod tests;
