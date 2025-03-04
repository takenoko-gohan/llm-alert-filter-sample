use crate::domain::entities::Feedback;
use crate::domain::repositories::FeedbackRepository;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use serde_dynamo::{from_items, to_item};
use std::error::Error;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct FeedbackRepositoryImpl {
    client: Client,
    table_name: String,
}

impl FeedbackRepository for FeedbackRepositoryImpl {
    async fn add_feedback(&self, feedback: Feedback) -> Result<(), Box<dyn Error>> {
        let item = to_item(feedback)?;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await?;

        Ok(())
    }

    async fn list_feedback_by_log_group(
        &self,
        log_group: &str,
    ) -> Result<Vec<Feedback>, Box<dyn Error>> {
        let mut results = vec![];
        let mut exclusive_start_key = None;

        loop {
            let resp = self
                .client
                .query()
                .table_name(&self.table_name)
                .index_name("log_group_index")
                .key_condition_expression("log_group = :log_group")
                .expression_attribute_values(":log_group", AttributeValue::S(log_group.to_string()))
                .set_exclusive_start_key(exclusive_start_key)
                .send()
                .await?;

            if let Some(items) = resp.items {
                let feedback: Vec<Feedback> = from_items(items)?;
                results.extend(feedback);

                match &resp.last_evaluated_key {
                    Some(last_evaluated_key) => {
                        exclusive_start_key = Some(last_evaluated_key.clone());
                    }
                    None => {
                        break;
                    }
                }
            } else {
                break;
            }
        }

        Ok(results)
    }
}
