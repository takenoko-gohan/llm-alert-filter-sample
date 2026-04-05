use aws_config::BehaviorVersion;
use axum::Router;
use lambda::application::config::CollectorConfig;
use lambda::infrastructure::{secrets, slack};
use lambda::interface::middleware::create_auth_layer;
use lambda::interface::routers::create_feedback_router;
use lambda_http::{run, tracing, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

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
