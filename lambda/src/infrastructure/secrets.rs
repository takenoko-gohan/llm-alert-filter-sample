use std::collections::HashMap;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct Client {
    inner: aws_sdk_secretsmanager::Client,
}

impl Client {
    pub async fn load_secrets(
        &self,
        secret_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let resp = self
            .inner
            .get_secret_value()
            .secret_id(secret_id)
            .send()
            .await?;

        let secrets = resp
            .secret_string
            .ok_or(format!("Secret not found: {}", secret_id))?;

        for (k, v) in serde_json::from_str::<HashMap<String, String>>(&secrets)? {
            std::env::set_var(k, v);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_secretsmanager::operation::get_secret_value::GetSecretValueOutput;
    use aws_smithy_mocks::{mock, mock_client};

    #[tokio::test]
    async fn test_load_secrets_sets_env_vars() {
        let rule = mock!(aws_sdk_secretsmanager::Client::get_secret_value).then_output(|| {
            GetSecretValueOutput::builder()
                .secret_string(r#"{"__MOCK_SECRET_A":"value_a","__MOCK_SECRET_B":"value_b"}"#)
                .build()
        });
        let mock_sdk = mock_client!(aws_sdk_secretsmanager, &[&rule]);
        let client = Client::builder().inner(mock_sdk).build();

        let result = client.load_secrets("test-secret").await;
        assert!(result.is_ok());
        assert_eq!(std::env::var("__MOCK_SECRET_A").unwrap(), "value_a");
        assert_eq!(std::env::var("__MOCK_SECRET_B").unwrap(), "value_b");

        std::env::remove_var("__MOCK_SECRET_A");
        std::env::remove_var("__MOCK_SECRET_B");
    }

    #[tokio::test]
    async fn test_load_secrets_missing_secret_string() {
        let rule = mock!(aws_sdk_secretsmanager::Client::get_secret_value)
            .then_output(|| GetSecretValueOutput::builder().build());
        let mock_sdk = mock_client!(aws_sdk_secretsmanager, &[&rule]);
        let client = Client::builder().inner(mock_sdk).build();

        let result = client.load_secrets("missing-secret").await;
        assert!(result.is_err());
    }
}
