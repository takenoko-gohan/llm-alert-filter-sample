use crate::domain::entities::Feedback;
use crate::domain::repositories::FeedbackRepository;
use crate::domain::value_objects::{FeedbackId, Timestamp};
use crate::infrastructure::repositories_impl::FeedbackRepositoryImpl;
use crate::infrastructure::slack::PrivateMetadata;
use crate::infrastructure::{bedrock, slack};
use crate::util::now_rfc3339;
use aws_lambda_events::cloudwatch_logs::LogsEvent;
use lambda_runtime::LambdaEvent;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct NotificationService {
    repo: FeedbackRepositoryImpl,
    bedrock_client: bedrock::Client,
    slack_client: slack::Client,
    slack_channel_id: String,
}

impl NotificationService {
    pub async fn slack_notification(
        &self,
        event: LambdaEvent<LogsEvent>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let payload = event.payload;
        let log_events = payload.aws_logs.data.log_events;
        let log_group = payload.aws_logs.data.log_group;

        if !log_events.is_empty() {
            let feedback = self.repo.list_feedback_by_log_group(&log_group).await?;
            for log_event in log_events {
                let message = log_event.message;

                if self
                    .bedrock_client
                    .needs_notification(feedback.clone(), message.clone(), now_rfc3339())
                    .await?
                {
                    self.slack_client
                        .post_alert(&self.slack_channel_id, &log_group, &message)
                        .await?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone, TypedBuilder)]
pub(crate) struct CollectionService {
    repo: FeedbackRepositoryImpl,
    slack_client: slack::Client,
    slack_channel_id: String,
}

impl CollectionService {
    pub(crate) async fn add_feedback(
        &self,
        private_metadata: &str,
        needs_notification: bool,
        reason: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let private_metadata = PrivateMetadata::try_from(private_metadata)?;

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
    }

    pub(crate) async fn open_modal(
        &self,
        trigger_id: &str,
        ts: String,
        log_group: String,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let private_metadata = PrivateMetadata::builder()
            .ts(ts)
            .log_group(log_group)
            .message(message)
            .build()
            .encode_base64()?;
        self.slack_client
            .open_modal(trigger_id, &private_metadata)
            .await
    }
}
