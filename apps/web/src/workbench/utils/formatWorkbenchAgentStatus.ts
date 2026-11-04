import { agentObservationMessages, type IntlShape, type MessageDescriptor } from '@orchbee/i18n';

import type { WorkbenchAgentState } from '../types/workbench-agent-observation';

export interface WorkbenchAgentStatusView {
	readonly glyph: '●' | '•' | '○';
	readonly status: string;
	readonly tone: 'attention' | 'danger' | 'working' | 'idle' | 'neutral' | 'success';
}

interface WorkbenchAgentStateView extends Pick<WorkbenchAgentStatusView, 'glyph' | 'tone'> {
	readonly message: MessageDescriptor;
}

/** The TUI's state glyphs and fixed traffic-light tones. */
const STATE_VIEWS: Readonly<Record<WorkbenchAgentState, WorkbenchAgentStateView>> = {
	blocked: {
		glyph: '●',
		message: agentObservationMessages.statusNeedsAttention,
		tone: 'attention',
	},
	done: { glyph: '●', message: agentObservationMessages.statusSucceeded, tone: 'success' },
	error: { glyph: '●', message: agentObservationMessages.statusFailed, tone: 'danger' },
	idle: { glyph: '○', message: agentObservationMessages.statusReady, tone: 'idle' },
	unknown: { glyph: '•', message: agentObservationMessages.statusUnknown, tone: 'neutral' },
	working: { glyph: '●', message: agentObservationMessages.statusWorking, tone: 'working' },
};

export function formatWorkbenchAgentStatus(
	intl: IntlShape,
	state: WorkbenchAgentState,
): WorkbenchAgentStatusView {
	const view = STATE_VIEWS[state];
	return { glyph: view.glyph, status: intl.formatMessage(view.message), tone: view.tone };
}
