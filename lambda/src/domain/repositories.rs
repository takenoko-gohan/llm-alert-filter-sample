use crate::domain::entities::Feedback;

pub(crate) trait FeedbackRepository {
    async fn add_feedback(&self, feedback: Feedback) -> Result<(), Box<dyn std::error::Error>>;
    async fn list_feedback_by_log_group(
        &self,
        log_group: &str,
    ) -> Result<Vec<Feedback>, Box<dyn std::error::Error>>;
}
