pub mod commands;
pub mod files;
pub mod package;
pub mod report;

pub type Result<T> = std::result::Result<T, String>;
pub fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

pub fn message(locale: &str, key: &str) -> String {
    let raw = match locale {
        "zh" => include_str!("../../../src/messages/zh.json"),
        "de" => include_str!("../../../src/messages/de.json"),
        "es" => include_str!("../../../src/messages/es.json"),
        "fr" => include_str!("../../../src/messages/fr.json"),
        "ja" => include_str!("../../../src/messages/ja.json"),
        "pt" => include_str!("../../../src/messages/pt.json"),
        "tr" => include_str!("../../../src/messages/tr.json"),
        _ => include_str!("../../../src/messages/en.json"),
    };
    let values: serde_json::Value = serde_json::from_str(raw).unwrap();
    values[key].as_str().map(str::to_string).unwrap_or_else(|| {
        let en: serde_json::Value =
            serde_json::from_str(include_str!("../../../src/messages/en.json")).unwrap();
        en[key].as_str().unwrap_or(key).to_string()
    })
}
