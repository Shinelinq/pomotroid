use crate::settings::{self, Settings};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub const STORAGE_KEY: &str = "timer_plans_v1";
pub const CONFIG_KEYS: &[&str] = &[
    "time_work_secs",
    "time_short_break_secs",
    "time_long_break_secs",
    "work_rounds",
    "short_breaks_enabled",
    "long_breaks_enabled",
    "auto_start_work",
    "auto_start_break",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimerConfig {
    pub time_work_secs: u32,
    pub time_short_break_secs: u32,
    pub time_long_break_secs: u32,
    pub long_break_interval: u32,
    pub short_breaks_enabled: bool,
    pub long_breaks_enabled: bool,
    pub auto_start_work: bool,
    pub auto_start_break: bool,
}

impl TimerConfig {
    pub fn from_settings(s: &Settings) -> Self {
        Self {
            time_work_secs: s.time_work_secs,
            time_short_break_secs: s.time_short_break_secs,
            time_long_break_secs: s.time_long_break_secs,
            long_break_interval: s.long_break_interval,
            short_breaks_enabled: s.short_breaks_enabled,
            long_breaks_enabled: s.long_breaks_enabled,
            auto_start_work: s.auto_start_work,
            auto_start_break: s.auto_start_break,
        }
    }

    pub fn apply(&self, s: &mut Settings) {
        s.time_work_secs = self.time_work_secs;
        s.time_short_break_secs = self.time_short_break_secs;
        s.time_long_break_secs = self.time_long_break_secs;
        s.long_break_interval = self.long_break_interval;
        s.short_breaks_enabled = self.short_breaks_enabled;
        s.long_breaks_enabled = self.long_breaks_enabled;
        s.auto_start_work = self.auto_start_work;
        s.auto_start_break = self.auto_start_break;
    }

    pub fn validate(&self) -> Result<(), String> {
        if [
            self.time_work_secs,
            self.time_short_break_secs,
            self.time_long_break_secs,
        ]
        .iter()
        .any(|n| !(60..=5400).contains(n))
            || !(1..=12).contains(&self.long_break_interval)
        {
            return Err("plan_invalid_config".into());
        }
        Ok(())
    }

    pub fn change(&mut self, key: &str, value: &str) -> Result<(), String> {
        let number = || {
            value
                .parse::<u32>()
                .map_err(|_| "plan_invalid_config".to_string())
        };
        let boolean = || {
            value
                .parse::<bool>()
                .map_err(|_| "plan_invalid_config".to_string())
        };
        match key {
            "time_work_secs" => self.time_work_secs = number()?,
            "time_short_break_secs" => self.time_short_break_secs = number()?,
            "time_long_break_secs" => self.time_long_break_secs = number()?,
            "work_rounds" => self.long_break_interval = number()?,
            "short_breaks_enabled" => self.short_breaks_enabled = boolean()?,
            "long_breaks_enabled" => self.long_breaks_enabled = boolean()?,
            "auto_start_work" => self.auto_start_work = boolean()?,
            "auto_start_break" => self.auto_start_break = boolean()?,
            _ => return Err("plan_invalid_config".into()),
        }
        self.validate()
    }

    pub fn template(long: bool) -> Self {
        Self {
            time_work_secs: if long { 3000 } else { 1500 },
            time_short_break_secs: if long { 600 } else { 300 },
            time_long_break_secs: if long { 1800 } else { 900 },
            long_break_interval: 4,
            short_breaks_enabled: true,
            long_breaks_enabled: true,
            auto_start_work: false,
            auto_start_break: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub stable_id: String,
    pub name: String,
    /// The first plan's default display name comes from the message catalog.
    #[serde(default)]
    pub initial_name: bool,
    pub config: TimerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanBook {
    pub plans: Vec<Plan>,
    pub selected_id: String,
    pub working: TimerConfig,
    next_id: u64,
}

fn normalized(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

impl PlanBook {
    pub fn load(conn: &Connection, settings: &Settings) -> Result<Self, String> {
        if let Some(json) = settings::get_setting(conn, STORAGE_KEY) {
            let book: Self = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            book.find(&book.selected_id)?;
            return Ok(book);
        }
        let config = TimerConfig::from_settings(settings);
        let book = Self {
            plans: vec![Plan {
                id: "plan-1".into(),
                stable_id: uuid::Uuid::new_v4().to_string(),
                name: serde_json::from_str::<serde_json::Value>(include_str!(
                    "../../../src/messages/en.json"
                ))
                .unwrap()["plan_current"]
                    .as_str()
                    .unwrap()
                    .to_string(),
                initial_name: true,
                config: config.clone(),
            }],
            selected_id: "plan-1".into(),
            working: config,
            next_id: 2,
        };
        book.persist(conn)?;
        Ok(book)
    }

    pub fn find(&self, id: &str) -> Result<&Plan, String> {
        self.plans
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| "plan_missing".into())
    }

    pub fn name(&self, name: &str, except: Option<&str>) -> Result<String, String> {
        let name = name.trim();
        if !(1..=24).contains(&name.chars().count()) {
            return Err("plan_invalid_name".into());
        }
        let key = normalized(name);
        // Default names are translated at the UI, so check all catalog versions.
        let initial_names = [
            include_str!("../../../src/messages/en.json"),
            include_str!("../../../src/messages/zh.json"),
            include_str!("../../../src/messages/de.json"),
            include_str!("../../../src/messages/es.json"),
            include_str!("../../../src/messages/fr.json"),
            include_str!("../../../src/messages/ja.json"),
            include_str!("../../../src/messages/pt.json"),
            include_str!("../../../src/messages/tr.json"),
        ];
        if self.plans.iter().any(|p| {
            Some(p.id.as_str()) != except
                && (normalized(&p.name) == key
                    || (p.initial_name
                        && initial_names.iter().any(|json| {
                            serde_json::from_str::<serde_json::Value>(json)
                                .ok()
                                .and_then(|v| v["plan_current"].as_str().map(normalized))
                                .is_some_and(|n| n == key)
                        })))
        }) {
            return Err("plan_duplicate_name".into());
        }
        Ok(name.into())
    }

    pub fn create(&mut self, name: &str, config: TimerConfig) -> Result<String, String> {
        let name = self.name(name, None)?;
        config.validate()?;
        let id = format!("plan-{}", self.next_id);
        self.next_id += 1;
        self.plans.push(Plan {
            id: id.clone(),
            stable_id: uuid::Uuid::new_v4().to_string(),
            name,
            initial_name: false,
            config,
        });
        Ok(id)
    }

    /// Collection, selection, working configuration and legacy timer keys commit together.
    pub fn persist(&self, conn: &Connection) -> Result<(), String> {
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        settings::save_setting(&tx, STORAGE_KEY, &json).map_err(|e| e.to_string())?;
        let mut s = Settings::default();
        self.working.apply(&mut s);
        for (key, value) in [
            ("time_work_secs", s.time_work_secs.to_string()),
            ("time_short_break_secs", s.time_short_break_secs.to_string()),
            ("time_long_break_secs", s.time_long_break_secs.to_string()),
            ("work_rounds", s.long_break_interval.to_string()),
            ("short_breaks_enabled", s.short_breaks_enabled.to_string()),
            ("long_breaks_enabled", s.long_breaks_enabled.to_string()),
            ("auto_start_work", s.auto_start_work.to_string()),
            ("auto_start_break", s.auto_start_break.to_string()),
        ] {
            settings::save_setting(&tx, key, &value).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlanAction {
    ReplaceConfig {
        config: TimerConfig,
    },
    Select {
        id: String,
    },
    Edit {
        key: String,
        value: String,
    },
    Save,
    SaveAs {
        name: String,
    },
    Rename {
        id: String,
        name: String,
    },
    Delete {
        id: String,
    },
    Template {
        name: String,
        long: bool,
    },
    CancelPending,
    ApplyNow {
        round_id: u64,
        pending_revision: u64,
    },
    StopAfter {
        round_id: u64,
        enabled: bool,
    },
}
