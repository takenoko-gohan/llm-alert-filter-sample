// Mock RustFunction to avoid cargo-lambda dependency in unit tests
jest.mock("cargo-lambda-cdk", () => {
	const cdkLib = require("aws-cdk-lib");
	class RustFunction extends cdkLib.aws_lambda.Function {
		constructor(scope: unknown, id: string, props: Record<string, unknown>) {
			const {
				manifestPath: _m,
				binaryName: _b,
				bundling: _bu,
				...rest
			} = props;
			super(scope, id, {
				...rest,
				runtime: cdkLib.aws_lambda.Runtime.PROVIDED_AL2023,
				handler: "bootstrap",
				code: cdkLib.aws_lambda.Code.fromAsset(__dirname),
			});
		}
	}
	return { RustFunction };
});

import * as cdk from "aws-cdk-lib";
import { Template } from "aws-cdk-lib/assertions";

import { LlmAlertFilterStack } from "../lib/llm-alert-filter-stack";

function createTemplate(): Template {
	const app = new cdk.App();
	const stack = new LlmAlertFilterStack(app, "TestStack", {
		env: { region: "us-east-1" },
	});
	return Template.fromStack(stack);
}

describe("LlmAlertFilterStack", () => {
	const template = createTemplate();

	test("DynamoDB table is created with PAY_PER_REQUEST billing", () => {
		template.hasResourceProperties("AWS::DynamoDB::Table", {
			BillingMode: "PAY_PER_REQUEST",
			TableName: "llm_alert_filter_feedback",
		});
	});

	test("DynamoDB table has log_group GSI", () => {
		template.hasResourceProperties("AWS::DynamoDB::Table", {
			GlobalSecondaryIndexes: [
				{
					IndexName: "log_group_index",
					KeySchema: [
						{
							AttributeName: "log_group",
							KeyType: "HASH",
						},
					],
				},
			],
		});
	});

	test("Two Lambda functions are created", () => {
		template.resourceCountIs("AWS::Lambda::Function", 2);
	});

	test("Notifier Lambda has correct timeout and architecture", () => {
		template.hasResourceProperties("AWS::Lambda::Function", {
			FunctionName: "llm-alert-filter-notifier",
			Timeout: 120,
			MemorySize: 128,
			Architectures: ["arm64"],
		});
	});

	test("Collector Lambda has correct timeout", () => {
		template.hasResourceProperties("AWS::Lambda::Function", {
			FunctionName: "llm-alert-filter-collector",
			Timeout: 30,
		});
	});

	test("Subscription filters are created for test log groups", () => {
		template.resourceCountIs("AWS::Logs::SubscriptionFilter", 2);
	});

	test("Bedrock IAM policy is attached to notifier role", () => {
		template.hasResourceProperties("AWS::IAM::Role", {
			RoleName: "LlmAlertFilterNotifier",
			Policies: [
				{},
				{
					PolicyName: "bedrockPolicy",
					PolicyDocument: {
						Statement: [
							{
								Action: "bedrock:InvokeModel",
								Effect: "Allow",
								Resource: "*",
							},
						],
					},
				},
				{},
			],
		});
	});

	describe("CfnParameters", () => {
		test("BedrockModelId has correct default", () => {
			template.hasParameter("BedrockModelId", {
				Type: "String",
				Default: "us.amazon.nova-2-lite-v1:0",
			});
		});

		test("AppLanguage has correct default and allowed values", () => {
			template.hasParameter("AppLanguage", {
				Type: "String",
				Default: "en",
				AllowedValues: ["en", "ja"],
			});
		});

		test("MaxRetries has correct type and constraints", () => {
			template.hasParameter("MaxRetries", {
				Type: "Number",
				Default: 3,
				MinValue: 0,
				MaxValue: 6,
			});
		});

		test("BaseDelayMs has correct type and constraints", () => {
			template.hasParameter("BaseDelayMs", {
				Type: "Number",
				Default: 500,
				MinValue: 100,
				MaxValue: 10000,
			});
		});
	});
});
