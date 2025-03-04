use crate::application::services::CollectionService;
use crate::interface::payloads::{BlockActions, InteractivityPayload};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Form;
use std::collections::HashMap;

pub(crate) async fn add_feedback_handler(
    State(state): State<CollectionService>,
    req: Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let payload = match req.get("payload") {
        Some(payload) => payload,
        None => {
            tracing::warn!("Payload not found");
            return StatusCode::BAD_REQUEST;
        }
    };

    let payload: InteractivityPayload = match serde_json::from_str(payload) {
        Ok(payload) => payload,
        Err(e) => {
            tracing::warn!("Failed to parse payload: {:?}", e);
            return StatusCode::BAD_REQUEST;
        }
    };

    match payload {
        InteractivityPayload::BlockActions(payload) => match payload {
            BlockActions::OpenModal(payload) => {
                tracing::info!("Called open modal");

                let ts = payload.get_ts().to_string();
                let Some(log_group) = payload.get_log_group() else {
                    tracing::warn!("Log group not found");
                    return StatusCode::BAD_REQUEST;
                };
                let Some(message) = payload.get_message() else {
                    tracing::warn!("Message not found");
                    return StatusCode::BAD_REQUEST;
                };

                match state
                    .open_modal(
                        payload.trigger_id(),
                        ts,
                        log_group.to_string(),
                        message.to_string(),
                    )
                    .await
                {
                    Ok(_) => StatusCode::OK,
                    Err(e) => {
                        tracing::error!("Failed to open modal: {:?}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                }
            }
            BlockActions::Other(_) => StatusCode::OK,
        },
        InteractivityPayload::ViewSubmission(payload) => {
            tracing::info!("Called view submission");

            let private_metadata = payload.get_private_metadata();
            let (needs_notification, reason) = match payload.get_modal_values() {
                Ok(values) => values,
                Err(e) => {
                    tracing::warn!("Failed to get modal values: {:?}", e);
                    return StatusCode::BAD_REQUEST;
                }
            };

            match state
                .add_feedback(private_metadata, needs_notification, reason)
                .await
            {
                Ok(_) => StatusCode::OK,
                Err(e) => {
                    tracing::error!("Failed to add feedback: {:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            }
        }
    }
}
