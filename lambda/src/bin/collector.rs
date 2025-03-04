use aws_config::BehaviorVersion;
use axum::Router;
use lambda::infrastructure::{secrets, slack};
use lambda::interface::middleware::create_auth_layer;
use lambda::interface::routers::create_feedback_router;
use lambda_http::{run, tracing, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let secret_id = std::env::var("SECRET_ID").expect("SECRET_ID is not set");
    let table_name = std::env::var("TABLE_NAME").expect("TABLE_NAME is not set");
    let slack_channel_id = std::env::var("SLACK_CHANNEL_ID").expect("SLACK_CHANNEL is not set");

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let secrets_client = secrets::Client::builder()
        .inner(aws_sdk_secretsmanager::Client::new(&config))
        .build();

    secrets_client.load_secrets(&secret_id).await?;
    let signing_secret = std::env::var("SIGNING_SECRET").expect("SIGNING_SECRET is not set");
    let slack_token = std::env::var("SLACK_TOKEN").expect("SLACK_TOKEN is not set");

    let slack_client = slack::Client::builder()
        .inner_client(reqwest::Client::new())
        .token(slack_token)
        .build();

    let feedback =
        create_feedback_router(dynamodb_client, table_name, slack_client, slack_channel_id).await;
    let auth = create_auth_layer(signing_secret);
    let app = Router::new().nest("/feedback", feedback).layer(auth);

    run(app).await
}
