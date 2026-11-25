import { agentObservationMessages, type IntlShape, type MessageDescriptor } from '@orchbee/i18n';

import type { WorkbenchAgentState } from '../types/workbench-agent-observation';

export interface WorkbenchAgentStatusView {
	/**
	 * The glyph's figure: a hexagon, or a dot while the state is unknown. Whether it is drawn solid or
	 * hollow says whether its Agent is the current one, so the place that draws it decides that.
	 */
	readonly figure: 'dot' | 'hexagon';
	readonly status: string;
	readonly tone: 'attention' | 'danger' | 'working' | 'idle' | 'neutral' | 'success';
}

interface WorkbenchAgentStateView extends Pick<WorkbenchAgentStatusView, 'figure' | 'tone'> {
	readonly message: MessageDescriptor;
}

/** Each Agent state's figure and fixed traffic-light tone. */
const STATE_VIEWS: Readonly<Record<WorkbenchAgentState, WorkbenchAgentStateView>> = {
	blocked: {
		message: agentObservationMessages.statusNeedsAttention,
		figure: 'hexagon',
		tone: 'attention',
	},
	// Done also covers Working to Idle once the Server holds that fact (ORC-332, ORC-339).
	done: {
		message: agentObservationMessages.statusSucceeded,
		figure: 'hexagon',
		tone: 'success',
	},
	error: {
		message: agentObservationMessages.statusFailed,
		figure: 'hexagon',
		tone: 'danger',
	},
	idle: { message: agentObservationMessages.statusReady, figure: 'hexagon', tone: 'idle' },
	// Opening breathes with the working glyph until the Agent's screen is recognised.
	opening: {
		message: agentObservationMessages.statusStarting,
		figure: 'hexagon',
		tone: 'working',
	},
	unknown: {
		message: agentObservationMessages.statusUnknown,
		figure: 'dot',
		tone: 'neutral',
	},
	working: {
		message: agentObservationMessages.statusWorking,
		figure: 'hexagon',
		tone: 'working',
	},
};

export function formatWorkbenchAgentStatus(
	intl: IntlShape,
	state: WorkbenchAgentState,
): WorkbenchAgentStatusView {
	const view = STATE_VIEWS[state];
	return {
		figure: view.figure,
		status: intl.formatMessage(view.message),
		tone: view.tone,
	};
}
