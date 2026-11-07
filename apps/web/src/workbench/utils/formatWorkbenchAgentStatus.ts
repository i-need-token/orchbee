import { agentObservationMessages, type IntlShape, type MessageDescriptor } from '@orchbee/i18n';

import type { WorkbenchAgentState } from '../types/workbench-agent-observation';

export interface WorkbenchAgentStatusView {
	/** The sidebar's drawn glyph: a filled hexagon, its outline, or a dot (ORC-320). */
	readonly shape: 'dot' | 'hex' | 'ring';
	readonly status: string;
	readonly tone: 'attention' | 'danger' | 'working' | 'idle' | 'neutral' | 'success';
}

interface WorkbenchAgentStateView extends Pick<WorkbenchAgentStatusView, 'shape' | 'tone'> {
	readonly message: MessageDescriptor;
}

/** Each Agent state's drawn shape and fixed traffic-light tone. */
const STATE_VIEWS: Readonly<Record<WorkbenchAgentState, WorkbenchAgentStateView>> = {
	blocked: {
		message: agentObservationMessages.statusNeedsAttention,
		shape: 'hex',
		tone: 'attention',
	},
	// Done also covers Working to Idle once the Server holds that fact (ORC-332, ORC-339).
	done: {
		message: agentObservationMessages.statusSucceeded,
		shape: 'hex',
		tone: 'success',
	},
	error: {
		message: agentObservationMessages.statusFailed,
		shape: 'hex',
		tone: 'danger',
	},
	idle: { message: agentObservationMessages.statusReady, shape: 'ring', tone: 'idle' },
	unknown: {
		message: agentObservationMessages.statusUnknown,
		shape: 'dot',
		tone: 'neutral',
	},
	working: {
		message: agentObservationMessages.statusWorking,
		shape: 'hex',
		tone: 'working',
	},
};

export function formatWorkbenchAgentStatus(
	intl: IntlShape,
	state: WorkbenchAgentState,
): WorkbenchAgentStatusView {
	const view = STATE_VIEWS[state];
	return {
		shape: view.shape,
		status: intl.formatMessage(view.message),
		tone: view.tone,
	};
}
