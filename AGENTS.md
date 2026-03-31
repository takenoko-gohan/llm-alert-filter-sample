# AGENTS.md

## Overview

This repository contains a serverless sample that uses Amazon Bedrock to filter CloudWatch Logs error alerts before posting to Slack.

- `lambda/`: Rust Lambda application code
- `cdk/`: AWS CDK app written in TypeScript
- `slack_app/`: Slack app manifest
- `docs/`: documentation assets

The system has two Lambda binaries:

- `notifier`: triggered by CloudWatch Logs subscription filters, evaluates whether an error should be sent to Slack
- `collector`: receives Slack feedback via Function URL and stores user feedback in DynamoDB

## Working Guidelines

- Keep changes scoped and consistent with the existing architecture.
- Prefer fixing root causes over layering on ad-hoc workarounds.
- Do not commit generated artifacts from `cdk/cdk.out/`, `cdk/node_modules/`, or `lambda/target/`.
- If you change infrastructure, deployment steps, or developer workflows, update `README.md` as needed.
- Preserve the current region assumption of `us-east-1` unless the task explicitly requires changing it.

## Project Structure

### Rust Lambda

The Rust code follows a layered structure under `lambda/src/`:

- `domain/`: entities, repository traits, and value objects
- `application/`: orchestration and service logic
- `infrastructure/`: Bedrock, Slack, Secrets Manager, and DynamoDB integrations
- `interface/`: Axum routers, handlers, payloads, and middleware
- `bin/`: Lambda entrypoints (`collector.rs`, `notifier.rs`)

When modifying Rust code:

- Keep domain logic free from AWS or HTTP-specific concerns.
- Put external service integrations in `infrastructure/`.
- Put routing, request parsing, and middleware concerns in `interface/`.
- Keep service orchestration in `application/services.rs`.

### CDK

The infrastructure code lives in `cdk/`.

- Main stack: `cdk/lib/llm-alert-filter-stack.ts`
- Entry point: `cdk/bin/cdk.ts`

When modifying CDK code:

- Keep logical resource naming aligned with the existing stack.
- Avoid changing deployed resource names unless explicitly required.
- Be careful with changes that affect Bedrock model configuration, Function URLs, IAM permissions, or DynamoDB schema.

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
cd cdk && npm run build
cd cdk && npm test
cd cdk && npm run check
cd cdk && npm run check:fix   # biome check --write (auto-fix)
cd cdk && npm run lint:fix    # biome lint --write (auto-fix)
cd cdk && npm run fmt         # biome format --write (auto-fix)
cd cdk && npx cdk synth
```

## Validation Expectations

Prefer targeted validation for the area you changed:

- Rust logic changes: run `cargo test` and format with `cargo fmt`
- Rust integration or compile-surface changes: consider `cargo clippy` and `cargo lambda build --release --arm64`
- CDK changes: run `npm run build`, `npm test`, `npm run check`, and use `npm run fmt` or `npm run check:fix` when formatting or auto-fix is needed

If you cannot run validation, explicitly note that in your handoff.

## Bedrock Constraints

- Preserve the Converse API pattern that defines the `judge_needs_notification` tool and expects a boolean result.
- Treat log messages with similarity of 80% or higher as the same class of log when evaluating prior feedback.
- When past feedback conflicts, prefer the most recent feedback.
- Keep the system prompt behavior aligned with the existing five inference rules; check `CLAUDE.md` before changing prompt or tool semantics.

## Notes

- Slack secrets are managed via AWS Secrets Manager.
- Feedback history is stored in DynamoDB and is used to influence alert decisions.
- Bedrock model selection is defined in the CDK stack and should be treated as a deliberate configuration choice.
