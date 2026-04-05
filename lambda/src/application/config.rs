use crate::domain::errors::AppError;
use crate::infrastructure::bedrock::PromptLanguage;

#[derive(Debug, Clone)]
pub struct NotifierConfig {
    table_name: String,
    model_id: String,
    top_p: f32,
    slack_channel_id: String,
    secret_id: String,
    prompt_language: PromptLanguage,
    max_retries: u32,
    base_delay_ms: u64,
}

impl NotifierConfig {
    pub fn from_env() -> Result<Self, AppError> {
        let top_p: f32 = parse_env("BEDROCK_TOP_P")?;
        if !(0.0..=1.0).contains(&top_p) {
            return Err(AppError::Config(format!(
                "BEDROCK_TOP_P must be 0.0..=1.0, got {top_p}"
            )));
        }

        let prompt_language: PromptLanguage = env_or("PROMPT_LANGUAGE", "en")
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
            top_p,
            slack_channel_id: require_env("SLACK_CHANNEL_ID")?,
            secret_id: require_env("SECRET_ID")?,
            prompt_language,
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

    pub fn top_p(&self) -> f32 {
        self.top_p
    }

    pub fn slack_channel_id(&self) -> &str {
        &self.slack_channel_id
    }

    pub fn secret_id(&self) -> &str {
        &self.secret_id
    }

    pub fn prompt_language(&self) -> &PromptLanguage {
        &self.prompt_language
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
}

impl CollectorConfig {
    pub fn from_env() -> Result<Self, AppError> {
        Ok(Self {
            table_name: require_env("TABLE_NAME")?,
            secret_id: require_env("SECRET_ID")?,
            slack_channel_id: require_env("SLACK_CHANNEL_ID")?,
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
}

fn require_env(key: &str) -> Result<String, AppError> {
    std::env::var(key).map_err(|_| AppError::Config(format!("{key} is not set")))
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_env<T: std::str::FromStr>(key: &str) -> Result<T, AppError>
where
    T::Err: std::fmt::Display,
{
    let val = require_env(key)?;
    val.parse()
        .map_err(|e| AppError::Config(format!("{key} is invalid: {e}")))
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
