# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Bedrock Claude を使って CloudWatch Logs のエラーログを評価し、過去のフィードバック履歴に基づいて Slack 通知の要否を判定するサーバーレスアプリケーション。Rust Lambda + AWS CDK (TypeScript) 構成。

## Build & Development Commands

### Rust Lambda

```bash
# ビルド（cargo-lambda 必須）
cd lambda && cargo lambda build --release --arm64

# テスト
cd lambda && cargo test

# clippy
cd lambda && cargo clippy

# フォーマット
cd lambda && cargo fmt
```

### CDK (TypeScript)

```bash
# 依存インストール
cd cdk && npm ci

# ビルド
cd cdk && npm run build

# テスト
cd cdk && npm test

# リント・フォーマット
cd cdk && npm run check        # biome check
cd cdk && npm run check:fix    # biome check --write
cd cdk && npm run fmt          # biome format --write
cd cdk && npm run lint         # biome lint
cd cdk && npm run lint:fix     # biome lint --write

# CDK デプロイ
cd cdk && npx cdk deploy
```

## Architecture

クリーンアーキテクチャ（Domain → Application → Infrastructure → Interface）。

### Lambda バイナリ（2つ）

- **notifier** (`lambda/src/bin/notifier.rs`): CloudWatch Logs サブスクリプションフィルタから起動。エラーログを Bedrock Claude で評価し、通知要と判定されれば Slack に投稿。
- **collector** (`lambda/src/bin/collector.rs`): Slack のボタン操作・モーダル送信を受け取る Function URL エンドポイント（Axum）。ユーザーフィードバックを DynamoDB に保存。

### レイヤー構成 (`lambda/src/`)

- **domain/**: Feedback エンティティ、FeedbackId/Timestamp 値オブジェクト、Repository トレイト
- **application/services.rs**: NotificationService（ログ評価→通知）、CollectionService（フィードバック収集）
- **infrastructure/**: Bedrock Converse API、Slack API、Secrets Manager、DynamoDB Repository 実装
- **interface/**: Axum ルーター、Slack リクエスト署名検証ミドルウェア、ハンドラー

### Bedrock 連携の要点

- Converse API で `judge_needs_notification` ツールを定義し、boolean を返させる
- ログメッセージの類似度 80% 以上を「同様のログ」とみなす
- 矛盾するフィードバックがある場合は最新を優先
- システムプロンプトに5つの推論ルールを定義

### インフラ（CDK）

- リージョン: us-east-1
- DynamoDB: PAY_PER_REQUEST、GSI `log_group_index`
- Lambda: ARM_64、128MB（notifier: 120s / collector: 30s タイムアウト）
- Secrets Manager: Slack トークン・署名シークレットを管理
