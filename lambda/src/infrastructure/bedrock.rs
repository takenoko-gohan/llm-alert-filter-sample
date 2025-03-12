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
use typed_builder::TypedBuilder;

const SYSTEM_PROMPT: &str = "
<role>
You are a log monitor.
</role>
<question>
Refer to the list of past notification feedback (`feedback`) to determine whether a notification is required for the currently occurring error log (`target_log`).
</question>
<data_info>
- feedback: A list of feedback regarding notifications from the operator
  - created_at: The date and time when the feedback was added
  - message: The content of the error log that received feedback
  - needs_notification: Whether a notification is required (`true` means required, `false` means not required)
  - reason: Reasons for necessity or non-necessity (optional)
- target_log: The error log subject to the decision
  - message: The content of the log
  - timestamp: The date and time when the log was generated
</data_info>
<rule>
- Think step-by-step.
- Make a decision only if sufficient inference can be drawn from the feedback content; if not, always return `true`.
- Treat feedback as similar if the `message` in both `feedback` and `target_log` matches 80% or more.
- If the referenced `feedback` for inference contains a `reason`, take its content into account.
- If similar feedback contradict each other, prioritize the feedback with the most recent `created_at` timestamp.
</rule>
";

#[derive(Serialize, TypedBuilder)]
struct FeedbackDto {
    created_at: String,
    message: String,
    needs_notification: bool,
    reason: Option<String>,
}

impl TryFrom<Feedback> for FeedbackDto {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: Feedback) -> Result<Self, Self::Error> {
        let created_at = DateTime::<Utc>::try_from(value.created_at().to_owned())?;
        Ok(Self::builder()
            .created_at(created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .message(value.message().into())
            .needs_notification(value.needs_notification())
            .reason(value.reason().map(|r| r.into()))
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
    temperature: f32,
}

impl Client {
    pub(crate) async fn needs_notification(
        &self,
        feedback: Vec<Feedback>,
        message: String,
        timestamp: String,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let msg = Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text(format!(
                "<feedback>{}</feedback><target_log>{}</target_log>",
                serde_json::to_string(
                    &feedback
                        .into_iter()
                        .map(|v| v.try_into())
                        .collect::<Result<Vec<FeedbackDto>, _>>()?
                )?,
                serde_json::to_string(
                    &TargetLog::builder()
                        .message(message)
                        .timestamp(timestamp)
                        .build()
                )?
            )))
            .build()?;

        let inference_config = InferenceConfiguration::builder()
            .top_p(self.top_p)
            .temperature(self.temperature)
            .build();
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
            .system(SystemContentBlock::Text(SYSTEM_PROMPT.into()))
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
}
