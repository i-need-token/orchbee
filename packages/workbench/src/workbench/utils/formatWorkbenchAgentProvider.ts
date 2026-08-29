import type { AgentAggregatePresentation } from '@orchbee/agent';
import { agentObservationMessages, type IntlShape } from '@orchbee/i18n';

export function formatWorkbenchAgentProvider(
	formatter: IntlShape,
	providerIds: AgentAggregatePresentation['providerIds'],
): string {
	if (providerIds.length > 1) {
		return formatter.formatMessage(agentObservationMessages.providerAgents);
	}
	switch (providerIds[0]) {
		case 'claude-code':
			return formatter.formatMessage(agentObservationMessages.providerClaudeCode);
		case 'cline':
			return formatter.formatMessage(agentObservationMessages.providerCline);
		case 'codex':
			return formatter.formatMessage(agentObservationMessages.providerCodex);
		case 'gemini':
			return formatter.formatMessage(agentObservationMessages.providerGeminiCli);
		case 'github-copilot':
			return formatter.formatMessage(agentObservationMessages.providerGitHubCopilotCli);
		case 'grok':
			return formatter.formatMessage(agentObservationMessages.providerGrok);
		case 'omp':
			return formatter.formatMessage(agentObservationMessages.providerOmp);
		case 'opencode':
			return formatter.formatMessage(agentObservationMessages.providerOpenCode);
		case 'pi':
			return formatter.formatMessage(agentObservationMessages.providerPi);
		case 'qwen-code':
			return formatter.formatMessage(agentObservationMessages.providerQwenCode);
		default:
			return formatter.formatMessage(agentObservationMessages.providerAgent);
	}
}
