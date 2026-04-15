use crate::domain::entities::{Confidence, Feedback, Language, NotificationDecision};
use crate::domain::errors::BedrockError;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, JsonSchemaDefinition, Message, OutputConfig, OutputFormat,
    OutputFormatStructure, OutputFormatType, SystemContentBlock, Tool, ToolChoice,
    ToolConfiguration, ToolInputSchema, ToolSpecification,
};
use aws_smithy_types::Document;
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use typed_builder::TypedBuilder;

const SYSTEM_PROMPT_EN: &str = include_str!("prompts/system_prompt_en.txt");
const SYSTEM_PROMPT_JA: &str = include_str!("prompts/system_prompt_ja.txt");

const TOOL_NAME: &str = "judge_needs_notification";

fn system_prompt(lang: &Language) -> &'static str {
    match lang {
        Language::En => SYSTEM_PROMPT_EN,
        Language::Ja => SYSTEM_PROMPT_JA,
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
    #[builder(default)]
    prompt_language: Language,
    #[builder(default = 3)]
    max_retries: u32,
    #[builder(default = 500)]
    base_delay_ms: u64,
}

impl Client {
    pub(crate) async fn needs_notification(
        &self,
        feedback: &[Feedback],
        message: &str,
        timestamp: &str,
    ) -> Result<NotificationDecision, BedrockError> {
        let msg = Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text(format!(
                "<feedback>{}</feedback><target_log>{}</target_log>",
                serde_json::to_string(
                    &feedback
                        .iter()
                        .map(|v| v.try_into())
                        .collect::<Result<Vec<FeedbackDto>, _>>()
                        .map_err(|e: Box<dyn std::error::Error>| {
                            BedrockError::RequestBuild {
                                detail: e.to_string(),
                            }
                        })?,
                )
                .map_err(|e| BedrockError::RequestBuild {
                    detail: e.to_string(),
                })?,
                serde_json::to_string(
                    &TargetLog::builder()
                        .message(message.to_string())
                        .timestamp(timestamp.to_string())
                        .build()
                )
                .map_err(|e| BedrockError::RequestBuild {
                    detail: e.to_string(),
                })?
            )))
            .build()
            .map_err(|e| BedrockError::RequestBuild {
                detail: e.to_string(),
            })?;

        let resp = if self.supports_structured_output() {
            self.converse_with_structured_output(msg).await?
        } else {
            self.converse_with_tool_use(msg).await?
        };

