use crate::domain::entities::Language;
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder)]
pub struct Messages {
    alert_header: String,
    log_group_label: String,
    message_label: String,
    confidence_label: String,
    feedback_button: String,
    feedback_done: String,
    modal_title: String,
    needs_notification_question: String,
    needs_notification_yes: String,
    needs_notification_no: String,
    reason_label: String,
    cancel: String,
    submit: String,
}

impl Messages {
    pub fn from_language(lang: &Language) -> Self {
        match lang {
            Language::En => Self::en(),
            Language::Ja => Self::ja(),
        }
    }

    fn en() -> Self {
        Self::builder()
            .alert_header(":rotating_light: An error has occurred :rotating_light:".into())
            .log_group_label("*CloudWatch Logs Log Group*".into())
            .message_label("*Log Message*".into())
            .confidence_label("Confidence".into())
            .feedback_button("Feedback".into())
            .feedback_done("_Feedback submitted_".into())
            .modal_title("Feedback".into())
            .needs_notification_question("Is notification required?".into())
            .needs_notification_yes("Required".into())
            .needs_notification_no("Not required".into())
            .reason_label("Reason".into())
            .cancel("Cancel".into())
            .submit("Submit".into())
            .build()
    }

    fn ja() -> Self {
        Self::builder()
            .alert_header(":rotating_light: エラーが発生しました :rotating_light:".into())
            .log_group_label("*CloudWatch Logs ロググループ*".into())
            .message_label("*ログメッセージ*".into())
            .confidence_label("Confidence".into())
            .feedback_button("フィードバック".into())
            .feedback_done("_フィードバック済み_".into())
            .modal_title("フィードバック".into())
            .needs_notification_question("通知が必要ですか？".into())
            .needs_notification_yes("必要".into())
            .needs_notification_no("不要".into())
            .reason_label("理由".into())
            .cancel("キャンセル".into())
            .submit("送信".into())
            .build()
    }

    pub fn alert_header(&self) -> &str {
        &self.alert_header
    }

    pub fn log_group_label(&self) -> &str {
        &self.log_group_label
    }

    pub fn message_label(&self) -> &str {
        &self.message_label
    }

    pub fn confidence_label(&self) -> &str {
        &self.confidence_label
    }

    pub fn feedback_button(&self) -> &str {
        &self.feedback_button
    }

    pub fn feedback_done(&self) -> &str {
        &self.feedback_done
    }

    pub fn modal_title(&self) -> &str {
        &self.modal_title
    }

    pub fn needs_notification_question(&self) -> &str {
        &self.needs_notification_question
    }

    pub fn needs_notification_yes(&self) -> &str {
        &self.needs_notification_yes
    }

    pub fn needs_notification_no(&self) -> &str {
        &self.needs_notification_no
    }

    pub fn reason_label(&self) -> &str {
        &self.reason_label
    }

    pub fn cancel(&self) -> &str {
        &self.cancel
    }

    pub fn submit(&self) -> &str {
        &self.submit
    }
}
