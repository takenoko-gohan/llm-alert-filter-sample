use crate::domain::entities::{Confidence, Feedback, NotificationDecision};
use crate::domain::errors::{BedrockError, SlackError};
use std::future::Future;

pub trait NotificationJudge: Send + Sync {
    fn judge(
        &self,
        feedback: &[Feedback],
        message: &str,
        timestamp: &str,
    ) -> impl Future<Output = Result<NotificationDecision, BedrockError>> + Send;
}

pub trait AlertNotifier: Send + Sync {
    fn post_alert(
        &self,
        channel_id: &str,
        log_group: &str,
        message: &str,
        confidence: Option<Confidence>,
    ) -> impl Future<Output = Result<(), SlackError>> + Send;

    fn close_feedback_button(
        &self,
        channel_id: &str,
        ts: &str,
        log_group: &str,
        message: &str,
    ) -> impl Future<Output = Result<(), SlackError>> + Send;

    fn open_modal(
        &self,
        trigger_id: &str,
        private_metadata: &str,
    ) -> impl Future<Output = Result<(), SlackError>> + Send;
}
