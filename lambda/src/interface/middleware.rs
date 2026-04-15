use crate::util::now_timestamp;
use axum::body::Body;
use axum::http::request::Parts;
use axum::http::{Request, Response, StatusCode};
use futures_util::future::BoxFuture;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::str::from_utf8;
use tower_http::auth::{AsyncAuthorizeRequest, AsyncRequireAuthorizationLayer};
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Auth {
    signing_secret: String,
}

impl AsyncAuthorizeRequest<Body> for Auth {
    type RequestBody = Body;
    type ResponseBody = Body;
    type Future = BoxFuture<'static, Result<Request<Body>, Response<Self::ResponseBody>>>;

    fn authorize(&mut self, request: Request<Body>) -> Self::Future {
        let signing_secret = self.signing_secret.clone();
        Box::pin(async move {
            let (parts, body) = request.into_parts();
            let bytes = axum::body::to_bytes(body, usize::MAX)
                .await
                .map_err(|_| make_error_response())?;

            let check_result = match check_signature(&parts, bytes.as_ref(), &signing_secret).await
            {
                Ok(result) => result,
                Err(e) => {
                    tracing::warn!("Failed to check signature: {:?}", e);
                    return Err(make_error_response());
                }
            };

            if check_result {
                let body = Body::from(bytes);
                let request = Request::from_parts(parts, body);
                Ok(request)
            } else {
                Err(make_error_response())
            }
        })
    }
}

async fn check_signature(
    parts: &Parts,
    body: &[u8],
    signing_secret: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let signature = parts
        .headers
        .get("X-Slack-Signature")
        .ok_or("no signature")?
        .to_str()?;
    let timestamp = parts
        .headers
        .get("X-Slack-Request-Timestamp")
        .ok_or("no timestamp")?
        .to_str()?;

    if now_timestamp() - timestamp.parse::<i64>()? > 60 * 5 {
        return Err("timestamp is too old".into());
    }

    let mut mac = Hmac::<Sha256>::new_from_slice(signing_secret.as_bytes())?;

    let hash = format!("v0:{}:{}", timestamp, from_utf8(body)?,);

    mac.update(hash.as_bytes());

    let result = format!("v0={:x}", mac.finalize().into_bytes());

    Ok(signature == result)
}

fn make_error_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(Body::empty())
        .unwrap()
}

pub fn create_auth_layer(signing_secret: String) -> AsyncRequireAuthorizationLayer<Auth> {
    AsyncRequireAuthorizationLayer::new(Auth::builder().signing_secret(signing_secret).build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    fn make_signature(secret: &str, timestamp: &str, body: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        let hash = format!("v0:{}:{}", timestamp, body);
        mac.update(hash.as_bytes());
        format!("v0={:x}", mac.finalize().into_bytes())
    }

    fn make_parts(signature: &str, timestamp: &str) -> Parts {
        let request = Request::builder()
            .header("X-Slack-Signature", signature)
            .header("X-Slack-Request-Timestamp", timestamp)
            .body(())
            .unwrap();
        request.into_parts().0
    }

    #[tokio::test]
    async fn test_valid_signature() {
        let secret = "test_secret";
        let body = "test_body";
        let timestamp = &now_timestamp().to_string();
        let signature = make_signature(secret, timestamp, body);
        let parts = make_parts(&signature, timestamp);

        let result = check_signature(&parts, body.as_bytes(), secret)
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_invalid_signature() {
        let secret = "test_secret";
        let body = "test_body";
        let timestamp = &now_timestamp().to_string();
        let parts = make_parts("v0=invalid_signature", timestamp);

        let result = check_signature(&parts, body.as_bytes(), secret)
            .await
            .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_expired_timestamp() {
        let secret = "test_secret";
        let body = "test_body";
        let expired_timestamp = (now_timestamp() - 600).to_string();
        let signature = make_signature(secret, &expired_timestamp, body);
        let parts = make_parts(&signature, &expired_timestamp);

        let result = check_signature(&parts, body.as_bytes(), secret).await;
        assert!(result.is_err());
    }
}
