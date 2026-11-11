import { agentObservationMessages, type IntlShape, type MessageDescriptor } from '@orchbee/i18n';

/** Product names for the labels the Server reports for the providers it recognizes. */
const AGENT_NAMES: ReadonlyMap<string, MessageDescriptor> = new Map<string, MessageDescriptor>([
	['agy', agentObservationMessages.providerAntigravityCli],
	['amp', agentObservationMessages.providerAmp],
	['atomcode', agentObservationMessages.providerAtomCode],
	['claude', agentObservationMessages.providerClaudeCode],
	['cline', agentObservationMessages.providerCline],
	['codebuddy', agentObservationMessages.providerCodeBuddy],
	['codex', agentObservationMessages.providerCodex],
	['command-code', agentObservationMessages.providerCommandCode],
	['copilot', agentObservationMessages.providerGitHubCopilotCli],
	['cursor', agentObservationMessages.providerCursor],
	['deveco', agentObservationMessages.providerDevEco],
	['devin', agentObservationMessages.providerDevin],
	['droid', agentObservationMessages.providerDroid],
	['gemini', agentObservationMessages.providerGeminiCli],
	['grok', agentObservationMessages.providerGrok],
	['hermes', agentObservationMessages.providerHermes],
	['jcode', agentObservationMessages.providerJcode],
	['kilo', agentObservationMessages.providerKilo],
	['kimi', agentObservationMessages.providerKimi],
	['kiro', agentObservationMessages.providerKiroCli],
	['letta', agentObservationMessages.providerLetta],
	['maki', agentObservationMessages.providerMaki],
	['mastracode', agentObservationMessages.providerMastracode],
	['mimo', agentObservationMessages.providerMimo],
	['minimax-code', agentObservationMessages.providerMinimaxCode],
	['mothx', agentObservationMessages.providerMothx],
	['muse', agentObservationMessages.providerMuse],
	['omp', agentObservationMessages.providerOmp],
	['openclaw', agentObservationMessages.providerOpenClaw],
	['opencode', agentObservationMessages.providerOpenCode],
	['pi', agentObservationMessages.providerPi],
	['prime-agent', agentObservationMessages.providerPrimeAgent],
	['qodercli', agentObservationMessages.providerQoderCli],
	['qwen', agentObservationMessages.providerQwenCode],
	['reasonix', agentObservationMessages.providerReasonix],
	['snow', agentObservationMessages.providerSnow],
]);

/** The provider-registry IDs whose report label differs, as the Server's `report_label` maps them. */
const PROVIDER_LABELS: ReadonlyMap<string, string> = new Map<string, string>([
	['antigravity', 'agy'],
	['claude-code', 'claude'],
	['github-copilot', 'copilot'],
	['qwen-code', 'qwen'],
]);

/** The commands that start a provider where they differ from its report label and registry ID. */
const PROVIDER_COMMANDS: ReadonlyMap<string, string> = new Map<string, string>([
	['cursor', 'agent'],
	['kiro', 'kiro-cli'],
	['minimax-code', 'mcode'],
	['qodercli', 'qoder'],
]);

/**
 * Every name that only names this Agent's provider, as the TUI compares them: its report label,
 * its product name, its provider-registry ID and the command that starts it.
 */
export function listWorkbenchAgentProviderNames(intl: IntlShape, agent: string): readonly string[] {
	const registryIds = [...PROVIDER_LABELS].flatMap(([id, label]) => (label === agent ? [id] : []));
	const command = PROVIDER_COMMANDS.get(agent);
	return [
		agent,
		formatWorkbenchAgentName(intl, agent),
		...registryIds,
		...(command === undefined ? [] : [command]),
	];
}

/** A custom report's Agent keeps the name it reported. */
export function formatWorkbenchAgentName(intl: IntlShape, agent: string): string {
	const name = AGENT_NAMES.get(agent);
	return name === undefined ? agent : intl.formatMessage(name);
}

/** The product name for a provider-registry ID, such as an Agent session's provider. */
export function formatWorkbenchAgentProvider(intl: IntlShape, providerId: string): string {
	return formatWorkbenchAgentName(intl, PROVIDER_LABELS.get(providerId) ?? providerId);
}

/** One name for one Agent; a generic name once different Agents share a summary. */
export function formatWorkbenchAgentGroupName(intl: IntlShape, agents: readonly string[]): string {
	const [only, ...others] = agents;
	return only !== undefined && others.length === 0
		? formatWorkbenchAgentName(intl, only)
		: intl.formatMessage(agentObservationMessages.providerAgents);
}
