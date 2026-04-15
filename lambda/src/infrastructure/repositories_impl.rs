use crate::domain::entities::Feedback;
use crate::domain::errors::DynamoDbError;
use crate::domain::repositories::FeedbackRepository;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use serde_dynamo::{from_items, to_item};
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct FeedbackRepositoryImpl {
    client: Client,
    table_name: String,
}

impl FeedbackRepository for FeedbackRepositoryImpl {
    async fn add_feedback(&self, feedback: Feedback) -> Result<(), DynamoDbError> {
        let item = to_item(feedback).map_err(|e| DynamoDbError::Put(e.into()))?;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| DynamoDbError::Put(e.into()))?;

        Ok(())
    }

    async fn list_feedback_by_log_group(
        &self,
        log_group: &str,
    ) -> Result<Vec<Feedback>, DynamoDbError> {
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
                .await
                .map_err(|e| DynamoDbError::Query {
                    log_group: log_group.to_string(),
                    source: e.into(),
                })?;

            if let Some(items) = resp.items {
                let feedback: Vec<Feedback> =
                    from_items(items).map_err(|e| DynamoDbError::Deserialize(e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repositories::FeedbackRepository;
    use crate::domain::value_objects::{FeedbackId, Timestamp};
    use aws_sdk_dynamodb::operation::put_item::PutItemOutput;
    use aws_sdk_dynamodb::operation::query::QueryOutput;
    use aws_sdk_dynamodb::types::AttributeValue;
    use aws_smithy_mocks::{mock, mock_client, RuleMode};
    use std::collections::HashMap;

    fn make_feedback() -> Feedback {
        Feedback::builder()
            .id(FeedbackId::new())
            .created_at(Timestamp::new())
            .log_group("/aws/lambda/test".to_string())
            .message("Error: test".to_string())
            .needs_notification(false)
            .reason(Some("known issue".to_string()))
            .build()
    }

    #[tokio::test]
    async fn test_add_feedback_success() {
        let rule = mock!(aws_sdk_dynamodb::Client::put_item)
            .then_output(|| PutItemOutput::builder().build());
        let client = mock_client!(aws_sdk_dynamodb, &[&rule]);
        let repo = FeedbackRepositoryImpl::builder()
            .client(client)
            .table_name("test-table".to_string())
            .build();

        assert!(repo.add_feedback(make_feedback()).await.is_ok());
    }

    #[tokio::test]
    async fn test_list_feedback_returns_items() {
        let feedback = make_feedback();
        let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(feedback).unwrap();

        let rule = mock!(aws_sdk_dynamodb::Client::query).then_output(move || {
            QueryOutput::builder()
                .set_items(Some(vec![item.clone()]))
                .build()
        });
        let client = mock_client!(aws_sdk_dynamodb, &[&rule]);
        let repo = FeedbackRepositoryImpl::builder()
            .client(client)
            .table_name("test-table".to_string())
            .build();

        let result = repo.list_feedback_by_log_group("/aws/lambda/test").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_list_feedback_empty() {
        let rule =
            mock!(aws_sdk_dynamodb::Client::query).then_output(|| QueryOutput::builder().build());
        let client = mock_client!(aws_sdk_dynamodb, &[&rule]);
        let repo = FeedbackRepositoryImpl::builder()
            .client(client)
            .table_name("test-table".to_string())
            .build();

        let result = repo.list_feedback_by_log_group("/aws/lambda/test").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_feedback_pagination() {
        let fb1 = make_feedback();
        let fb2 = make_feedback();
        let item1: HashMap<String, AttributeValue> = serde_dynamo::to_item(fb1).unwrap();
        let item2: HashMap<String, AttributeValue> = serde_dynamo::to_item(fb2).unwrap();

        let page1 = mock!(aws_sdk_dynamodb::Client::query).then_output(move || {
            QueryOutput::builder()
                .set_items(Some(vec![item1.clone()]))
                .set_last_evaluated_key(Some(HashMap::from([(
                    "id".to_string(),
                    AttributeValue::S("cursor".to_string()),
                )])))
                .build()
        });
        let page2 = mock!(aws_sdk_dynamodb::Client::query).then_output(move || {
            QueryOutput::builder()
                .set_items(Some(vec![item2.clone()]))
                .build()
        });

        let client = mock_client!(aws_sdk_dynamodb, RuleMode::Sequential, &[&page1, &page2]);
        let repo = FeedbackRepositoryImpl::builder()
            .client(client)
            .table_name("test-table".to_string())
            .build();

        let result = repo.list_feedback_by_log_group("/aws/lambda/test").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }
}
