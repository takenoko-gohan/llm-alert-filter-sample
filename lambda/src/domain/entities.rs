use crate::domain::value_objects::{FeedbackId, Timestamp};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationDecision {
    needs_notification: bool,
    #[serde(default)]
    confidence: Option<String>,
    #[serde(default)]
    matched_feedback_reason: Option<String>,
}

impl NotificationDecision {
    pub fn needs_notification(&self) -> bool {
        self.needs_notification
    }

    pub fn confidence(&self) -> Option<&str> {
        self.confidence.as_deref()
    }

    pub fn matched_feedback_reason(&self) -> Option<&str> {
        self.matched_feedback_reason.as_deref()
    }
}

#[derive(Clone, Serialize, Deserialize, TypedBuilder)]
pub(crate) struct Feedback {
    id: FeedbackId,
    created_at: Timestamp,
    log_group: String,
    message: String,
    needs_notification: bool,
    reason: Option<String>,
}

impl Feedback {
    pub(crate) fn created_at(&self) -> &Timestamp {
        &self.created_at
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn needs_notification(&self) -> bool {
        self.needs_notification
    }

    pub(crate) fn reason(&self) -> Option<&String> {
        self.reason.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_builder() {
        let id = FeedbackId::new();
        let created_at = Timestamp::new();
        let feedback = Feedback::builder()
            .id(id.clone())
            .created_at(created_at.clone())
            .log_group("/aws/lambda/my-function".to_string())
            .message("Hello, world!".to_string())
            .needs_notification(true)
            .reason(Some("Just because".to_string()))
            .build();

        assert_eq!(feedback.id, id);
        assert_eq!(feedback.created_at, created_at);
        assert_eq!(feedback.message, "Hello, world!");
        assert!(feedback.needs_notification);
        assert_eq!(feedback.reason, Some("Just because".to_string()));
    }
}
