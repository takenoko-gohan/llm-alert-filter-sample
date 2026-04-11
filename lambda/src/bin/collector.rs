use aws_config::BehaviorVersion;
use axum::Router;
use lambda::application::config::CollectorConfig;
use lambda::infrastructure::i18n::Messages;
use lambda::infrastructure::{secrets, slack};
use lambda::interface::middleware::create_auth_layer;
use lambda::interface::routers::create_feedback_router;
use lambda::util::parse_lambda_log_level;
use lambda_http::{run, Error};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Error> {
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

    let app_config = CollectorConfig::from_env()?;

    let aws_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);
    let secrets_client = secrets::Client::builder()
        .inner(aws_sdk_secretsmanager::Client::new(&aws_config))
        .build();

    secrets_client.load_secrets(app_config.secret_id()).await?;
    let signing_secret = std::env::var("SIGNING_SECRET").expect("SIGNING_SECRET is not set");
    let slack_token = std::env::var("SLACK_TOKEN").expect("SLACK_TOKEN is not set");

    let slack_client = slack::Client::builder()
        .inner_client(reqwest::Client::new())
        .token(slack_token)
        .messages(Messages::from_language(app_config.language()))
        .build();

    let feedback = create_feedback_router(
        dynamodb_client,
        app_config.table_name().to_string(),
        slack_client,
        app_config.slack_channel_id().to_string(),
    )
    .await;
    let auth = create_auth_layer(signing_secret);
    let app = Router::new().nest("/feedback", feedback).layer(auth);

    run(app).await
}
