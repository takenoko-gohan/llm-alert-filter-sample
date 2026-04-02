use crate::domain::entities::Feedback;
use aws_sdk_bedrockruntime::operation::converse::ConverseOutput;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, InferenceConfiguration, Message, SystemContentBlock, Tool,
    ToolConfiguration, ToolInputSchema, ToolSpecification,
};
use aws_smithy_types::Document;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
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
    ) -> Result<bool, Box<dyn std::error::Error>> {
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
        let tool_config = ToolConfiguration::builder()
            .tools(Tool::ToolSpec(
                ToolSpecification::builder()
                    .name("judge_needs_notification")
                    .description("Determines if notification is required.")
                    .input_schema(ToolInputSchema::Json(self.make_tool_schema()))
                    .build()?,
            ))
            .build()?;

        let resp = self
            .inner_client
            .converse()
            .model_id(&self.model_id)
            .system(SystemContentBlock::Text(self.system_prompt().to_string()))
            .messages(msg)
            .inference_config(inference_config)
            .tool_config(tool_config)
            .send()
            .await?;

        self.get_converse_output(resp)
    }

    fn make_tool_schema(&self) -> Document {
        Document::Object(HashMap::<String, Document>::from([
            ("type".into(), Document::String("object".into())),
            (
                "properties".into(),
                Document::Object(HashMap::<String, Document>::from([
                    (
                        "needs_notification".into(),
                        Document::Object(HashMap::<String, Document>::from([
                            ("type".into(), Document::String("boolean".into())),
                            (
                                "description".into(),
                                Document::String("If notification is necessary, set to true, otherwise set to false.".into()),
                            ),
                        ])),
                    ),
                ])),
            ),
            (
                "required".into(),
                Document::Array(vec![
                    Document::String("needs_notification".into()),
                ]),
            ),
        ]))
    }

    fn get_converse_output(
        &self,
        resp: ConverseOutput,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let output = resp.output.ok_or("Output not found")?;

        let mut needs_notification = None;
        for content in output
            .as_message()
            .map_err(|_| "Output is not a message")?
            .content()
        {
            match content.as_tool_use() {
                Ok(tool_use) => {
                    let result = tool_use
                        .input()
                        .as_object()
                        .ok_or("Input is not an object")?
                        .get("needs_notification")
                        .ok_or("needs_notification not found")?
                        .as_bool()
                        .ok_or("needs_notification is not a boolean")?;

                    needs_notification = Some(result);
                }
                Err(_) => continue,
            }
        }

        Ok(needs_notification.ok_or("Failed not found toolUse")?)
    }

    fn system_prompt(&self) -> &str {
        self.prompt_language.system_prompt()
    }
}
