import type {
	AgentActivity,
	AgentAggregatePresentation,
	AgentSemanticStatus,
} from '@orchbee/agent';
import { agentObservationMessages, type IntlShape } from '@orchbee/i18n';

import { formatWorkbenchAgentProvider } from './formatWorkbenchAgentProvider';

export interface WorkbenchAgentStatusView {
	readonly glyph: '●' | '•' | '○';
	readonly provider: string;
	readonly status: string;
	readonly tone: 'attention' | 'danger' | 'working' | 'idle' | 'neutral' | 'success';
}

export function formatWorkbenchAgentStatus<
	Presentation extends Pick<AgentAggregatePresentation, 'freshness' | 'providerIds' | 'status'>,
>(intl: IntlShape, presentation: Presentation): WorkbenchAgentStatusView {
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
		glyph: resolveGlyph(presentation.status),
		provider,
		status,
		tone: resolveTone(presentation.status),
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
function resolveTone(value: AgentSemanticStatus): WorkbenchAgentStatusView['tone'] {
	switch (value.kind) {
		case 'needs-attention':
			return 'attention';
		case 'failed':
		case 'canceled':
		case 'interrupted':
			return 'danger';
		case 'working':
			return 'working';
		case 'succeeded':
			return 'success';
		case 'idle':
			return 'idle';
		default:
			return 'neutral';
	}
}

function resolveGlyph(value: AgentSemanticStatus): WorkbenchAgentStatusView['glyph'] {
	switch (value.kind) {
		case 'needs-attention':
		case 'failed':
		case 'canceled':
		case 'interrupted':
		case 'succeeded':
		case 'working':
			return '●';
		case 'idle':
			return '○';
		case 'unknown':
		case 'ended':
			return '•';
	}
}
