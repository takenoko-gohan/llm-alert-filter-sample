import * as cdk from "aws-cdk-lib";
import { LlmAlertFilterStack } from "../lib/llm-alert-filter-stack";

const app = new cdk.App();
new LlmAlertFilterStack(app, "LlmAlertFilterStack", {
	env: { region: "us-east-1" },
});
