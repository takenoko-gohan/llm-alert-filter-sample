use crate::domain::value_objects::{FeedbackId, Timestamp};
use serde::{Deserialize, Serialize};
use std::fmt;
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, Default)]
pub enum Language {
    #[default]
    En,
    Ja,
}

impl std::str::FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "en" => Ok(Language::En),
            "ja" => Ok(Language::Ja),
            other => Err(format!(
                "unsupported language: '{}' (expected 'en' or 'ja')",
                other
            )),
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Language::En => write!(f, "en"),
            Language::Ja => write!(f, "ja"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn is_high(&self) -> bool {
        matches!(self, Confidence::High)
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Confidence::High => ":large_green_circle:",
            Confidence::Medium => ":large_yellow_circle:",
            Confidence::Low => ":red_circle:",
        }
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Confidence::High => write!(f, "high"),
            Confidence::Medium => write!(f, "medium"),
            Confidence::Low => write!(f, "low"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder)]
pub struct NotificationDecision {
    needs_notification: bool,
    #[serde(default)]
    #[builder(default)]
    confidence: Option<Confidence>,
    #[serde(default)]
    #[builder(default)]
    matched_feedback_reason: Option<String>,
}

impl NotificationDecision {
    pub fn needs_notification(&self) -> bool {
        self.needs_notification
    }

    pub fn confidence(&self) -> Option<&Confidence> {
        self.confidence.as_ref()
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
