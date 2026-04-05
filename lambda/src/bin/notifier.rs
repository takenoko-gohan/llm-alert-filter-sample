use aws_config::BehaviorVersion;
use lambda::application::services::NotificationService;
use lambda::application::config::NotifierConfig;
use lambda::domain::errors::AppError;
use lambda::infrastructure::repositories_impl::FeedbackRepositoryImpl;
use lambda::infrastructure::{bedrock, secrets, slack};
use lambda_runtime::{run, service_fn, tracing};

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    tracing::init_default_subscriber();

    let app_config = NotifierConfig::from_env()?;

    let aws_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);

    let bedrock_client = bedrock::Client::builder()
        .inner_client(aws_sdk_bedrockruntime::Client::new(&aws_config))
        .model_id(app_config.model_id().to_string())
        .top_p(app_config.top_p())
        .prompt_language(app_config.prompt_language().clone())
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
