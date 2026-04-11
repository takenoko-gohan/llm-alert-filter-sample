use chrono::SecondsFormat;
use serde::de;

pub(crate) fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;

    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(de::Error::unknown_variant(s, &["true", "false"])),
    }
}

pub(crate) fn now_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

pub(crate) fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn parse_lambda_log_level(level: &str) -> Option<String> {
    match level.to_uppercase().as_str() {
        "TRACE" => Some("trace".into()),
        "DEBUG" => Some("debug".into()),
        "INFO" => Some("info".into()),
        "WARN" => Some("warn".into()),
        "ERROR" | "FATAL" => Some("error".into()),
        _ => None,
    }
}
