use aws_config::BehaviorVersion;
use lambda::application::config::NotifierConfig;
use lambda::application::services::NotificationService;
use lambda::domain::errors::AppError;
use lambda::infrastructure::i18n::Messages;
use lambda::infrastructure::repositories_impl::FeedbackRepositoryImpl;
use lambda::infrastructure::{bedrock, secrets, slack};
use lambda::util::parse_lambda_log_level;
use lambda_runtime::{run, service_fn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
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

    let app_config = NotifierConfig::from_env()?;

    let aws_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);

    let bedrock_client = bedrock::Client::builder()
        .inner_client(aws_sdk_bedrockruntime::Client::new(&aws_config))
        .model_id(app_config.model_id().to_string())
        .prompt_language(app_config.language().clone())
        .max_retries(app_config.max_retries())
        .base_delay_ms(app_config.base_delay_ms())
        .build();
    let secrets_client = secrets::Client::builder()
        .inner(aws_sdk_secretsmanager::Client::new(&aws_config))
        .build();

    secrets_client.load_secrets(app_config.secret_id()).await?;
    let slack_token = std::env::var("SLACK_TOKEN").expect("SLACK_TOKEN is not set");

    let slack_client = slack::Client::builder()
        .inner_client(reqwest::Client::new())
        .token(slack_token)
        .messages(Messages::from_language(app_config.language()))
        .build();

    let repo = FeedbackRepositoryImpl::builder()
        .client(dynamodb_client)
        .table_name(app_config.table_name().to_string())
        .build();
    let srv = NotificationService::builder()
        .repo(repo)
        .bedrock_client(bedrock_client)
        .slack_client(slack_client)
        .slack_channel_id(app_config.slack_channel_id().to_string())
        .build();

    run(service_fn(|event| async {
        srv.slack_notification(event)
            .await
            .map_err(|e: AppError| -> lambda_runtime::Diagnostic { e.into() })
    }))
    .await
}
