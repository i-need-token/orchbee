import { agentObservationMessages, type MessageDescriptor } from '@orchbee/i18n';

/**
 * The Agent CLIs the product recognizes, by the names the product shows. Each name is the
 * product's own descriptor; the set and its order follow the provider descriptors of
 * `agentObservationMessages` (`packages/i18n/src/descriptors/agent-observation.ts`), which the
 * desktop application's `AGENT_NAMES` (`apps/web/src/workbench/utils/workbench-agent-names.ts`)
 * maps every recognized provider to, as the Server's `Provider` list
 * (`crates/agent-observation-protocol/src/report_values/provider.rs`) reports them. A provider
 * added there is added here too.
 */
export const RECOGNIZED_AGENTS: readonly MessageDescriptor[] = [
	agentObservationMessages.providerClaudeCode,
	agentObservationMessages.providerCodex,
	agentObservationMessages.providerGrok,
	agentObservationMessages.providerGeminiCli,
	agentObservationMessages.providerGitHubCopilotCli,
	agentObservationMessages.providerCline,
	agentObservationMessages.providerOmp,
	agentObservationMessages.providerOpenCode,
	agentObservationMessages.providerPi,
	agentObservationMessages.providerQwenCode,
	agentObservationMessages.providerKilo,
	agentObservationMessages.providerCursor,
	agentObservationMessages.providerQoderCli,
	agentObservationMessages.providerAntigravityCli,
	agentObservationMessages.providerMastracode,
	agentObservationMessages.providerDroid,
	agentObservationMessages.providerKimi,
	agentObservationMessages.providerLetta,
	agentObservationMessages.providerDevin,
	agentObservationMessages.providerHermes,
	agentObservationMessages.providerKiroCli,
	agentObservationMessages.providerAmp,
	agentObservationMessages.providerMaki,
	agentObservationMessages.providerMuse,
	agentObservationMessages.providerAtomCode,
	agentObservationMessages.providerCodeBuddy,
	agentObservationMessages.providerCommandCode,
	agentObservationMessages.providerDevEco,
	agentObservationMessages.providerMimo,
	agentObservationMessages.providerReasonix,
	agentObservationMessages.providerSnow,
	agentObservationMessages.providerMinimaxCode,
	agentObservationMessages.providerMothx,
	agentObservationMessages.providerJcode,
	agentObservationMessages.providerOpenClaw,
	agentObservationMessages.providerPrimeAgent,
];
