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
