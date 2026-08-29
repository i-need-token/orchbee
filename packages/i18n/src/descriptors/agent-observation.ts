import { defineMessages } from 'react-intl';

import { enUSAgentObservationMessages } from '../locales';

export const agentObservationMessages = defineMessages({
	notificationAttentionBody: {
		id: 'agent.observation.notification.attention.body',
		defaultMessage: enUSAgentObservationMessages['agent.observation.notification.attention.body'],
		description: 'Safe body for an Agent attention system notification.',
	},
	notificationAttentionTitle: {
		id: 'agent.observation.notification.attention.title',
		defaultMessage: enUSAgentObservationMessages['agent.observation.notification.attention.title'],
		description: 'Title for an Agent attention system notification.',
	},
	notificationFailedBody: {
		id: 'agent.observation.notification.failed.body',
		defaultMessage: enUSAgentObservationMessages['agent.observation.notification.failed.body'],
		description: 'Safe body for a failed Agent Turn system notification.',
	},
	notificationFailedTitle: {
		id: 'agent.observation.notification.failed.title',
		defaultMessage: enUSAgentObservationMessages['agent.observation.notification.failed.title'],
		description: 'Title for a failed Agent Turn system notification.',
	},
	notificationSucceededBody: {
		id: 'agent.observation.notification.succeeded.body',
		defaultMessage: enUSAgentObservationMessages['agent.observation.notification.succeeded.body'],
		description: 'Safe body for a completed Agent Turn system notification.',
	},
	notificationSucceededTitle: {
		id: 'agent.observation.notification.succeeded.title',
		defaultMessage: enUSAgentObservationMessages['agent.observation.notification.succeeded.title'],
		description: 'Title for a completed Agent Turn system notification.',
	},
	providerAgent: {
		id: 'agent.observation.provider.agent',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.agent'],
		description: 'Safe generic provider name for one observed Agent.',
	},
	providerAgents: {
		id: 'agent.observation.provider.agents',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.agents'],
		description: 'Safe generic provider name for multiple observed Agent providers.',
	},
	providerClaudeCode: {
		id: 'agent.observation.provider.claudeCode',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.claudeCode'],
		description: 'Curated product name for the Claude Code Agent provider.',
	},
	providerCodex: {
		id: 'agent.observation.provider.codex',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.codex'],
		description: 'Curated product name for the Codex Agent provider.',
	},
	providerGrok: {
		id: 'agent.observation.provider.grok',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.grok'],
		description: 'Curated product name for the Grok Agent provider.',
	},
	providerOmp: {
		id: 'agent.observation.provider.omp',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.omp'],
		description: 'Curated product name for the OMP Agent provider.',
	},
	providerOpenCode: {
		id: 'agent.observation.provider.openCode',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.openCode'],
		description: 'Curated product name for the OpenCode Agent provider.',
	},
	providerPi: {
		id: 'agent.observation.provider.pi',
		defaultMessage: enUSAgentObservationMessages['agent.observation.provider.pi'],
		description: 'Curated product name for the Pi Agent provider.',
	},
	statusCanceled: {
		id: 'agent.observation.status.canceled',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.canceled'],
		description: 'Observed Agent Turn canceled status.',
	},
	statusCommand: {
		id: 'agent.observation.status.command',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.command'],
		description: 'Observed Agent command activity.',
	},
	statusCompacting: {
		id: 'agent.observation.status.compacting',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.compacting'],
		description: 'Observed Agent context-compaction activity.',
	},
	statusDisconnected: {
		id: 'agent.observation.status.disconnected',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.disconnected'],
		description: 'Observed Agent source disconnected health.',
	},
	statusEditing: {
		id: 'agent.observation.status.editing',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.editing'],
		description: 'Observed Agent editing activity.',
	},
	statusFailed: {
		id: 'agent.observation.status.failed',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.failed'],
		description: 'Observed Agent Turn failed status.',
	},
	statusInterrupted: {
		id: 'agent.observation.status.interrupted',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.interrupted'],
		description: 'Observed Agent Turn interrupted status.',
	},
	statusLimitedActivity: {
		id: 'agent.observation.status.limitedActivity',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.limitedActivity'],
		description: 'Untrusted terminal-scoped activity hint with explicitly limited status.',
	},
	statusNeedsAttention: {
		id: 'agent.observation.status.needsAttention',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.needsAttention'],
		description: 'Observed Agent status with an exact pending-attention count.',
	},
	statusOtherActivity: {
		id: 'agent.observation.status.otherActivity',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.otherActivity'],
		description: 'Observed Agent activity without a more specific product label.',
	},
	statusQueued: {
		id: 'agent.observation.status.queued',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.queued'],
		description: 'Observed Agent Turn queued status.',
	},
	statusReady: {
		id: 'agent.observation.status.ready',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.ready'],
		description: 'Affirmatively observed idle Agent status.',
	},
	statusRetrying: {
		id: 'agent.observation.status.retrying',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.retrying'],
		description: 'Observed Agent retry activity.',
	},
	statusStale: {
		id: 'agent.observation.status.stale',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.stale'],
		description: 'Observed Agent source stale health.',
	},
	statusSubagent: {
		id: 'agent.observation.status.subagent',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.subagent'],
		description: 'Observed Agent subagent activity.',
	},
	statusSucceeded: {
		id: 'agent.observation.status.succeeded',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.succeeded'],
		description: 'Observed Agent Turn completed status.',
	},
	statusThinking: {
		id: 'agent.observation.status.thinking',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.thinking'],
		description: 'Observed Agent thinking activity.',
	},
	statusTool: {
		id: 'agent.observation.status.tool',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.tool'],
		description: 'Observed Agent tool activity.',
	},
	statusUnknown: {
		id: 'agent.observation.status.unknown',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.unknown'],
		description: 'Limited observed Agent status that does not guess idle or work.',
	},
	statusWaitingExternal: {
		id: 'agent.observation.status.waitingExternal',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.waitingExternal'],
		description: 'Observed Agent Turn waiting for external action status.',
	},
	statusWithHealth: {
		id: 'agent.observation.status.withHealth',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.withHealth'],
		description: 'Observed Agent semantic status combined with degraded source health.',
	},
	statusWorking: {
		id: 'agent.observation.status.working',
		defaultMessage: enUSAgentObservationMessages['agent.observation.status.working'],
		description: 'Observed Agent Turn working status.',
	},
});
