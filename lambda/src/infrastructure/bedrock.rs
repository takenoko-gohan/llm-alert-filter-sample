use crate::domain::entities::{Confidence, Feedback, NotificationDecision};
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, InferenceConfiguration, JsonSchemaDefinition, Message,
    OutputConfig, OutputFormat, OutputFormatStructure, OutputFormatType, SystemContentBlock,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use std::fmt;
use typed_builder::TypedBuilder;

const SYSTEM_PROMPT_EN: &str = include_str!("prompts/system_prompt_en.txt");
const SYSTEM_PROMPT_JA: &str = include_str!("prompts/system_prompt_ja.txt");

#[derive(Debug, Clone, Default)]
pub enum PromptLanguage {
    #[default]
    En,
    Ja,
}

impl PromptLanguage {
    fn system_prompt(&self) -> &str {
        match self {
            PromptLanguage::En => SYSTEM_PROMPT_EN,
            PromptLanguage::Ja => SYSTEM_PROMPT_JA,
        }
    }
}

impl std::str::FromStr for PromptLanguage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "en" => Ok(PromptLanguage::En),
            "ja" => Ok(PromptLanguage::Ja),
            other => Err(format!(
                "unsupported PROMPT_LANGUAGE: '{}' (expected 'en' or 'ja')",
                other
            )),
        }
    }
}

impl fmt::Display for PromptLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PromptLanguage::En => write!(f, "en"),
            PromptLanguage::Ja => write!(f, "ja"),
        }
    }
}

#[derive(Serialize, TypedBuilder)]
struct FeedbackDto {
    created_at: String,
    message: String,
    needs_notification: bool,
    reason: Option<String>,
}

impl TryFrom<&Feedback> for FeedbackDto {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: &Feedback) -> Result<Self, Self::Error> {
        let created_at = DateTime::<Utc>::try_from(value.created_at().to_owned())?;
        Ok(Self::builder()
            .created_at(created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .message(value.message().to_string())
            .needs_notification(value.needs_notification())
            .reason(value.reason().map(|r| r.to_string()))
            .build())
    }
}

#[derive(Serialize, TypedBuilder)]
struct TargetLog {
    message: String,
    timestamp: String,
}

#[derive(TypedBuilder)]
pub struct Client {
    inner_client: aws_sdk_bedrockruntime::Client,
    model_id: String,
    top_p: f32,
    #[builder(default)]
    prompt_language: PromptLanguage,
}

impl Client {
    pub(crate) async fn needs_notification(
        &self,
        feedback: &[Feedback],
        message: &str,
        timestamp: &str,
    ) -> Result<NotificationDecision, Box<dyn std::error::Error>> {
        let msg = Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text(format!(
                "<feedback>{}</feedback><target_log>{}</target_log>",
                serde_json::to_string(&feedback.iter().map(|v| v.try_into()).collect::<Result<
                    Vec<FeedbackDto>,
                    _,
                >>(
                )?)?,
                serde_json::to_string(
                    &TargetLog::builder()
                        .message(message.to_string())
                        .timestamp(timestamp.to_string())
                        .build()
                )?
            )))
            .build()?;

        let inference_config = InferenceConfiguration::builder().top_p(self.top_p).build();
        let output_config = OutputConfig::builder()
            .text_format(
                OutputFormat::builder()
                    .r#type(OutputFormatType::JsonSchema)
                    .structure(OutputFormatStructure::JsonSchema(
                        JsonSchemaDefinition::builder()
                            .schema(self.make_json_schema())
                            .name("notification_decision")
                            .description("Decision on whether a notification should be sent.")
                            .build()?,
                    ))
                    .build()?,
            )
            .build();

        let resp = self
            .inner_client
            .converse()
            .model_id(&self.model_id)
            .system(SystemContentBlock::Text(self.system_prompt().to_string()))
            .messages(msg)
            .inference_config(inference_config)
            .output_config(output_config)
            .send()
            .await?;

        self.parse_response(resp)
    }

    fn make_json_schema(&self) -> String {
        json!({
            "type": "object",
            "properties": {
                "needs_notification": {
                    "type": "boolean",
                    "description": "If notification is necessary, set to true, otherwise set to false."
                },
                "confidence": {
                    "type": "string",
                    "description": "Confidence level of the decision.",
                    "enum": [
                        Confidence::High.to_string(),
                        Confidence::Medium.to_string(),
                        Confidence::Low.to_string(),
                    ]
                },
                "matched_feedback_reason": {
                    "type": "string",
                    "description": "Explanation of which feedback was matched and why this decision was made."
                }
            },
            "required": ["needs_notification", "confidence", "matched_feedback_reason"],
            "additionalProperties": false
        })
        .to_string()
    }

    fn parse_response(
        &self,
        resp: aws_sdk_bedrockruntime::operation::converse::ConverseOutput,
    ) -> Result<NotificationDecision, Box<dyn std::error::Error>> {
        let output = resp.output.ok_or("Output not found")?;
        let message = output.as_message().map_err(|_| "Output is not a message")?;

        for content in message.content() {
            if let ContentBlock::Text(text) = content {
                let decision: NotificationDecision = serde_json::from_str(text)?;
                return Ok(decision);
            }
        }

        Err("No text block found in response".into())
    }

    fn system_prompt(&self) -> &str {
        self.prompt_language.system_prompt()
    }
}
