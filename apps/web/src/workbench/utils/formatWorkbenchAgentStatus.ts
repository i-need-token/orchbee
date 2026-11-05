import { agentObservationMessages, type IntlShape, type MessageDescriptor } from '@orchbee/i18n';

import type { WorkbenchAgentState } from '../types/workbench-agent-observation';

export interface WorkbenchAgentStatusView {
	/** The TUI's text glyph, which a Tab's status adornment draws. */
	readonly glyph: '●' | '•' | '○';
	/** The sidebar's drawn glyph: a filled hexagon, its outline, or a dot (ORC-320). */
	readonly shape: 'dot' | 'hex' | 'ring';
	readonly status: string;
	readonly tone: 'attention' | 'danger' | 'working' | 'idle' | 'neutral' | 'success';
}

interface WorkbenchAgentStateView extends Pick<
	WorkbenchAgentStatusView,
	'glyph' | 'shape' | 'tone'
> {
	readonly message: MessageDescriptor;
}

/** The TUI's state glyphs and fixed traffic-light tones. */
const STATE_VIEWS: Readonly<Record<WorkbenchAgentState, WorkbenchAgentStateView>> = {
	blocked: {
		glyph: '●',
		message: agentObservationMessages.statusNeedsAttention,
		shape: 'hex',
		tone: 'attention',
	},
	// Done also covers Working to Idle once the Server holds that fact (ORC-332, ORC-339).
	done: {
		glyph: '●',
		message: agentObservationMessages.statusSucceeded,
		shape: 'hex',
		tone: 'success',
	},
	error: {
		glyph: '●',
		message: agentObservationMessages.statusFailed,
		shape: 'hex',
		tone: 'danger',
	},
	idle: { glyph: '○', message: agentObservationMessages.statusReady, shape: 'ring', tone: 'idle' },
	unknown: {
		glyph: '•',
		message: agentObservationMessages.statusUnknown,
		shape: 'dot',
		tone: 'neutral',
	},
	working: {
		glyph: '●',
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
		glyph: view.glyph,
		shape: view.shape,
		status: intl.formatMessage(view.message),
		tone: view.tone,
	};
}
