import { describe, expect, it } from 'vitest';

import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAgentState,
} from '../../types/workbench-agent-observation';
import { WorkbenchAgentAttention } from '../WorkbenchAgentAttention';

function agent(
	state: WorkbenchAgentState,
	sequence: bigint,
	binding = 'terminal:1',
): WorkbenchAgentObservation {
	return { agent: 'codex', binding, sequence, state };
}

function snapshot(
	entries: Readonly<Record<string, WorkbenchAgentObservation>>,
	complete = true,
): WorkbenchAgentSnapshot {
	return { complete, presentations: new Map(Object.entries(entries)) };
}

function source(initial: WorkbenchAgentSnapshot) {
	let current = initial;
	const listeners = new Set<() => void>();
	return {
		getSnapshot: () => current,
		subscribe: (listener: () => void) => {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		},
		publish(next: WorkbenchAgentSnapshot) {
			current = next;
			for (const listener of listeners) listener();
		},
	};
}

describe('Workbench Agent attention', () => {
	it('starts from the first complete observation and marks only later versions unread', () => {
		const server = source(snapshot({ terminal: agent('working', 1n) }, false));
		const viewer = new WorkbenchAgentAttention(server);
		const unsubscribe = viewer.subscribe(() => {});
		// Nothing is unread before every Presentation has been answered once.
		server.publish(snapshot({ terminal: agent('done', 2n) }, false));
		expect(viewer.getSnapshot().unread).toEqual(new Set());
		const answered = { other: agent('idle', 1n, 'terminal:2'), terminal: agent('done', 2n) };
		server.publish(snapshot(answered));
		expect(viewer.getSnapshot().unread).toEqual(new Set());
		// Another frame with the same versions is the same state.
		server.publish(snapshot(answered));
		expect(viewer.getSnapshot().unread).toEqual(new Set());
		server.publish(snapshot({ ...answered, terminal: agent('working', 3n) }));
		expect(viewer.getSnapshot().unread).toEqual(new Set(['terminal']));
		unsubscribe();
	});

	it('takes an already complete observation as the baseline it was created with', () => {
		const server = source(snapshot({ terminal: agent('idle', 1n) }));
		const viewer = new WorkbenchAgentAttention(server);
		const unsubscribe = viewer.subscribe(() => {});
		expect(viewer.getSnapshot().unread).toEqual(new Set());
		server.publish(snapshot({ terminal: agent('working', 2n) }));
		expect(viewer.getSnapshot().unread).toEqual(new Set(['terminal']));
		unsubscribe();
	});

	it('marks a replacement terminal and a returning Agent unread', () => {
		const server = source(snapshot({ terminal: agent('idle', 1n) }));
		const viewer = new WorkbenchAgentAttention(server);
		const unsubscribe = viewer.subscribe(() => {});
		server.publish(snapshot({ terminal: agent('idle', 1n, 'terminal:2') }));
		expect(viewer.getSnapshot().unread).toEqual(new Set(['terminal']));
		viewer.acknowledge(viewer.getSnapshot(), new Set(['terminal']), true);
		expect(viewer.getSnapshot().unread).toEqual(new Set());
		server.publish(snapshot({}));
		server.publish(snapshot({ terminal: agent('idle', 1n, 'terminal:2') }));
		expect(viewer.getSnapshot().unread).toEqual(new Set(['terminal']));
		unsubscribe();
	});

	it('acknowledges only a focused, current paint, and only for this Client', () => {
		const server = source(snapshot({ terminal: agent('idle', 1n) }));
		const viewer = new WorkbenchAgentAttention(server);
		const other = new WorkbenchAgentAttention(server);
		const unsubscribe = viewer.subscribe(() => {});
		const unsubscribeOther = other.subscribe(() => {});
		server.publish(snapshot({ terminal: agent('blocked', 2n) }));
		const painted = viewer.getSnapshot();
		server.publish(snapshot({ terminal: agent('error', 3n) }));
		viewer.acknowledge(painted, new Set(['terminal']), true);
		viewer.acknowledge(viewer.getSnapshot(), new Set(['another-terminal']), true);
		viewer.acknowledge(viewer.getSnapshot(), new Set(['terminal']), false);
		expect(viewer.getSnapshot().unread).toEqual(new Set(['terminal']));
		const shown = server.getSnapshot();
		viewer.acknowledge(viewer.getSnapshot(), new Set(['terminal']), true);
		expect(viewer.getSnapshot().unread).toEqual(new Set());
		expect(other.getSnapshot().unread).toEqual(new Set(['terminal']));
		expect(server.getSnapshot()).toBe(shown);
		unsubscribe();
		unsubscribeOther();
	});

	it('keeps a change that returned to the previous state unread', () => {
		const server = source(snapshot({ terminal: agent('idle', 1n) }));
		const viewer = new WorkbenchAgentAttention(server);
		const unsubscribe = viewer.subscribe(() => {});
		server.publish(snapshot({ terminal: agent('working', 2n) }));
		server.publish(snapshot({ terminal: agent('idle', 3n) }));
		expect(viewer.getSnapshot().unread).toEqual(new Set(['terminal']));
		unsubscribe();
	});

	it('shows a completion the person typed into as idle until its state changes', () => {
		const server = source(snapshot({ terminal: agent('done', 1n) }));
		const viewer = new WorkbenchAgentAttention(server);
		const unsubscribe = viewer.subscribe(() => {});
		const shown = () => viewer.getSnapshot().presentation.presentations.get('terminal')?.state;
		viewer.terminalEdited('unrelated-terminal');
		expect(shown()).toBe('done');
		viewer.terminalEdited('terminal');
		expect(shown()).toBe('idle');
		expect(viewer.getSnapshot().observation.presentations.get('terminal')?.state).toBe('done');
		server.publish(snapshot({ terminal: agent('done', 2n) }));
		expect(shown()).toBe('done');
		server.publish(snapshot({ terminal: agent('working', 3n) }));
		viewer.terminalEdited('terminal');
		expect(shown()).toBe('working');
		unsubscribe();
	});
});
