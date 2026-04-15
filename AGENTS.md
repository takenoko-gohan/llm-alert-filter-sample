# AGENTS.md

## languages

Please use Japanese only.

## Overview

This repository contains a serverless sample that uses Amazon Bedrock to filter CloudWatch Logs error alerts before posting to Slack.

- `lambda/`: Rust Lambda application code
- `cdk/`: AWS CDK app written in TypeScript
- `slack_app/`: Slack app manifest
- `docs/`: documentation assets
- `.github/`: CI workflows and Renovate config

The system has two Lambda binaries:

- `notifier`: triggered by CloudWatch Logs subscription filters, evaluates whether an error should be sent to Slack
- `collector`: receives Slack feedback via Function URL and stores user feedback in DynamoDB

## Working Guidelines

- Keep changes scoped and consistent with the existing architecture.
- Prefer fixing root causes over layering on ad-hoc workarounds.
- Do not commit generated artifacts from `cdk/cdk.out/`, `cdk/node_modules/`, or `lambda/target/`.
- If you change infrastructure, deployment steps, or developer workflows, update both `README.md` and `README.ja.md` as needed.
- The default region is `us-east-1` but is configurable via CDK context or environment variables (`CDK_DEFAULT_REGION`, `AWS_DEFAULT_REGION`).

## Project Structure

### Rust Lambda

The Rust code follows a layered structure with port/adapter abstraction under `lambda/src/`:

- `domain/`: entities (`Feedback`, `NotificationDecision`, `Confidence`, `Language`), repository traits, value objects
- `application/ports.rs`: port traits (`NotificationJudge`, `AlertNotifier`) abstracting Bedrock and Slack
- `application/config.rs`: `NotifierConfig` / `CollectorConfig` with env var validation
- `application/services.rs`: generic services (`NotificationService<R, B, S>`, `CollectionService<R, S>`) depending on port traits
- `infrastructure/`: Bedrock Converse API (structured output + tool use), Slack API, Secrets Manager, DynamoDB repository, i18n messages, system prompts
- `interface/`: Axum routers, handlers, payloads, and Slack signature verification middleware
- `bin/`: Lambda entrypoints (`collector.rs`, `notifier.rs`)

When modifying Rust code:

- Keep domain logic free from AWS or HTTP-specific concerns.
- Put external service integrations in `infrastructure/`.
- Put routing, request parsing, and middleware concerns in `interface/`.
- Keep service orchestration in `application/services.rs`.
- Add new external dependency abstractions as port traits in `application/ports.rs`.

### CDK

The infrastructure code lives in `cdk/`.

- Main stack: `cdk/lib/llm-alert-filter-stack.ts`
- Entry point: `cdk/bin/cdk.ts`
- CfnParameters: `BedrockModelId`, `AppLanguage`, `MaxRetries`, `BaseDelayMs`, `SlackChannelId`

When modifying CDK code:

- Keep logical resource naming aligned with the existing stack.
- Avoid changing deployed resource names unless explicitly required.
- Be careful with changes that affect Bedrock model configuration, Function URLs, IAM permissions, or DynamoDB schema.
- CfnParameter constraints (allowedValues, min/max) must be consistent with Lambda-side validation in `application/config.rs`.

## Common Commands

### Rust

```bash
cd lambda && cargo test
cd lambda && cargo fmt
cd lambda && cargo clippy
# requires cargo-lambda: https://www.cargo-lambda.info/
cd lambda && cargo lambda build --release --arm64
```

### CDK

```bash
cd cdk && npm ci
cd cdk && npx cdk synth
cd cdk && npm test
cd cdk && npm run check
cd cdk && npm run check:fix   # biome check --write (auto-fix)
cd cdk && npm run lint:fix    # biome lint --write (auto-fix)
cd cdk && npm run fmt         # biome format --write (auto-fix)
```

### CI Validation

```bash
# GitHub Actions workflow lint
actionlint .github/workflows/ci.yml
# GitHub Actions security audit
GH_TOKEN=$(gh auth token) zizmor --persona=pedantic .
```

## Validation Expectations

Prefer targeted validation for the area you changed:

- Rust logic changes: run `cargo test` and format with `cargo fmt`
- Rust integration or compile-surface changes: consider `cargo clippy` and `cargo lambda build --release --arm64`
- CDK changes: run `npx cdk synth`, `npm test`, `npm run check`, and use `npm run fmt` or `npm run check:fix` when formatting or auto-fix is needed
- CI workflow changes: run `actionlint` and `zizmor --persona=pedantic .`

If you cannot run validation, explicitly note that in your handoff.

## Bedrock Constraints

- Two response modes: structured output (JSON Schema) for non-Nova models, tool use for Amazon Nova models. The `supports_structured_output()` method selects the mode based on model ID.
- Both modes use the same response schema: `needs_notification` (boolean), `confidence` (enum: high/medium/low), `matched_feedback_reason` (string).
- Retry logic: exponential backoff with jitter, configurable via `MAX_RETRIES` (0-6) and `BASE_DELAY_MS` (100-10000). Only `ThrottlingException` and `ServiceUnavailableException` are retryable.
- Fail-safe on exhausted retries: returns `needs_notification=true` with `Confidence::Low`.
- Service-level fail-safe: if Bedrock says "don't notify" but confidence is not High, override to notify (false-negative avoidance).
- Treat log messages with similarity of 80% or higher as the same class of log when evaluating prior feedback.
- When past feedback conflicts, prefer the most recent feedback.
- System prompts are external files in `infrastructure/prompts/` (English and Japanese).

## Testing Patterns

- **Service layer**: manual mock structs (`MockRepo`, `MockJudge`, `MockNotifier`) implementing port traits for business logic testing
- **Infrastructure layer**: `aws-smithy-mocks` crate with `mock!()` / `mock_client!()` macros for AWS SDK mock testing (DynamoDB, Bedrock retry, Secrets Manager)
- **CDK**: `jest` with `jest.mock("cargo-lambda-cdk")` to avoid cargo-lambda build during tests
- Time-sensitive tests (retry backoff): use `#[tokio::test(start_paused = true)]`

## Notes

- Slack secrets are managed via AWS Secrets Manager.
- Feedback history is stored in DynamoDB and is used to influence alert decisions.
- Bedrock model selection is configurable via CfnParameter `BedrockModelId` (default: `us.amazon.nova-2-lite-v1:0`).
- Slack notifications and prompts support English and Japanese via `AppLanguage` parameter and the i18n module (`infrastructure/i18n.rs`).