        self.parse_response(resp)
    }

    fn supports_structured_output(&self) -> bool {
        !self.model_id.contains("amazon.nova")
    }

    async fn converse_with_structured_output(
        &self,
        msg: Message,
    ) -> Result<aws_sdk_bedrockruntime::operation::converse::ConverseOutput, BedrockError> {
        let output_config = OutputConfig::builder()
            .text_format(
                OutputFormat::builder()
                    .r#type(OutputFormatType::JsonSchema)
                    .structure(OutputFormatStructure::JsonSchema(
                        JsonSchemaDefinition::builder()
                            .schema(self.make_json_schema())
                            .name("notification_decision")
                            .description("Decision on whether a notification should be sent.")
                            .build()
                            .map_err(|e| BedrockError::RequestBuild {
                                detail: e.to_string(),
                            })?,
                    ))
                    .build()
                    .map_err(|e| BedrockError::RequestBuild {
                        detail: e.to_string(),
                    })?,
            )
            .build();

        self.inner_client
            .converse()
            .model_id(&self.model_id)
            .system(SystemContentBlock::Text(
                system_prompt(&self.prompt_language).to_string(),
            ))
            .messages(msg)
            .output_config(output_config)
            .send()
            .await
            .map_err(|e| BedrockError::ConverseFailed {
                source: Box::new(e),
            })
    }

    async fn converse_with_tool_use(
        &self,
        msg: Message,
    ) -> Result<aws_sdk_bedrockruntime::operation::converse::ConverseOutput, BedrockError> {
        let tool_config = ToolConfiguration::builder()
            .tools(Tool::ToolSpec(
                ToolSpecification::builder()
                    .name(TOOL_NAME)
                    .description("Determines if notification is required.")
                    .input_schema(ToolInputSchema::Json(self.make_tool_schema()))
                    .build()
                    .map_err(|e| BedrockError::RequestBuild {
                        detail: e.to_string(),
                    })?,
            ))
            .tool_choice(ToolChoice::Tool(
                aws_sdk_bedrockruntime::types::SpecificToolChoice::builder()
                    .name(TOOL_NAME)
                    .build()
                    .map_err(|e| BedrockError::RequestBuild {
                        detail: e.to_string(),
                    })?,
            ))
            .build()
            .map_err(|e| BedrockError::RequestBuild {
                detail: e.to_string(),
            })?;

        self.inner_client
            .converse()
            .model_id(&self.model_id)
            .system(SystemContentBlock::Text(
                system_prompt(&self.prompt_language).to_string(),
            ))
            .messages(msg)
            .tool_config(tool_config)
            .send()
            .await
            .map_err(|e| BedrockError::ConverseFailed {
                source: Box::new(e),
            })
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

    fn make_tool_schema(&self) -> Document {
        Document::Object(HashMap::from([
            ("type".into(), Document::String("object".into())),
            (
                "properties".into(),
                Document::Object(HashMap::from([
                    (
                        "needs_notification".into(),
                        Document::Object(HashMap::from([
                            ("type".into(), Document::String("boolean".into())),
                            (
                                "description".into(),
                                Document::String(
                                    "If notification is necessary, set to true, otherwise set to false.".into(),
                                ),
                            ),
                        ])),
                    ),
                    (
                        "confidence".into(),
                        Document::Object(HashMap::from([
                            ("type".into(), Document::String("string".into())),
                            (
                                "description".into(),
                                Document::String("Confidence level of the decision.".into()),
                            ),
                            (
                                "enum".into(),
                                Document::Array(vec![
                                    Document::String(Confidence::High.to_string()),
                                    Document::String(Confidence::Medium.to_string()),
                                    Document::String(Confidence::Low.to_string()),
                                ]),
                            ),
                        ])),
                    ),
                    (
                        "matched_feedback_reason".into(),
                        Document::Object(HashMap::from([
                            ("type".into(), Document::String("string".into())),
                            (
                                "description".into(),
                                Document::String(
                                    "Explanation of which feedback was matched and why this decision was made.".into(),
                                ),
                            ),
                        ])),
                    ),
                ])),
            ),
            (
                "required".into(),
                Document::Array(vec![
                    Document::String("needs_notification".into()),
                    Document::String("confidence".into()),
                    Document::String("matched_feedback_reason".into()),
                ]),
            ),
        ]))
    }

    fn parse_response(
        &self,
        resp: aws_sdk_bedrockruntime::operation::converse::ConverseOutput,
    ) -> Result<NotificationDecision, BedrockError> {
        let output = resp.output.ok_or(BedrockError::NoValidBlock)?;
        let message = output
            .as_message()
            .map_err(|_| BedrockError::NoValidBlock)?;

        // ToolUse ブロックを優先的に探す（Nova は ToolUse の前後に Text を返すことがある）
        let mut tool_use_decision = None;
        let mut text_decision = None;

        for content in message.content() {
            match content {
                ContentBlock::ToolUse(tool_use) => {
                    let input =
                        tool_use
                            .input()
                            .as_object()
                            .ok_or(BedrockError::ResponseParse {
                                detail: "Input is not an object".into(),
                            })?;

                    let needs_notification = input
                        .get("needs_notification")
                        .ok_or(BedrockError::ResponseParse {
                            detail: "needs_notification not found".into(),
                        })?
                        .as_bool()
                        .ok_or(BedrockError::ResponseParse {
                            detail: "needs_notification is not a boolean".into(),
                        })?;

                    let confidence = input
                        .get("confidence")
                        .and_then(|v| v.as_string())
                        .and_then(|s| {
                            serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
                        });

                    let matched_feedback_reason = input
                        .get("matched_feedback_reason")
                        .and_then(|v| v.as_string())
                        .map(|s| s.to_string());

                    tool_use_decision = Some(
                        NotificationDecision::builder()
                            .needs_notification(needs_notification)
                            .confidence(confidence)
                            .matched_feedback_reason(matched_feedback_reason)
                            .build(),
                    );
                }
                ContentBlock::Text(text) => {
                    if let Ok(decision) = serde_json::from_str::<NotificationDecision>(text) {
                        text_decision = Some(decision);
                    }
                }
                _ => continue,
            }
        }

        tool_use_decision
            .or(text_decision)
            .ok_or(BedrockError::NoValidBlock)
    }

    pub(crate) async fn needs_notification_with_retry(
        &self,
        feedback: &[Feedback],
        message: &str,
        timestamp: &str,
    ) -> Result<NotificationDecision, BedrockError> {
        let mut last_err = None;

        for attempt in 0..=self.max_retries {
            match self.needs_notification(feedback, message, timestamp).await {
                Ok(decision) => return Ok(decision),
                Err(e) => {
                    if !e.is_retryable() {
                        return Err(e);
                    }
                    if attempt == self.max_retries {
                        last_err = Some(e);
                        break;
                    }

                    let base = self
                        .base_delay_ms
                        .saturating_mul(2u64.saturating_pow(attempt));
                    let jitter = rand::rng().random_range(0..=1000u64);
                    let delay = base + jitter;

                    tracing::warn!(
                        attempt = attempt + 1,
                        max_retries = self.max_retries,
                        delay_ms = delay,
                        error = %e,
                        "Bedrock API call failed, retrying"
                    );

                    sleep(Duration::from_millis(delay)).await;
                }
            }
        }

        // All retries exhausted — fail-safe: notify
        let err = last_err.expect("last_err must be set when all retries exhausted");
        tracing::error!(
            error = %err,
            "All Bedrock retries exhausted, falling back to fail-safe notification"
        );

        Ok(NotificationDecision::builder()
            .needs_notification(true)
            .confidence(Some(Confidence::Low))
            .matched_feedback_reason(Some(format!(
                "Fail-safe: Bedrock API error after retries: {err}"
            )))
            .build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_bedrockruntime::operation::converse::ConverseOutput;
    use aws_sdk_bedrockruntime::types::{
        ContentBlock, ConverseOutput as ConverseOutputType, Message, ToolUseBlock,
    };

    fn make_test_client() -> Client {
        let config = aws_sdk_bedrockruntime::Config::builder()
            .behavior_version(aws_sdk_bedrockruntime::config::BehaviorVersion::latest())
            .build();
        Client::builder()
            .inner_client(aws_sdk_bedrockruntime::Client::from_conf(config))
            .model_id("test-model".to_string())
            .build()
    }

    fn make_tool_use_output(
        needs_notification: bool,
        confidence: &str,
        reason: &str,
    ) -> ConverseOutput {
        let tool_input = Document::Object(HashMap::from([
            (
                "needs_notification".into(),
                Document::Bool(needs_notification),
            ),
            ("confidence".into(), Document::String(confidence.into())),
            (
                "matched_feedback_reason".into(),
                Document::String(reason.into()),
            ),
        ]));

        let tool_use = ContentBlock::ToolUse(
            ToolUseBlock::builder()
                .tool_use_id("test-id")
                .name("judge_needs_notification")
                .input(tool_input)
                .build()
                .unwrap(),
        );

        let message = Message::builder()
            .role(ConversationRole::Assistant)
            .content(tool_use)
            .build()
            .unwrap();

        ConverseOutput::builder()
            .output(ConverseOutputType::Message(message))
            .stop_reason(aws_sdk_bedrockruntime::types::StopReason::ToolUse)
            .build()
            .unwrap()
    }

    #[test]
    fn test_parse_notification_needed() {
        let client = make_test_client();
        let output = make_tool_use_output(true, "high", "Matched critical error pattern");

        let decision = client.parse_response(output).unwrap();
        assert!(decision.needs_notification());
        assert_eq!(decision.confidence(), Some(&Confidence::High));
        assert_eq!(
            decision.matched_feedback_reason(),
            Some("Matched critical error pattern")
        );
    }

    #[test]
    fn test_parse_notification_not_needed() {
        let client = make_test_client();
        let output = make_tool_use_output(false, "high", "Known benign error");

        let decision = client.parse_response(output).unwrap();
        assert!(!decision.needs_notification());
        assert_eq!(decision.confidence(), Some(&Confidence::High));
    }

    #[test]
    fn test_parse_no_valid_block() {
        let client = make_test_client();

        let message = Message::builder()
            .role(ConversationRole::Assistant)
            .content(ContentBlock::Text("Hello".to_string()))
            .build()
            .unwrap();

        let output = ConverseOutput::builder()
            .output(ConverseOutputType::Message(message))
            .stop_reason(aws_sdk_bedrockruntime::types::StopReason::EndTurn)
            .build()
            .unwrap();

        let err = client.parse_response(output).unwrap_err();
        assert!(matches!(err, BedrockError::NoValidBlock));
    }

    #[test]
    fn test_parse_missing_confidence() {
        let client = make_test_client();

        let tool_input = Document::Object(HashMap::from([
            ("needs_notification".into(), Document::Bool(true)),
            (
                "matched_feedback_reason".into(),
                Document::String("test".into()),
            ),
        ]));

        let tool_use = ContentBlock::ToolUse(
            ToolUseBlock::builder()
                .tool_use_id("test-id")
                .name("judge_needs_notification")
                .input(tool_input)
                .build()
                .unwrap(),
        );

        let message = Message::builder()
            .role(ConversationRole::Assistant)
            .content(tool_use)
            .build()
            .unwrap();

        let output = ConverseOutput::builder()
            .output(ConverseOutputType::Message(message))
            .stop_reason(aws_sdk_bedrockruntime::types::StopReason::ToolUse)
            .build()
            .unwrap();

        let decision = client.parse_response(output).unwrap();
        assert!(decision.needs_notification());
        assert_eq!(decision.confidence(), None);
    }

    #[test]
    fn test_parse_text_response() {
        let client = make_test_client();

        let message = Message::builder()
            .role(ConversationRole::Assistant)
            .content(ContentBlock::Text(
                r#"{"needs_notification":true,"confidence":"medium","matched_feedback_reason":"test"}"#.to_string(),
            ))
            .build()
            .unwrap();

        let output = ConverseOutput::builder()
            .output(ConverseOutputType::Message(message))
            .stop_reason(aws_sdk_bedrockruntime::types::StopReason::EndTurn)
            .build()
            .unwrap();

        let decision = client.parse_response(output).unwrap();
        assert!(decision.needs_notification());
        assert_eq!(decision.confidence(), Some(&Confidence::Medium));
    }
}
