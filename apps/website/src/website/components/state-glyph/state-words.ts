import { agentObservationMessages, type MessageDescriptor } from '@orchbee/i18n';

import type { AgentState } from './StateGlyph';

/** Every Agent state the glyph draws. */
export const AGENT_STATES: readonly AgentState[] = [
	'working',
	'attention',
	'done',
	'idle',
	'failed',
];

/** The product's own word for each Agent state. */
export const AGENT_STATE_WORDS: Readonly<Record<AgentState, MessageDescriptor>> = {
	working: agentObservationMessages.statusWorking,
	attention: agentObservationMessages.statusNeedsAttention,
	done: agentObservationMessages.statusSucceeded,
	idle: agentObservationMessages.statusReady,
	failed: agentObservationMessages.statusFailed,
};
