import {
	AgentRuntimeSnapshotSchema,
	projectAgentSession,
	type AgentRuntimeSnapshot,
} from '@orchbee/agent';
import { describe, expect, it } from 'vitest';

import { WorkbenchAgentAttention } from '../WorkbenchAgentAttention';

function observation(
	state: 'working' | 'waiting' | 'succeeded' | 'failed' | 'idle' | 'unknown',
	revision = 1,
): AgentRuntimeSnapshot {
	return AgentRuntimeSnapshotSchema.parse({
		revision,
		sessions: [
			{
				activityHint: null,
				capabilities: ['sessionLifecycle', 'turnLifecycle', 'approvals', 'activity'],
				executionLocation: { executionLocationId: 'execution' },
				lastOutcome:
					state === 'succeeded' || state === 'failed'
						? { eventId: 'outcome', kind: state, turnId: 'turn' }
						: null,
				lifecycle: 'live',
				observation: {
					fidelity: 'rich',
					freshness: 'fresh',
					generation: 0,
					sequence: revision,
					source: 'hook',
				},
				parentSessionId: null,
				pendingRequests:
					state === 'waiting'
						? [
								{
									actionPath: 'navigate-only',
									kind: 'approval',
									requestId: 'request',
									turnId: null,
								},
							]
						: [],
				presentationAttachments: [
					{
						generation: 'binding',
						navigable: true,
						presentationId: 'terminal',
						visibility: 'visible',
					},
				],
				providerId: 'codex',
				sessionId: 'session',
				turn:
					state === 'working'
						? { kind: 'active', phase: 'working', activity: 'thinking', turnId: 'turn' }
						: { kind: state === 'unknown' ? 'unknown' : 'idle' },
			},
		],
	});
}

function source(initial = observation('idle')) {
	let snapshot = initial;
	const listeners = new Set<() => void>();
	return {
		getSnapshot: () => snapshot,
		subscribe: (listener: () => void) => {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		},
		publish(next: AgentRuntimeSnapshot) {
			snapshot = next;
			for (const listener of listeners) listener();
		},
	};
}

describe('Workbench Agent acknowledgement', () => {
	it('keeps completion on focus, consumes it on editing, and preserves canonical outcomes and later completions', () => {
		const runtime = source(observation('succeeded'));
		const viewer = new WorkbenchAgentAttention(runtime);
		const unsubscribe = viewer.subscribe(() => {});
		const status = () =>
			viewer.getSnapshot().presentation.sessions.map(projectAgentSession)[0]?.status.kind;
		viewer.acknowledge(viewer.getSnapshot(), new Set(['terminal']), true);
		expect(status()).toBe('succeeded');
		viewer.terminalEdited('unrelated-terminal');
		expect(status()).toBe('succeeded');
		viewer.terminalEdited('terminal');
		expect(status()).toBe('idle');
		expect(runtime.getSnapshot().sessions.map(projectAgentSession)[0]?.status.kind).toBe(
			'succeeded',
		);
		runtime.publish(observation('succeeded', 2));
		expect(status()).toBe('idle');
		const next = observation('succeeded', 3);
		runtime.publish(
			AgentRuntimeSnapshotSchema.parse({
				...next,
				sessions: next.sessions.map((session) => ({
					...session,
					lastOutcome: { eventId: 'next-outcome', kind: 'succeeded', turnId: 'next-turn' },
				})),
			}),
		);
		expect(status()).toBe('succeeded');
		viewer.terminalEdited('terminal');
		runtime.publish(observation('working', 4));
		viewer.terminalEdited('terminal');
		expect(status()).toBe('working');
		runtime.publish(observation('failed', 5));
		viewer.terminalEdited('terminal');
		expect(status()).toBe('failed');
		unsubscribe();
	});

	it.each(['working', 'waiting', 'succeeded', 'failed', 'idle', 'unknown'] as const)(
		'reading %s only clears this Client’s unread mark',
		(state) => {
			const runtime = source(observation(state === 'unknown' ? 'working' : 'unknown'));
			const viewer = new WorkbenchAgentAttention(runtime);
			const other = new WorkbenchAgentAttention(runtime);
			const unsubscribe = viewer.subscribe(() => {});
			const unsubscribeOther = other.subscribe(() => {});
			expect(viewer.getSnapshot().unread.size).toBe(0);
			const next = observation(state, 2);
			runtime.publish(next);
			expect(viewer.getSnapshot().unread.size).toBe(1);
			const before = next.sessions.map(projectAgentSession);
			viewer.acknowledge(viewer.getSnapshot(), new Set(['terminal']), true);
			expect(viewer.getSnapshot().unread.size).toBe(0);
			expect(other.getSnapshot().unread.size).toBe(1);
			expect(runtime.getSnapshot()).toBe(next);
			expect(runtime.getSnapshot().sessions.map(projectAgentSession)).toEqual(before);
			unsubscribe();
			unsubscribeOther();
		},
	);

	it('rejects stale paints, other terminal views and unfocused windows', () => {
		const runtime = source();
		const viewer = new WorkbenchAgentAttention(runtime);
		const unsubscribe = viewer.subscribe(() => {});
		runtime.publish(observation('waiting', 2));
		const painted = viewer.getSnapshot();
		runtime.publish(observation('failed', 3));
		viewer.acknowledge(painted, new Set(['terminal']), true);
		viewer.acknowledge(viewer.getSnapshot(), new Set(['another-terminal']), true);
		viewer.acknowledge(viewer.getSnapshot(), new Set(['terminal']), false);
		expect(viewer.getSnapshot().unread.size).toBe(1);
		viewer.acknowledge(viewer.getSnapshot(), new Set(['terminal']), true);
		expect(viewer.getSnapshot().unread.size).toBe(0);
		unsubscribe();
	});

	it('does not reopen a read state for sequence or attachment metadata changes', () => {
		const runtime = source();
		const viewer = new WorkbenchAgentAttention(runtime);
		const unsubscribe = viewer.subscribe(() => {});
		runtime.publish(observation('working', 2));
		viewer.acknowledge(viewer.getSnapshot(), new Set(['terminal']), true);
		runtime.publish(observation('working', 3));
		const next = observation('working', 4);
		runtime.publish({
			...next,
			sessions: next.sessions.map((session) => ({ ...session, presentationAttachments: [] })),
		});
		expect(viewer.getSnapshot().unread.size).toBe(0);
		unsubscribe();
	});

	it('retains a change that returns to the previous state before rendering', () => {
		const runtime = source();
		const viewer = new WorkbenchAgentAttention(runtime);
		const unsubscribe = viewer.subscribe(() => {});
		runtime.publish(observation('working', 2));
		runtime.publish(observation('idle', 3));
		expect(viewer.getSnapshot().unread.size).toBe(1);
		expect(viewer.getSnapshot().observation.sessions.map(projectAgentSession)[0]?.status).toEqual({
			kind: 'idle',
		});
		unsubscribe();
	});
});
