import * as cdk from "aws-cdk-lib";
import { RustFunction } from "cargo-lambda-cdk";
import type { Construct } from "constructs";

export class LlmAlertFilterStack extends cdk.Stack {
	constructor(scope: Construct, id: string, props?: cdk.StackProps) {
		super(scope, id, props);

		// cfn parameters
		const slackChannelId = new cdk.CfnParameter(this, "SlackChannelId", {
			type: "String",
			description: "Slack Channel",
		});

		// DynamoDB Table
		const table = new cdk.aws_dynamodb.Table(this, "FeedbackTable", {
			tableName: "llm_alert_filter_feedback",
			billingMode: cdk.aws_dynamodb.BillingMode.PAY_PER_REQUEST,
			encryption: cdk.aws_dynamodb.TableEncryption.AWS_MANAGED,
			partitionKey: { name: "id", type: cdk.aws_dynamodb.AttributeType.STRING },
			removalPolicy: cdk.RemovalPolicy.DESTROY,
		});
		table.addGlobalSecondaryIndex({
			indexName: "log_group_index",
			partitionKey: {
				name: "log_group",
				type: cdk.aws_dynamodb.AttributeType.STRING,
			},
		});

		// CloudWatch Log Group
		const notifierLogGroup = new cdk.aws_logs.LogGroup(
			this,
			"NotifierLogGroup",
			{
				logGroupName: "/aws/lambda/llm-alert-filter-notifier",
				removalPolicy: cdk.RemovalPolicy.DESTROY,
			},
		);

		const collectorLogGroup = new cdk.aws_logs.LogGroup(
			this,
			"CollectorLogGroup",
			{
				logGroupName: "/aws/lambda/llm-alert-filter-collector",
				removalPolicy: cdk.RemovalPolicy.DESTROY,
			},
		);

		// Secrets Manager
		const notifierSecrets = new cdk.aws_secretsmanager.Secret(
			this,
			"NotifierSecrets",
			{
				secretName: "llm-alert-filter-notifier",
				secretObjectValue: {
					SLACK_TOKEN: cdk.SecretValue.unsafePlainText("dummy"),
				},
			},
		);

		const collectorSecrets = new cdk.aws_secretsmanager.Secret(
			this,
			"CollectorSecrets",
			{
				secretName: "llm-alert-filter-collector",
				secretObjectValue: {
					SIGNING_SECRET: cdk.SecretValue.unsafePlainText("dummy"),
					SLACK_TOKEN: cdk.SecretValue.unsafePlainText("dummy"),
				},
			},
		);

		// IAM Role
		const notifierRole = new cdk.aws_iam.Role(this, "NotifierRole", {
			roleName: "LlmAlertFilterNotifier",
			assumedBy: new cdk.aws_iam.ServicePrincipal("lambda.amazonaws.com"),
			managedPolicies: [
				cdk.aws_iam.ManagedPolicy.fromAwsManagedPolicyName(
					"service-role/AWSLambdaBasicExecutionRole",
				),
			],
			inlinePolicies: {
				dynamoDbPolicy: new cdk.aws_iam.PolicyDocument({
					statements: [
						new cdk.aws_iam.PolicyStatement({
							effect: cdk.aws_iam.Effect.ALLOW,
							actions: ["dynamodb:Query"],
							resources: [`${table.tableArn}/*`],
						}),
					],
				}),
				bedrockPolicy: new cdk.aws_iam.PolicyDocument({
					statements: [
						new cdk.aws_iam.PolicyStatement({
							effect: cdk.aws_iam.Effect.ALLOW,
							actions: ["bedrock:InvokeModel"],
							resources: ["*"],
						}),
					],
				}),
				secretsManagerPolicy: new cdk.aws_iam.PolicyDocument({
					statements: [
						new cdk.aws_iam.PolicyStatement({
							effect: cdk.aws_iam.Effect.ALLOW,
							actions: ["secretsmanager:GetSecretValue"],
							resources: [notifierSecrets.secretArn],
						}),
					],
				}),
			},
		});

		const collectorRole = new cdk.aws_iam.Role(this, "CollectorRole", {
			roleName: "LlmAlertFilterCollector",
			assumedBy: new cdk.aws_iam.ServicePrincipal("lambda.amazonaws.com"),
			managedPolicies: [
				cdk.aws_iam.ManagedPolicy.fromAwsManagedPolicyName(
					"service-role/AWSLambdaBasicExecutionRole",
				),
			],
			inlinePolicies: {
				dynamoDbPolicy: new cdk.aws_iam.PolicyDocument({
					statements: [
						new cdk.aws_iam.PolicyStatement({
							effect: cdk.aws_iam.Effect.ALLOW,
							actions: ["dynamodb:PutItem"],
							resources: [table.tableArn],
						}),
					],
				}),
				secretsmanagerPolicy: new cdk.aws_iam.PolicyDocument({
					statements: [
						new cdk.aws_iam.PolicyStatement({
							effect: cdk.aws_iam.Effect.ALLOW,
							actions: ["secretsmanager:GetSecretValue"],
							resources: [collectorSecrets.secretArn],
						}),
					],
				}),
			},
		});

		// Lambda Functions
		const notifierFunction = new RustFunction(this, "NotifierFunction", {
			functionName: "llm-alert-filter-notifier",
			role: notifierRole,
			logGroup: notifierLogGroup,
			timeout: cdk.Duration.seconds(120),
			memorySize: 128,
			architecture: cdk.aws_lambda.Architecture.ARM_64,
			environment: {
				TABLE_NAME: table.tableName,
				//BEDROCK_MODEL_ID: "us.amazon.nova-lite-v1:0",
				//BEDROCK_MODEL_ID: "us.amazon.nova-pro-v1:0",
				//BEDROCK_MODEL_ID: "us.anthropic.claude-3-5-haiku-20241022-v1:0",
				BEDROCK_MODEL_ID: "us.anthropic.claude-3-7-sonnet-20250219-v1:0",
				BEDROCK_TOP_P: "0.9",
				BEDROCK_TEMPERATURE: "0.7",
				SLACK_CHANNEL_ID: slackChannelId.valueAsString,
				SECRET_ID: notifierSecrets.secretName,
			},
			manifestPath: "../lambda/Cargo.toml",
			binaryName: "notifier",
			bundling: {
				cargoLambdaFlags: ["--bin", "notifier", "--release"],
			},
		});

		const collectorFunction = new RustFunction(this, "CollectorFunction", {
			functionName: "llm-alert-filter-collector",
			role: collectorRole,
			logGroup: collectorLogGroup,
			timeout: cdk.Duration.seconds(30),
			memorySize: 128,
			architecture: cdk.aws_lambda.Architecture.ARM_64,
			environment: {
				TABLE_NAME: table.tableName,
				SECRET_ID: collectorSecrets.secretName,
				SLACK_CHANNEL_ID: slackChannelId.valueAsString,
			},
			manifestPath: "../lambda/Cargo.toml",
			binaryName: "collector",
			bundling: {
				cargoLambdaFlags: ["--bin", "collector", "--release"],
			},
		});
		collectorFunction.addFunctionUrl({
			authType: cdk.aws_lambda.FunctionUrlAuthType.NONE,
		});

		// Test Resource
		const test1LogGroup = new cdk.aws_logs.LogGroup(this, "Test1LogGroup", {
			logGroupName: "llm-alert-filter-test1",
			removalPolicy: cdk.RemovalPolicy.DESTROY,
		});
		test1LogGroup.addSubscriptionFilter("Test1SubscriptionFilter", {
			destination: new cdk.aws_logs_destinations.LambdaDestination(
				notifierFunction,
			),
			filterPattern: cdk.aws_logs.FilterPattern.anyTerm(
				"ERROR",
				"Error",
				"error",
			),
		});
		new cdk.aws_logs.LogStream(this, "Test1LogStream", {
			logGroup: test1LogGroup,
			logStreamName: "hoge-app",
		});

		const test2LogGroup = new cdk.aws_logs.LogGroup(this, "Test2LogGroup", {
			logGroupName: "llm-alert-filter-test2",
			removalPolicy: cdk.RemovalPolicy.DESTROY,
		});
		test2LogGroup.addSubscriptionFilter("Test2SubscriptionFilter", {
			destination: new cdk.aws_logs_destinations.LambdaDestination(
				notifierFunction,
			),
			filterPattern: cdk.aws_logs.FilterPattern.anyTerm(
				"ERROR",
				"Error",
				"error",
			),
		});
		new cdk.aws_logs.LogStream(this, "Test2LogStream", {
			logGroup: test2LogGroup,
			logStreamName: "fuga-app",
		});
	}
}
