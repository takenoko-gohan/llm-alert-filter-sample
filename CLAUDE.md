# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A serverless application that uses Amazon Bedrock to evaluate CloudWatch Logs error logs and determine whether to send Slack notifications, based on past user feedback history. Built with Rust Lambda + AWS CDK (TypeScript).

## Build & Development Commands

### Rust Lambda

```bash
# Build (requires cargo-lambda)
cd lambda && cargo lambda build --release --arm64

# Test
cd lambda && cargo test

# Clippy
cd lambda && cargo clippy

# Format
cd lambda && cargo fmt
```

### CDK (TypeScript)

```bash
# Install dependencies
cd cdk && npm ci

# Synth (compile + generate CloudFormation template)
cd cdk && npx cdk synth

# Test
cd cdk && npm test

# Lint & format
cd cdk && npm run check        # biome check
cd cdk && npm run check:fix    # biome check --write
cd cdk && npm run fmt          # biome format --write
cd cdk && npm run lint         # biome lint
cd cdk && npm run lint:fix     # biome lint --write

# CDK deploy
cd cdk && npx cdk deploy
```

## Architecture

Clean Architecture (Domain → Application → Infrastructure → Interface) combined with Ports & Adapters (Hexagonal) pattern.

### Lambda Binaries (2)

- **notifier** (`lambda/src/bin/notifier.rs`): Triggered by CloudWatch Logs subscription filters. Evaluates error logs via Bedrock and posts to Slack if notification is needed.
- **collector** (`lambda/src/bin/collector.rs`): Function URL endpoint (Axum) that receives Slack button interactions and modal submissions. Stores user feedback in DynamoDB.

### Layer Structure (`lambda/src/`)

- **domain/**: Feedback entity, FeedbackId/Timestamp value objects, FeedbackRepository trait, Language/Confidence/NotificationDecision types
- **application/ports.rs**: NotificationJudge trait (Bedrock judgment abstraction), AlertNotifier trait (Slack operations abstraction)
- **application/config.rs**: NotifierConfig / CollectorConfig (environment variable validation)
- **application/services.rs**: NotificationService (log evaluation → notification), CollectionService (feedback collection) — generic over port traits
- **infrastructure/**: Bedrock Converse API, Slack API, Secrets Manager, DynamoDB repository implementation, i18n messages
- **interface/**: Axum router, Slack request signature verification middleware, handlers

### Bedrock Integration

- Converse API supports two response modes:
  - **Structured Output** (JSON Schema): For non-Nova models (e.g., Claude)
  - **Tool Use**: Fallback for Amazon Nova models
- `parse_response()` prioritizes ToolUse blocks, falls back to Text JSON for unified parsing
- Response schema: `needs_notification` (bool), `confidence` ("high"/"medium"/"low"), `matched_feedback_reason` (string)
- Retry: Exponential backoff with jitter (only ThrottlingException / ServiceUnavailableException are retryable)
- Fail-safe: Returns `needs_notification=true, confidence=Low` when all retries are exhausted
- Service-level fail-safe: Overrides suppression to notify when confidence is not High (false-negative avoidance)
- Log messages with 80%+ similarity are treated as the same class of log
- When feedback conflicts, the most recent feedback takes precedence
- System prompts are external files in English and Japanese (`infrastructure/prompts/`)

### Infrastructure (CDK)

- Region: Environment-agnostic stack. Resolved at deploy time from the AWS CLI environment (`AWS_REGION` / `AWS_DEFAULT_REGION` / AWS profile).
- DynamoDB: PAY_PER_REQUEST, GSI `log_group_index`
- Lambda: ARM_64, 128MB (notifier: 120s / collector: 30s timeout)
- Secrets Manager: Manages Slack token and signing secret
- CfnParameters: BedrockModelId, AppLanguage, MaxRetries, BaseDelayMs, SlackChannelId

### Testing

- Service layer: Manual mocks (MockRepo, MockJudge, MockNotifier) for business logic testing
- Infrastructure layer: `aws-smithy-mocks` for AWS SDK mock testing (DynamoDB, Bedrock retry, Secrets Manager)
- CDK: `jest` + `jest.mock("cargo-lambda-cdk")` for build-free testing

### CI/CD

- GitHub Actions: actionlint + zizmor (workflow validation), Rust check, CDK check — 3 parallel jobs
- Renovate: Daily updates for npm / cargo / GitHub Actions dependencies, lock file maintenance enabled
