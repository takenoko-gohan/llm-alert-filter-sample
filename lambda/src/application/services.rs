use crate::application::ports::{AlertNotifier, NotificationJudge};
use crate::domain::entities::Feedback;
use crate::domain::errors::AppError;
use crate::domain::repositories::FeedbackRepository;
use crate::domain::value_objects::{FeedbackId, Timestamp};
use crate::infrastructure::slack::PrivateMetadata;
use crate::util::now_rfc3339;
use aws_lambda_events::cloudwatch_logs::LogsEvent;
use lambda_runtime::LambdaEvent;
use tokio::time::Instant;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct NotificationService<R, B, S> {
    repo: R,
    bedrock_client: B,
    slack_client: S,
    slack_channel_id: String,
}

impl<R, B, S> NotificationService<R, B, S>
where
    R: FeedbackRepository,
    B: NotificationJudge,
    S: AlertNotifier,
{
    pub async fn slack_notification(&self, event: LambdaEvent<LogsEvent>) -> Result<(), AppError> {
        let payload = event.payload;
        let log_events = payload.aws_logs.data.log_events;
        let log_group = payload.aws_logs.data.log_group;

        if !log_events.is_empty() {
            let feedback = self.repo.list_feedback_by_log_group(&log_group).await?;
            for log_event in log_events {
                let message = log_event.message;

                let start = Instant::now();
                let decision = self
                    .bedrock_client
                    .judge(&feedback, &message, &now_rfc3339())
                    .await?;
                let bedrock_latency_ms = start.elapsed().as_millis() as u64;

                let confidence = decision.confidence();
                let reason = decision.matched_feedback_reason().unwrap_or("");

                let is_high_confidence = confidence.is_some_and(|c| c.is_high());
                let should_notify = if !decision.needs_notification() && !is_high_confidence {
                    tracing::warn!(
                        log_group = %log_group,
                        confidence = ?confidence,
                        reason = %reason,
                        "Non-high confidence suppression overridden to notify (fail-safe)"
                    );
                    true
                } else {
                    decision.needs_notification()
                };

                let message_preview: String = message.chars().take(100).collect();
                tracing::info!(
                    log_group = %log_group,
                    message_preview = %message_preview,
                    bedrock_decision = decision.needs_notification(),
                    should_notify = should_notify,
                    confidence = ?confidence,
                    matched_reason = %reason,
                    bedrock_latency_ms = bedrock_latency_ms,
                    "Notification decision"
                );

                if should_notify {
                    self.slack_client
                        .post_alert(
                            &self.slack_channel_id,
                            &log_group,
                            &message,
                            confidence.cloned(),
                        )
                        .await?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone, TypedBuilder)]
pub(crate) struct CollectionService<R, S> {
    repo: R,
    slack_client: S,
    slack_channel_id: String,
}

impl<R, S> CollectionService<R, S>
where
    R: FeedbackRepository,
    S: AlertNotifier,
{
    pub(crate) async fn add_feedback(
        &self,
        private_metadata: &str,
        needs_notification: bool,
        reason: Option<String>,
    ) -> Result<(), AppError> {
        use crate::domain::errors::SlackError;

        let private_metadata =
            PrivateMetadata::try_from(private_metadata).map_err(|e| SlackError::ModalFailed {
                detail: e.to_string(),
            })?;

        let feedback = Feedback::builder()
            .id(FeedbackId::new())
            .created_at(Timestamp::new())
            .log_group(private_metadata.log_group().to_string())
            .message(private_metadata.message().to_string())
            .needs_notification(needs_notification)
            .reason(reason)
            .build();

        self.repo.add_feedback(feedback).await?;

        self.slack_client
            .close_feedback_button(
                &self.slack_channel_id,
                private_metadata.ts(),
                private_metadata.log_group(),
                private_metadata.message(),
            )
            .await
            .map_err(AppError::from)
    }

    pub(crate) async fn open_modal(
        &self,
        trigger_id: &str,
        ts: String,
        log_group: String,
        message: String,
    ) -> Result<(), AppError> {
        use crate::domain::errors::SlackError;

        let private_metadata = PrivateMetadata::builder()
            .ts(ts)
            .log_group(log_group)
            .message(message)
            .build()
            .encode_base64()
            .map_err(|e| SlackError::ModalFailed {
                detail: e.to_string(),
            })?;
        self.slack_client
            .open_modal(trigger_id, &private_metadata)
            .await
            .map_err(AppError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::{AlertNotifier, NotificationJudge};
    use crate::domain::entities::{Confidence, Feedback, NotificationDecision};
    use crate::domain::errors::{BedrockError, DynamoDbError, SlackError};
    use crate::domain::repositories::FeedbackRepository;
    use crate::domain::value_objects::{FeedbackId, Timestamp};
    use aws_lambda_events::cloudwatch_logs::{AwsLogs, LogData, LogEntry, LogsEvent};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    // ── Mock: FeedbackRepository ──

    #[derive(Clone)]
    struct MockRepo {
        feedback: Vec<Feedback>,
        add_called: Arc<AtomicBool>,
    }

    impl MockRepo {
        fn new(feedback: Vec<Feedback>) -> Self {
            Self {
                feedback,
                add_called: Arc::new(AtomicBool::new(false)),
            }
        }

        fn empty() -> Self {
            Self::new(vec![])
        }
    }

    impl FeedbackRepository for MockRepo {
        async fn add_feedback(&self, _feedback: Feedback) -> Result<(), DynamoDbError> {
            self.add_called.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn list_feedback_by_log_group(
            &self,
            _log_group: &str,
        ) -> Result<Vec<Feedback>, DynamoDbError> {
            Ok(self.feedback.clone())
        }
    }

    // ── Mock: NotificationJudge ──

    struct MockJudge {
        decision: NotificationDecision,
    }

    impl NotificationJudge for MockJudge {
        async fn judge(
            &self,
            _feedback: &[Feedback],
            _message: &str,
            _timestamp: &str,
        ) -> Result<NotificationDecision, BedrockError> {
            Ok(self.decision.clone())
        }
    }

    struct FailingJudge;

    impl NotificationJudge for FailingJudge {
        async fn judge(
            &self,
            _feedback: &[Feedback],
            _message: &str,
            _timestamp: &str,
        ) -> Result<NotificationDecision, BedrockError> {
            Err(BedrockError::NoValidBlock)
        }
    }

    // ── Mock: AlertNotifier ──

    #[derive(Clone, Default)]
    struct MockNotifier {
        post_alert_count: Arc<AtomicUsize>,
        close_feedback_count: Arc<AtomicUsize>,
        open_modal_count: Arc<AtomicUsize>,
    }

    impl AlertNotifier for MockNotifier {
        async fn post_alert(
            &self,
            _channel_id: &str,
            _log_group: &str,
            _message: &str,
            _confidence: Option<Confidence>,
        ) -> Result<(), SlackError> {
            self.post_alert_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn close_feedback_button(
            &self,
            _channel_id: &str,
            _ts: &str,
            _log_group: &str,
            _message: &str,
        ) -> Result<(), SlackError> {
            self.close_feedback_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn open_modal(
            &self,
            _trigger_id: &str,
            _private_metadata: &str,
        ) -> Result<(), SlackError> {
            self.open_modal_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    // ── Helpers ──

    fn make_decision(notify: bool, confidence: Option<Confidence>) -> NotificationDecision {
        NotificationDecision::builder()
            .needs_notification(notify)
            .confidence(confidence)
            .matched_feedback_reason(Some("test reason".to_string()))
            .build()
    }

    fn make_test_event(log_group: &str, messages: Vec<&str>) -> LambdaEvent<LogsEvent> {
        let log_events = messages
            .into_iter()
            .enumerate()
            .map(|(i, m)| {
                let mut entry = LogEntry::default();
                entry.id = format!("event-{i}");
                entry.timestamp = 1234567890;
                entry.message = m.to_string();
                entry
            })
            .collect();

        let mut log_data = LogData::default();
        log_data.log_group = log_group.to_string();
        log_data.log_events = log_events;

        let mut aws_logs = AwsLogs::default();
        aws_logs.data = log_data;

        let mut event = LogsEvent::default();
        event.aws_logs = aws_logs;

        LambdaEvent::new(event, lambda_runtime::Context::default())
    }

    fn make_encoded_metadata() -> String {
        PrivateMetadata::builder()
            .ts("1234567890.123456".to_string())
            .log_group("/aws/lambda/test".to_string())
            .message("Error: test".to_string())
            .build()
            .encode_base64()
            .unwrap()
    }

    // ── NotificationService tests ──

    #[tokio::test]
    async fn test_notify_high_confidence() {
        let notifier = MockNotifier::default();
        let service = NotificationService::builder()
            .repo(MockRepo::empty())
            .bedrock_client(MockJudge {
                decision: make_decision(true, Some(Confidence::High)),
            })
            .slack_client(notifier.clone())
            .slack_channel_id("C123".to_string())
            .build();

        let event = make_test_event("/aws/lambda/test", vec!["Error: test"]);
        assert!(service.slack_notification(event).await.is_ok());
        assert_eq!(notifier.post_alert_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_suppress_high_confidence() {
        let notifier = MockNotifier::default();
        let service = NotificationService::builder()
            .repo(MockRepo::empty())
            .bedrock_client(MockJudge {
                decision: make_decision(false, Some(Confidence::High)),
            })
            .slack_client(notifier.clone())
            .slack_channel_id("C123".to_string())
            .build();

        let event = make_test_event("/aws/lambda/test", vec!["Error: test"]);
        assert!(service.slack_notification(event).await.is_ok());
        assert_eq!(notifier.post_alert_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_fail_safe_low_confidence() {
        let notifier = MockNotifier::default();
        let service = NotificationService::builder()
            .repo(MockRepo::empty())
            .bedrock_client(MockJudge {
                decision: make_decision(false, Some(Confidence::Low)),
            })
            .slack_client(notifier.clone())
            .slack_channel_id("C123".to_string())
            .build();

        let event = make_test_event("/aws/lambda/test", vec!["Error: test"]);
        assert!(service.slack_notification(event).await.is_ok());
        assert_eq!(notifier.post_alert_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_fail_safe_medium_confidence() {
        let notifier = MockNotifier::default();
        let service = NotificationService::builder()
            .repo(MockRepo::empty())
            .bedrock_client(MockJudge {
                decision: make_decision(false, Some(Confidence::Medium)),
            })
            .slack_client(notifier.clone())
            .slack_channel_id("C123".to_string())
            .build();

        let event = make_test_event("/aws/lambda/test", vec!["Error: test"]);
        assert!(service.slack_notification(event).await.is_ok());
        assert_eq!(notifier.post_alert_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_fail_safe_no_confidence() {
        let notifier = MockNotifier::default();
        let service = NotificationService::builder()
            .repo(MockRepo::empty())
            .bedrock_client(MockJudge {
                decision: make_decision(false, None),
            })
            .slack_client(notifier.clone())
            .slack_channel_id("C123".to_string())
            .build();

        let event = make_test_event("/aws/lambda/test", vec!["Error: test"]);
        assert!(service.slack_notification(event).await.is_ok());
        assert_eq!(notifier.post_alert_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_empty_log_events() {
        let notifier = MockNotifier::default();
        let service = NotificationService::builder()
            .repo(MockRepo::empty())
            .bedrock_client(MockJudge {
                decision: make_decision(true, Some(Confidence::High)),
            })
            .slack_client(notifier.clone())
            .slack_channel_id("C123".to_string())
            .build();

        let event = make_test_event("/aws/lambda/test", vec![]);
        assert!(service.slack_notification(event).await.is_ok());
        assert_eq!(notifier.post_alert_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_bedrock_error_propagated() {
        let notifier = MockNotifier::default();
        let service = NotificationService::builder()
            .repo(MockRepo::empty())
            .bedrock_client(FailingJudge)
            .slack_client(notifier.clone())
            .slack_channel_id("C123".to_string())
            .build();

        let event = make_test_event("/aws/lambda/test", vec!["Error: test"]);
        assert!(service.slack_notification(event).await.is_err());
        assert_eq!(notifier.post_alert_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_multiple_log_events() {
        let notifier = MockNotifier::default();
        let service = NotificationService::builder()
            .repo(MockRepo::empty())
            .bedrock_client(MockJudge {
                decision: make_decision(true, Some(Confidence::High)),
            })
            .slack_client(notifier.clone())
            .slack_channel_id("C123".to_string())
            .build();

        let event = make_test_event("/aws/lambda/test", vec!["Error 1", "Error 2", "Error 3"]);
        assert!(service.slack_notification(event).await.is_ok());
        assert_eq!(notifier.post_alert_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_feedback_passed_to_judge() {
        let feedback = vec![Feedback::builder()
            .id(FeedbackId::new())
            .created_at(Timestamp::new())
            .log_group("/aws/lambda/test".to_string())
            .message("known error".to_string())
            .needs_notification(false)
            .reason(Some("already handled".to_string()))
            .build()];

        let notifier = MockNotifier::default();
        let service = NotificationService::builder()
            .repo(MockRepo::new(feedback))
            .bedrock_client(MockJudge {
                decision: make_decision(false, Some(Confidence::High)),
            })
            .slack_client(notifier.clone())
            .slack_channel_id("C123".to_string())
            .build();

        let event = make_test_event("/aws/lambda/test", vec!["known error"]);
        assert!(service.slack_notification(event).await.is_ok());
        assert_eq!(notifier.post_alert_count.load(Ordering::SeqCst), 0);
    }

    // ── CollectionService tests ──

    #[tokio::test]
    async fn test_add_feedback_success() {
        let repo = MockRepo::empty();
        let notifier = MockNotifier::default();
        let service = CollectionService::builder()
            .repo(repo.clone())
            .slack_client(notifier.clone())
            .slack_channel_id("C123".to_string())
            .build();

        let metadata = make_encoded_metadata();
        let result = service
            .add_feedback(&metadata, false, Some("known issue".to_string()))
            .await;

        assert!(result.is_ok());
        assert!(repo.add_called.load(Ordering::SeqCst));
        assert_eq!(notifier.close_feedback_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_add_feedback_invalid_metadata() {
        let service = CollectionService::builder()
            .repo(MockRepo::empty())
            .slack_client(MockNotifier::default())
            .slack_channel_id("C123".to_string())
            .build();

        let result = service.add_feedback("invalid-metadata", false, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_open_modal_success() {
        let notifier = MockNotifier::default();
        let service = CollectionService::builder()
            .repo(MockRepo::empty())
            .slack_client(notifier.clone())
            .slack_channel_id("C123".to_string())
            .build();

        let result = service
            .open_modal(
                "trigger-123",
                "ts-123".to_string(),
                "/aws/lambda/test".to_string(),
                "Error: test".to_string(),
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(notifier.open_modal_count.load(Ordering::SeqCst), 1);
    }
}
