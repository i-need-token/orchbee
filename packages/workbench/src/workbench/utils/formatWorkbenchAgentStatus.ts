import type {
	AgentActivity,
	AgentAggregatePresentation,
	AgentSemanticStatus,
} from '@orchbee/agent';
import { agentObservationMessages, type IntlShape } from '@orchbee/i18n';

import { formatWorkbenchAgentProvider } from './formatWorkbenchAgentProvider';

export interface WorkbenchAgentStatusView {
	readonly provider: string;
	readonly status: string;
}

export function formatWorkbenchAgentStatus(
	intl: IntlShape,
	presentation: AgentAggregatePresentation,
): WorkbenchAgentStatusView {
	const provider = formatWorkbenchAgentProvider(intl, presentation.providerIds);
	const semanticStatus = formatSemanticStatus(intl, presentation.status);
	const status =
		presentation.freshness === 'fresh'
			? semanticStatus
			: intl.formatMessage(agentObservationMessages.statusWithHealth, {
					health: intl.formatMessage(
						presentation.freshness === 'stale'
							? agentObservationMessages.statusStale
							: agentObservationMessages.statusDisconnected,
					),
					status: semanticStatus,
				});
	return {
		provider,
		status,
	};
}

function formatSemanticStatus(formatter: IntlShape, value: AgentSemanticStatus): string {
	switch (value.kind) {
		case 'needs-attention':
			return formatter.formatMessage(agentObservationMessages.statusNeedsAttention, {
				count: value.count,
			});
		case 'failed':
			return formatter.formatMessage(agentObservationMessages.statusFailed);
		case 'succeeded':
			return formatter.formatMessage(agentObservationMessages.statusSucceeded);
		case 'canceled':
			return formatter.formatMessage(agentObservationMessages.statusCanceled);
		case 'interrupted':
			return formatter.formatMessage(agentObservationMessages.statusInterrupted);
		case 'working':
			if (value.phase === 'queued') {
				return formatter.formatMessage(agentObservationMessages.statusQueued);
			}
			if (value.phase === 'waiting-external') {
				return formatter.formatMessage(agentObservationMessages.statusWaitingExternal);
			}
			return formatActivity(formatter, value.activity);
		case 'idle':
			return formatter.formatMessage(agentObservationMessages.statusReady);
		case 'unknown':
			return value.activityHint === null
				? formatter.formatMessage(agentObservationMessages.statusUnknown)
				: formatter.formatMessage(agentObservationMessages.statusLimitedActivity);
		case 'ended':
			return formatter.formatMessage(agentObservationMessages.statusUnknown);
	}
}

function formatActivity(formatter: IntlShape, activity: AgentActivity | null): string {
	switch (activity) {
		case 'thinking':
			return formatter.formatMessage(agentObservationMessages.statusThinking);
		case 'tool':
			return formatter.formatMessage(agentObservationMessages.statusTool);
		case 'editing':
			return formatter.formatMessage(agentObservationMessages.statusEditing);
		case 'command':
			return formatter.formatMessage(agentObservationMessages.statusCommand);
		case 'subagent':
			return formatter.formatMessage(agentObservationMessages.statusSubagent);
		case 'compacting':
			return formatter.formatMessage(agentObservationMessages.statusCompacting);
		case 'retrying':
			return formatter.formatMessage(agentObservationMessages.statusRetrying);
		case 'other':
			return formatter.formatMessage(agentObservationMessages.statusOtherActivity);
		case null:
			return formatter.formatMessage(agentObservationMessages.statusWorking);
	}
}
