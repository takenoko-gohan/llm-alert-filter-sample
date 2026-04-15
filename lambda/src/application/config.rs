use crate::domain::entities::Language;
use crate::domain::errors::AppError;

#[derive(Debug, Clone)]
pub struct NotifierConfig {
    table_name: String,
    model_id: String,
    slack_channel_id: String,
    secret_id: String,
    language: Language,
    max_retries: u32,
    base_delay_ms: u64,
}

impl NotifierConfig {
    pub fn from_env() -> Result<Self, AppError> {
        let language: Language = env_or("APP_LANGUAGE", "en")
            .parse()
            .map_err(|e: String| AppError::Config(e))?;

        let max_retries: u32 = parse_env_or("MAX_RETRIES", 3)?;
        if max_retries > 6 {
            return Err(AppError::Config(format!(
                "MAX_RETRIES must be 0..=6, got {max_retries}"
            )));
        }

        let base_delay_ms: u64 = parse_env_or("BASE_DELAY_MS", 500)?;
        if !(100..=10000).contains(&base_delay_ms) {
            return Err(AppError::Config(format!(
                "BASE_DELAY_MS must be 100..=10000, got {base_delay_ms}"
            )));
        }

        Ok(Self {
            table_name: require_env("TABLE_NAME")?,
            model_id: require_env("BEDROCK_MODEL_ID")?,
            slack_channel_id: require_env("SLACK_CHANNEL_ID")?,
            secret_id: require_env("SECRET_ID")?,
            language,
            max_retries,
            base_delay_ms,
        })
    }

    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn slack_channel_id(&self) -> &str {
        &self.slack_channel_id
    }

    pub fn secret_id(&self) -> &str {
        &self.secret_id
    }

    pub fn language(&self) -> &Language {
        &self.language
    }

    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    pub fn base_delay_ms(&self) -> u64 {
        self.base_delay_ms
    }
}

#[derive(Debug, Clone)]
pub struct CollectorConfig {
    table_name: String,
    secret_id: String,
    slack_channel_id: String,
    language: Language,
}

impl CollectorConfig {
    pub fn from_env() -> Result<Self, AppError> {
        let language: Language = env_or("APP_LANGUAGE", "en")
            .parse()
            .map_err(|e: String| AppError::Config(e))?;

        Ok(Self {
            table_name: require_env("TABLE_NAME")?,
            secret_id: require_env("SECRET_ID")?,
            slack_channel_id: require_env("SLACK_CHANNEL_ID")?,
            language,
        })
    }

    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    pub fn secret_id(&self) -> &str {
        &self.secret_id
    }

    pub fn slack_channel_id(&self) -> &str {
        &self.slack_channel_id
    }

    pub fn language(&self) -> &Language {
        &self.language
    }
}

fn require_env(key: &str) -> Result<String, AppError> {
    std::env::var(key).map_err(|_| AppError::Config(format!("{key} is not set")))
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_env_or<T: std::str::FromStr>(key: &str, default: T) -> Result<T, AppError>
where
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(val) => val
            .parse()
            .map_err(|e| AppError::Config(format!("{key} is invalid: {e}"))),
        Err(_) => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_required_env() {
        std::env::set_var("TABLE_NAME", "test-table");
        std::env::set_var("BEDROCK_MODEL_ID", "test-model");
        std::env::set_var("SLACK_CHANNEL_ID", "C123");
        std::env::set_var("SECRET_ID", "test-secret");
    }

    fn clear_env() {
        for key in [
            "TABLE_NAME",
            "BEDROCK_MODEL_ID",
            "SLACK_CHANNEL_ID",
            "SECRET_ID",
            "APP_LANGUAGE",
            "MAX_RETRIES",
            "BASE_DELAY_MS",
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn test_valid_config() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();

        let config = NotifierConfig::from_env();
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.table_name(), "test-table");
        assert_eq!(config.max_retries(), 3);
        assert_eq!(config.base_delay_ms(), 500);

        clear_env();
    }

    #[test]
    fn test_max_retries_out_of_range() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        std::env::set_var("MAX_RETRIES", "7");

        let result = NotifierConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("MAX_RETRIES"));

        clear_env();
    }

    #[test]
    fn test_base_delay_ms_too_low() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        std::env::set_var("BASE_DELAY_MS", "50");

        let result = NotifierConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("BASE_DELAY_MS"));

        clear_env();
    }

    #[test]
    fn test_base_delay_ms_too_high() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        std::env::set_var("BASE_DELAY_MS", "20000");

        let result = NotifierConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("BASE_DELAY_MS"));

        clear_env();
    }

    #[test]
    fn test_missing_required_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_env();

        let result = NotifierConfig::from_env();
        assert!(result.is_err());

        clear_env();
    }

    #[test]
    fn test_max_retries_boundary_values() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();

        std::env::set_var("MAX_RETRIES", "0");
        assert!(NotifierConfig::from_env().is_ok());

        std::env::set_var("MAX_RETRIES", "6");
        assert!(NotifierConfig::from_env().is_ok());

        clear_env();
    }

    #[test]
    fn test_base_delay_ms_boundary_values() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();

        std::env::set_var("BASE_DELAY_MS", "100");
        assert!(NotifierConfig::from_env().is_ok());

        std::env::set_var("BASE_DELAY_MS", "10000");
        assert!(NotifierConfig::from_env().is_ok());

        clear_env();
    }

    #[test]
    fn test_collector_config_valid() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("TABLE_NAME", "test-table");
        std::env::set_var("SECRET_ID", "test-secret");
        std::env::set_var("SLACK_CHANNEL_ID", "C123");

        let config = CollectorConfig::from_env();
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.table_name(), "test-table");
        assert!(matches!(config.language(), Language::En));

        clear_env();
    }
}
