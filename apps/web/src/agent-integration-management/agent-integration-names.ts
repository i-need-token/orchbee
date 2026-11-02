import { agentObservationMessages, type MessageDescriptor } from '@orchbee/i18n';

import type { AgentIntegrationId } from './agent-integration-management-control';

/** Curated product names shared by the Settings rows, their search entries, and the reminder. */
export const AGENT_INTEGRATION_NAME_MESSAGES: Readonly<
	Record<AgentIntegrationId, MessageDescriptor>
> = {
	antigravity: agentObservationMessages.providerAntigravityCli,
	'claude-code': agentObservationMessages.providerClaudeCode,
	cline: agentObservationMessages.providerCline,
	codex: agentObservationMessages.providerCodex,
	cursor: agentObservationMessages.providerCursor,
	devin: agentObservationMessages.providerDevin,
	droid: agentObservationMessages.providerDroid,
	gemini: agentObservationMessages.providerGeminiCli,
	'github-copilot': agentObservationMessages.providerGitHubCopilotCli,
	grok: agentObservationMessages.providerGrok,
	hermes: agentObservationMessages.providerHermes,
	kilo: agentObservationMessages.providerKilo,
	kimi: agentObservationMessages.providerKimi,
	letta: agentObservationMessages.providerLetta,
	mastracode: agentObservationMessages.providerMastracode,
	omp: agentObservationMessages.providerOmp,
	opencode: agentObservationMessages.providerOpenCode,
	pi: agentObservationMessages.providerPi,
	qodercli: agentObservationMessages.providerQoderCli,
	'qwen-code': agentObservationMessages.providerQwenCode,
};
