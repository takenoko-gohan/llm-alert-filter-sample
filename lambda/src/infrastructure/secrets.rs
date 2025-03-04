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
