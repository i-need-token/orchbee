import { expect, it } from 'vitest';
import type { WorkbenchAgentObservation, WorkbenchAgentSnapshot } from '../../../workbench';
import type { AgentSessionSummary } from '../../server-connection';
import { projectServerAgentBoardRows } from '../projectServerAgentBoardRows';

it('uses the same completion fact for the board, fences replacement generations and preserves provider blockers', () => {
	const id = '2'.repeat(32);
	const generation = '1'.repeat(32);
	const summary: AgentSessionSummary = {
		sessionId: 'session',
		providerId: 'codex',
		parentSessionId: null,
		lifecycle: 'live',
		executionLocation: { executionLocationId: 'local' },
		pendingRequestCount: 0,
		observation: {
			source: 'terminal',
			fidelity: 'heuristic',
			freshness: 'fresh',
			generation: 1,
			sequence: 1,
		},
		presentationAttachments: [
			{ presentationId: id, generation, navigable: true, visibility: 'visible' },
		],
		status: { kind: 'idle' },
	};
	const fact: WorkbenchAgentObservation = {
		agent: 'codex',
		binding: generation + ':epoch:terminal',
		sequence: 1n,
		state: 'idle',
		completion: 'awaiting-input',
	};
	const snapshot = (current: WorkbenchAgentObservation): WorkbenchAgentSnapshot => ({
		complete: true,
		presentations: new Map([[id, current]]),
	});
	expect(projectServerAgentBoardRows([summary], snapshot(fact))[0]?.status.kind).toBe('succeeded');
	expect(
		projectServerAgentBoardRows(
			[{ ...summary, status: { kind: 'succeeded' } }],
			snapshot({ ...fact, state: 'done', completion: 'acknowledged' }),
		)[0]?.status.kind,
	).toBe('idle');
	expect(
		projectServerAgentBoardRows(
			[summary],
			snapshot({ ...fact, binding: 'replacement:epoch:terminal' }),
		)[0]?.status.kind,
	).toBe('idle');
	expect(
		projectServerAgentBoardRows(
			[{ ...summary, status: { kind: 'needs-attention', count: 1 } }],
			snapshot(fact),
		)[0]?.status,
	).toEqual({ kind: 'needs-attention', count: 1 });
});
