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
