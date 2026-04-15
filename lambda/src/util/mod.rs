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

pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let env_filter = std::env::var("AWS_LAMBDA_LOG_LEVEL")
        .ok()
        .and_then(|level| parse_lambda_log_level(&level))
        .map(EnvFilter::new)
        .or_else(|| EnvFilter::try_from_default_env().ok())
        .unwrap_or_else(|| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .json()
        .with_ansi(false)
        .without_time()
        .with_env_filter(env_filter)
        .init();
}

fn parse_lambda_log_level(level: &str) -> Option<String> {
    match level.to_uppercase().as_str() {
        "TRACE" => Some("trace".into()),
        "DEBUG" => Some("debug".into()),
        "INFO" => Some("info".into()),
        "WARN" => Some("warn".into()),
        "ERROR" | "FATAL" => Some("error".into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn test_parse_lambda_log_level_all_variants() {
        assert_eq!(parse_lambda_log_level("TRACE"), Some("trace".into()));
        assert_eq!(parse_lambda_log_level("DEBUG"), Some("debug".into()));
        assert_eq!(parse_lambda_log_level("INFO"), Some("info".into()));
        assert_eq!(parse_lambda_log_level("WARN"), Some("warn".into()));
        assert_eq!(parse_lambda_log_level("ERROR"), Some("error".into()));
        assert_eq!(parse_lambda_log_level("FATAL"), Some("error".into()));
    }

    #[test]
    fn test_parse_lambda_log_level_case_insensitive() {
        assert_eq!(parse_lambda_log_level("info"), Some("info".into()));
        assert_eq!(parse_lambda_log_level("Warn"), Some("warn".into()));
        assert_eq!(parse_lambda_log_level("fatal"), Some("error".into()));
    }

    #[test]
    fn test_parse_lambda_log_level_unknown() {
        assert_eq!(parse_lambda_log_level("UNKNOWN"), None);
        assert_eq!(parse_lambda_log_level(""), None);
    }

    #[derive(Deserialize)]
    struct BoolWrapper {
        #[serde(deserialize_with = "deserialize_bool")]
        value: bool,
    }

    #[test]
    fn test_deserialize_bool_true() {
        let json = r#"{"value": "true"}"#;
        let wrapper: BoolWrapper = serde_json::from_str(json).unwrap();
        assert!(wrapper.value);
    }

    #[test]
    fn test_deserialize_bool_false() {
        let json = r#"{"value": "false"}"#;
        let wrapper: BoolWrapper = serde_json::from_str(json).unwrap();
        assert!(!wrapper.value);
    }

    #[test]
    fn test_deserialize_bool_invalid() {
        let json = r#"{"value": "yes"}"#;
        let result: Result<BoolWrapper, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
