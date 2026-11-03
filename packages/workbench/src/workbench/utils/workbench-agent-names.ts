import { agentObservationMessages, type IntlShape, type MessageDescriptor } from '@orchbee/i18n';

/** Product names for the labels the Server reports for the providers it recognizes. */
const AGENT_NAMES: ReadonlyMap<string, MessageDescriptor> = new Map<string, MessageDescriptor>([
	['agy', agentObservationMessages.providerAntigravityCli],
	['amp', agentObservationMessages.providerAmp],
	['claude', agentObservationMessages.providerClaudeCode],
	['cline', agentObservationMessages.providerCline],
	['codex', agentObservationMessages.providerCodex],
	['copilot', agentObservationMessages.providerGitHubCopilotCli],
	['cursor', agentObservationMessages.providerCursor],
	['devin', agentObservationMessages.providerDevin],
	['droid', agentObservationMessages.providerDroid],
	['gemini', agentObservationMessages.providerGeminiCli],
	['grok', agentObservationMessages.providerGrok],
	['hermes', agentObservationMessages.providerHermes],
	['kilo', agentObservationMessages.providerKilo],
	['kimi', agentObservationMessages.providerKimi],
	['kiro', agentObservationMessages.providerKiroCli],
	['letta', agentObservationMessages.providerLetta],
	['maki', agentObservationMessages.providerMaki],
	['mastracode', agentObservationMessages.providerMastracode],
	['muse', agentObservationMessages.providerMuse],
	['omp', agentObservationMessages.providerOmp],
	['opencode', agentObservationMessages.providerOpenCode],
	['pi', agentObservationMessages.providerPi],
	['qodercli', agentObservationMessages.providerQoderCli],
	['qwen', agentObservationMessages.providerQwenCode],
]);

/** A custom report's Agent keeps the name it reported. */
export function formatWorkbenchAgentName(intl: IntlShape, agent: string): string {
	const name = AGENT_NAMES.get(agent);
	return name === undefined ? agent : intl.formatMessage(name);
}

/** One name for one Agent; a generic name once different Agents share a summary. */
export function formatWorkbenchAgentGroupName(intl: IntlShape, agents: readonly string[]): string {
	const [only, ...others] = agents;
	return only !== undefined && others.length === 0
		? formatWorkbenchAgentName(intl, only)
		: intl.formatMessage(agentObservationMessages.providerAgents);
}
