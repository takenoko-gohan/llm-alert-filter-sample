use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Bedrock API error: {0}")]
    Bedrock(#[from] BedrockError),
    #[error("DynamoDB error: {0}")]
    DynamoDb(#[from] DynamoDbError),
    #[error("Slack API error: {0}")]
    Slack(#[from] SlackError),
    #[error("Configuration error: {0}")]
    Config(String),
}

impl From<AppError> for lambda_runtime::Diagnostic {
    fn from(error: AppError) -> Self {
        lambda_runtime::Diagnostic {
            error_type: match &error {
                AppError::Bedrock(_) => "BedrockError".into(),
                AppError::DynamoDb(_) => "DynamoDbError".into(),
                AppError::Slack(_) => "SlackError".into(),
                AppError::Config(_) => "ConfigError".into(),
            },
            error_message: error.to_string(),
        }
    }
}

#[derive(Debug, Error)]
pub enum BedrockError {
    #[error("Converse API call failed: {source}")]
    ConverseFailed {
        #[source]
        source: Box<
            aws_sdk_bedrockruntime::error::SdkError<
                aws_sdk_bedrockruntime::operation::converse::ConverseError,
            >,
        >,
    },
    #[error("Failed to build request: {detail}")]
    RequestBuild { detail: String },
    #[error("No valid response block found")]
    NoValidBlock,
    #[error("Failed to parse response: {detail}")]
    ResponseParse { detail: String },
    #[error("Throttled, retry after {retry_after_ms}ms")]
    Throttled { retry_after_ms: u64 },
}

impl BedrockError {
    pub fn is_retryable(&self) -> bool {
        match self {
            BedrockError::ConverseFailed { source } => {
                use aws_sdk_bedrockruntime::error::ProvideErrorMetadata;
                source.code().is_some_and(|code| {
                    code == "ThrottlingException" || code == "ServiceUnavailableException"
                })
            }
            BedrockError::Throttled { .. } => true,
            _ => false,
        }
    }
}

#[derive(Debug, Error)]
pub enum DynamoDbError {
    #[error("Query failed for log_group={log_group}: {source}")]
    Query {
        log_group: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Put item failed: {0}")]
    Put(Box<dyn std::error::Error + Send + Sync>),
    #[error("Deserialization failed: {0}")]
    Deserialize(String),
}

#[derive(Debug, Error)]
pub enum SlackError {
    #[error("Post to channel={channel} failed: {detail}")]
    PostFailed { channel: String, detail: String },
    #[error("Update message failed: {detail}")]
    UpdateFailed { detail: String },
    #[error("Modal open failed: {detail}")]
    ModalFailed { detail: String },
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
}
