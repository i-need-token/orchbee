import type {
	WorkbenchAgentObservation,
	WorkbenchAgentState,
} from '../types/workbench-agent-observation';

/** Shared Server-fact projection; provider state and unread versions remain untouched. */
export function resolveWorkbenchAgentDisplayState(
	observation: Pick<WorkbenchAgentObservation, 'state' | 'completion'>,
): WorkbenchAgentState {
	if (observation.state === 'idle' && observation.completion === 'awaiting-input') return 'done';
	if (observation.state === 'done' && observation.completion === 'acknowledged') return 'idle';
	return observation.state;
}
