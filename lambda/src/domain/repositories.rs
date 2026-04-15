use crate::domain::entities::Feedback;
use crate::domain::errors::DynamoDbError;
use std::future::Future;

pub trait FeedbackRepository: Send + Sync {
    fn add_feedback(
        &self,
        feedback: Feedback,
    ) -> impl Future<Output = Result<(), DynamoDbError>> + Send;
    fn list_feedback_by_log_group(
        &self,
        log_group: &str,
    ) -> impl Future<Output = Result<Vec<Feedback>, DynamoDbError>> + Send;
}
