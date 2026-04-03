use aws_config::BehaviorVersion;
use lambda::application::services::NotificationService;
use lambda::domain::errors::AppError;
use lambda::infrastructure::repositories_impl::FeedbackRepositoryImpl;
use lambda::infrastructure::{bedrock, secrets, slack};
use lambda_runtime::{run, service_fn, tracing};

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    tracing::init_default_subscriber();

    let table_name = std::env::var("TABLE_NAME").expect("TABLE_NAME is not set");
    let model_id = std::env::var("BEDROCK_MODEL_ID").expect("BEDROCK_MODEL_ID is not set");
    let top_p: f32 = std::env::var("BEDROCK_TOP_P")
        .expect("BEDROCK_TOP_P is not set")
        .parse()
        .expect("BEDROCK_TOP_P is not a valid float");
    let slack_channel_id = std::env::var("SLACK_CHANNEL_ID").expect("SLACK_CHANNEL is not set");
    let secret_id = std::env::var("SECRET_ID").expect("SECRET_ID is not set");
    let prompt_language: bedrock::PromptLanguage = std::env::var("PROMPT_LANGUAGE")
        .unwrap_or_else(|_| "en".to_string())
        .parse()
        .expect("PROMPT_LANGUAGE is invalid");

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let bedrock_client = bedrock::Client::builder()
        .inner_client(aws_sdk_bedrockruntime::Client::new(&config))
        .model_id(model_id)
        .top_p(top_p)
        .prompt_language(prompt_language)
        .build();
    let secrets_client = secrets::Client::builder()
        .inner(aws_sdk_secretsmanager::Client::new(&config))
        .build();

    secrets_client.load_secrets(&secret_id).await?;
    let slack_token = std::env::var("SLACK_TOKEN").expect("SLACK_TOKEN is not set");

    let slack_client = slack::Client::builder()
        .inner_client(reqwest::Client::new())
        .token(slack_token)
        .build();

    let repo = FeedbackRepositoryImpl::builder()
        .client(dynamodb_client)
        .table_name(table_name)
        .build();
    let srv = NotificationService::builder()
        .repo(repo)
        .bedrock_client(bedrock_client)
        .slack_client(slack_client)
        .slack_channel_id(slack_channel_id)
        .build();

    run(service_fn(|event| async {
        srv.slack_notification(event)
            .await
            .map_err(|e: AppError| -> lambda_runtime::Diagnostic { e.into() })
    }))
    .await
}
