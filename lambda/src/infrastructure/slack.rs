use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use typed_builder::TypedBuilder;

#[derive(Debug, Serialize, Deserialize, TypedBuilder)]
pub(crate) struct PrivateMetadata {
    ts: String,
    log_group: String,
    message: String,
}

impl PrivateMetadata {
    pub(crate) fn ts(&self) -> &str {
        &self.ts
    }

    pub(crate) fn log_group(&self) -> &str {
        &self.log_group
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn encode_base64(&self) -> Result<String, Box<dyn std::error::Error>> {
        let json = serde_json::to_string(self)?;

        let mut gz_encoder = GzEncoder::new(Vec::new(), Compression::default());
        gz_encoder.write_all(json.as_bytes())?;
        let compressed = gz_encoder.finish()?;

        Ok(BASE64_STANDARD.encode(compressed.as_slice()))
    }
}

impl TryFrom<&str> for PrivateMetadata {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let compressed = BASE64_STANDARD.decode(value.as_bytes())?;

        let mut gz_decoder = flate2::read::GzDecoder::new(compressed.as_slice());
        let mut json = String::new();
        gz_decoder.read_to_string(&mut json)?;

        Ok(serde_json::from_str(&json)?)
    }
}

#[derive(Deserialize)]
struct Response {
    ok: bool,
    error: Option<String>,
    response_metadata: Option<Value>,
}

#[derive(Clone, TypedBuilder)]
pub struct Client {
    inner_client: reqwest::Client,
    token: String,
}

const BASE_URL: &str = "https://slack.com/api";

impl Client {
    pub(crate) async fn post_alert(
        &self,
        channel_id: &str,
        log_group: &str,
        message: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/chat.postMessage", BASE_URL);

        let mut blocks = match self.make_base_alert_message(log_group, message).as_array() {
            Some(blocks) => blocks.to_vec(),
            None => vec![],
        };
        blocks.push(serde_json::json!({
            "type": "actions",
            "block_id": "feedback_button",
            "elements": [
                {
                    "type": "button",
                    "text": {
                        "type": "plain_text",
                        "text": "フィードバック"
                    },
                    "style": "primary",
                    "value": "send_feedback",
                    "action_id": "open_modal"
                }
            ]
        }));

        let resp = self
            .inner_client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&serde_json::json!({
                "channel": channel_id,
                "blocks": blocks
            }))
            .send()
            .await?;

        if resp.status().is_success() {
            let resp: Response = resp.json().await?;
            if resp.ok {
                Ok(())
            } else {
                Err(format!(
                    "Failed to post message: {}",
                    resp.error.unwrap_or("Unknown".into())
                )
                .into())
            }
        } else {
            Err(format!("Failed to post message: {}", resp.text().await?).into())
        }
    }

    pub(crate) async fn close_feedback_button(
        &self,
        channel_id: &str,
        ts: &str,
        log_group: &str,
        message: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/chat.update", BASE_URL);

        let mut blocks = match self.make_base_alert_message(log_group, message).as_array() {
            Some(blocks) => blocks.to_vec(),
            None => vec![],
        };
        blocks.push(serde_json::json!({
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": "_フィードバック済み_"
            }
        }));

        let resp = self
            .inner_client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&serde_json::json!({
                "channel": channel_id,
                "ts": ts,
                "blocks": blocks
            }))
            .send()
            .await?;

        if resp.status().is_success() {
            let resp: Response = resp.json().await?;
            if resp.ok {
                Ok(())
            } else {
                Err(format!(
                    "Failed to close feedback button: {}",
                    resp.error.unwrap_or("Unknown".into())
                )
                .into())
            }
        } else {
            Err(format!("Failed to close feedback button: {}", resp.text().await?).into())
        }
    }

    fn make_base_alert_message(&self, log_group: &str, message: &str) -> Value {
        serde_json::json!([
            {
                "type": "header",
                "block_id": "header",
                "text": {
                    "type": "plain_text",
                    "text": ":rotating_light: エラーが発生しました :rotating_light:",
                    "emoji": true
                }
            },
            {
                "type": "section",
                "block_id": "log_group_header",
                "text": {
                    "type": "mrkdwn",
                    "text": "*CloudWatch Logs ロググループ*"
                }
            },
            {
                "type": "section",
                "block_id": "log_group",
                "text": {
                    "type": "plain_text",
                    "text": log_group
                }
            },
            {
                "type": "section",
                "block_id": "message_header",
                "text": {
                    "type": "mrkdwn",
                    "text": "*ログメッセージ*"
                }
            },
            {
                "type": "section",
                "block_id": "message",
                "text": {
                    "type": "plain_text",
                    "text": message
                }
            },
            {
                "type": "divider",
                "block_id": "divider"
            }
        ])
    }

    pub(crate) async fn open_modal(
        &self,
        trigger_id: &str,
        private_metadata: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/views.open", BASE_URL);

        let view = make_feedback_view(private_metadata);

        let resp = self
            .inner_client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&serde_json::json!({
                "trigger_id": trigger_id,
                "view": view,
            }))
            .send()
            .await?;

        if resp.status().is_success() {
            let resp: Response = resp.json().await?;
            if resp.ok {
                Ok(())
            } else {
                Err(format!(
                    "Failed to open modal: {{ error: \"{}\", response_metadata: \"{}\" }}",
                    resp.error.unwrap_or_default(),
                    resp.response_metadata.unwrap_or_default(),
                )
                .into())
            }
        } else {
            Err(format!("Failed to open modal: {}", resp.text().await?).into())
        }
    }
}

fn make_feedback_view(private_metadata: &str) -> Value {
    serde_json::json!({
        "type": "modal",
        "callback_id": "send_feedback",
        "private_metadata": private_metadata,
        "title": {
            "type": "plain_text",
            "text": "フィードバック"
        },
        "blocks": [
            {
                "type": "section",
                "block_id": "needs_notification",
                "text": {
                    "type": "plain_text",
                    "text": "通知が必要ですか？"
                },
                "accessory": {
                    "type": "static_select",
                    "action_id": "needs_notification",
                    "initial_option": {
                        "text": {
                            "type": "plain_text",
                            "text": "不要"
                        },
                        "value": "false"
                    },
                    "options": [
                        {
                            "text": {
                                "type": "plain_text",
                                "text": "不要"
                            },
                            "value": "false"
                        },
                        {
                            "text": {
                                "type": "plain_text",
                                "text": "必要"
                            },
                            "value": "true"
                        },
                    ]
                }
            },
            {
                "type": "input",
                "block_id": "reason",
                "label": {
                    "type": "plain_text",
                    "text": "理由"
                },
                "element": {
                    "type": "plain_text_input",
                    "action_id": "reason",
                    "multiline": true
                },
                "optional": true
            },
        ],
        "close": {
            "type": "plain_text",
            "text": "キャンセル"
        },
        "submit": {
            "type": "plain_text",
            "text": "送信"
        },
    })
}
