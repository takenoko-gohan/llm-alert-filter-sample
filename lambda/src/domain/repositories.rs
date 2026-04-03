use crate::domain::entities::Feedback;
use crate::domain::errors::DynamoDbError;

pub(crate) trait FeedbackRepository {
    async fn add_feedback(&self, feedback: Feedback) -> Result<(), DynamoDbError>;
    async fn list_feedback_by_log_group(
        &self,
        log_group: &str,
    ) -> Result<Vec<Feedback>, DynamoDbError>;
}
