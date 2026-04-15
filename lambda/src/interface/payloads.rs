use crate::util::deserialize_bool;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum InteractivityPayload {
    BlockActions(BlockActions),
    ViewSubmission(ViewSubmission),
}

#[derive(Debug, Deserialize)]
#[serde(untagged, rename_all = "snake_case")]
pub(crate) enum BlockActions {
    OpenModal(OpenModal),
    #[allow(dead_code)]
    Other(serde_json::Value),
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenModal {
    trigger_id: String,
    message: Message,
}

impl OpenModal {
    pub(crate) fn trigger_id(&self) -> &str {
        &self.trigger_id
    }

    pub(crate) fn get_ts(&self) -> &str {
        &self.message.ts
    }

    pub(crate) fn get_log_group(&self) -> Option<&str> {
        self.message
            .blocks
            .iter()
            .find_map(|block| match block.block_id {
                BlockId::LogGroup => Some(&block.text.as_ref()?.value),
                _ => None,
            })
            .map(|value| value.as_str())
    }

    pub(crate) fn get_message(&self) -> Option<&str> {
        self.message
            .blocks
            .iter()
            .find_map(|block| match block.block_id {
                BlockId::Message => Some(&block.text.as_ref()?.value),
                _ => None,
            })
            .map(|value| value.as_str())
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ViewSubmission {
    view: View,
}

impl ViewSubmission {
    pub(crate) fn get_private_metadata(&self) -> &str {
        &self.view.private_metadata
    }

    pub(crate) fn get_modal_values(
        &self,
    ) -> Result<(bool, Option<String>), Box<dyn std::error::Error>> {
        let needs_notification = match self.view.state.values.get("needs_notification") {
            Some(Value::NeedsNotification(need_notification)) => {
                need_notification.selected_option.value
            }
            _ => return Err("Needs notification not found")?,
        };
        let reason = match self.view.state.values.get("reason") {
            Some(Value::Reason(reason)) => reason.value.clone(),
            _ => return Err("Reason not found")?,
        };
        Ok((needs_notification, reason))
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Message {
    ts: String,
    blocks: Vec<Block>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Block {
    block_id: BlockId,
    text: Option<Text>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlockId {
    Header,
    LogGroupHeader,
    MessageHeader,
    LogGroup,
    Message,
    Divider,
    Confidence,
    FeedbackButton,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Text {
    #[serde(rename = "text")]
    value: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct View {
    state: State,
    private_metadata: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct State {
    values: HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Value {
    #[serde(rename = "needs_notification")]
    NeedsNotification(NeedsNotification),
    Reason(Reason),
}

#[derive(Debug, Deserialize)]
pub(crate) struct NeedsNotification {
    selected_option: SelectedOption,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Reason {
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SelectedOption {
    #[serde(deserialize_with = "deserialize_bool")]
    value: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_open_modal() {
        let json = serde_json::json!({
            "type": "block_actions",
            "trigger_id": "trigger-123",
            "message": {
                "ts": "1234567890.123456",
                "blocks": [
                    { "block_id": "header", "text": { "text": "header" } },
                    { "block_id": "log_group", "text": { "text": "/aws/lambda/test" } },
                    { "block_id": "message", "text": { "text": "Error occurred" } },
                    { "block_id": "divider" }
                ]
            }
        });

        let payload: InteractivityPayload = serde_json::from_value(json).unwrap();
        match payload {
            InteractivityPayload::BlockActions(BlockActions::OpenModal(modal)) => {
                assert_eq!(modal.trigger_id(), "trigger-123");
                assert_eq!(modal.get_ts(), "1234567890.123456");
                assert_eq!(modal.get_log_group(), Some("/aws/lambda/test"));
                assert_eq!(modal.get_message(), Some("Error occurred"));
            }
            _ => panic!("Expected BlockActions(OpenModal)"),
        }
    }

    #[test]
    fn test_deserialize_view_submission() {
        let json = r#"{
            "type": "view_submission",
            "view": {
                "private_metadata": "encoded-metadata",
                "state": {
                    "values": {
                        "needs_notification": {
                            "needs_notification": {
                                "selected_option": {
                                    "value": "true"
                                }
                            }
                        },
                        "reason": {
                            "reason": {
                                "value": "This is important"
                            }
                        }
                    }
                }
            }
        }"#;

        let payload: InteractivityPayload = serde_json::from_str(json).unwrap();
        match payload {
            InteractivityPayload::ViewSubmission(submission) => {
                assert_eq!(submission.get_private_metadata(), "encoded-metadata");
                let (needs, reason) = submission.get_modal_values().unwrap();
                assert!(needs);
                assert_eq!(reason, Some("This is important".to_string()));
            }
            _ => panic!("Expected ViewSubmission"),
        }
    }

    #[test]
    fn test_deserialize_view_submission_false() {
        let json = r#"{
            "type": "view_submission",
            "view": {
                "private_metadata": "test",
                "state": {
                    "values": {
                        "needs_notification": {
                            "needs_notification": {
                                "selected_option": {
                                    "value": "false"
                                }
                            }
                        },
                        "reason": {
                            "reason": {
                                "value": null
                            }
                        }
                    }
                }
            }
        }"#;

        let payload: InteractivityPayload = serde_json::from_str(json).unwrap();
        match payload {
            InteractivityPayload::ViewSubmission(submission) => {
                let (needs, reason) = submission.get_modal_values().unwrap();
                assert!(!needs);
                assert_eq!(reason, None);
            }
            _ => panic!("Expected ViewSubmission"),
        }
    }
}
