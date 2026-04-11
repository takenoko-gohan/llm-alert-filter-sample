import * as cdk from "aws-cdk-lib";
import { LlmAlertFilterStack } from "../lib/llm-alert-filter-stack";

const app = new cdk.App();

const region =
	app.node.tryGetContext("region") ??
	process.env.CDK_DEFAULT_REGION ??
	process.env.AWS_DEFAULT_REGION ??
	"us-east-1";

new LlmAlertFilterStack(app, "LlmAlertFilterStack", {
	env: { region },
});
