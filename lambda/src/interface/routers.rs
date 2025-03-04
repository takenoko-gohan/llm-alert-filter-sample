use crate::application::services::CollectionService;
use crate::infrastructure::repositories_impl::FeedbackRepositoryImpl;
use crate::infrastructure::slack;
use crate::interface::handlers::add_feedback_handler;
use axum::routing::post;
use axum::Router;

pub async fn create_feedback_router(
    dynamodb_client: aws_sdk_dynamodb::Client,
    table_name: String,
    slack_client: slack::Client,
    slack_channel_id: String,
) -> Router {
    let repo = FeedbackRepositoryImpl::builder()
        .client(dynamodb_client)
        .table_name(table_name)
        .build();

    let state = CollectionService::builder()
        .repo(repo)
        .slack_client(slack_client)
        .slack_channel_id(slack_channel_id)
        .build();
    Router::new()
        .route("/", post(add_feedback_handler))
        .with_state(state)
}
