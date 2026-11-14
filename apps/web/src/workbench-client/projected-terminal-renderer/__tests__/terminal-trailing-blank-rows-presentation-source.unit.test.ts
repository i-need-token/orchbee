import type { WorkbenchAgentSnapshot, WorkbenchAgentSource } from '../../../workbench';
import { describe, expect, it, vi } from 'vitest';

import { createTerminalTrailingBlankRowsPresentationSource } from '../terminal-trailing-blank-rows-presentation-source';

describe('terminal trailing blank rows presentation source', () => {
	it('enables presentation only for a terminal with an observed Agent', () => {
		const agent = createAgentSource(agentSnapshot());
		const source = createTerminalTrailingBlankRowsPresentationSource({
			agentObservationSource: agent,
			presentationId: 'presentation:target',
		});
		const listener = vi.fn<(enabled: boolean) => void>();
		const unsubscribe = source.subscribe(listener);

		expect(source.current()).toBe(false);
		agent.setSnapshot(agentSnapshot('presentation:target'));
		expect(source.current()).toBe(true);
		agent.setSnapshot(agentSnapshot('presentation:other'));
		agent.setSnapshot(agentSnapshot('presentation:target'));
		// The Agent exited, so the Server no longer reports one in the terminal.
		agent.setSnapshot(agentSnapshot());
		expect(source.current()).toBe(false);
		expect(listener.mock.calls).toEqual([[true], [false], [true], [false]]);

		unsubscribe();
		agent.setSnapshot(agentSnapshot('presentation:target'));
		expect(listener).toHaveBeenCalledTimes(4);
	});

	it('stays disabled for ordinary terminals and missing application capabilities', () => {
		const agent = createAgentSource(agentSnapshot('presentation:other'));

		expect(
			createTerminalTrailingBlankRowsPresentationSource({
				agentObservationSource: agent,
				presentationId: 'presentation:target',
			}).current(),
		).toBe(false);
		expect(
			createTerminalTrailingBlankRowsPresentationSource({
				presentationId: 'presentation:target',
			}).current(),
		).toBe(false);
	});

	it('fails closed when an upstream source cannot be observed', () => {
		const agent = createAgentSource(agentSnapshot('presentation:target'));
		agent.subscribe = () => {
			throw new Error('injected observation failure');
		};
		const source = createTerminalTrailingBlankRowsPresentationSource({
			agentObservationSource: agent,
			presentationId: 'presentation:target',
		});
		const listener = vi.fn<(enabled: boolean) => void>();

		expect(source.current()).toBe(true);
		expect(() => source.subscribe(listener)).not.toThrow();
		expect(source.current()).toBe(false);
		expect(listener).toHaveBeenCalledWith(false);
	});
});

interface MutableAgentSource extends WorkbenchAgentSource {
	setSnapshot(snapshot: WorkbenchAgentSnapshot): void;
	subscribe(listener: () => void): () => void;
}

function createAgentSource(initialSnapshot: WorkbenchAgentSnapshot): MutableAgentSource {
	let snapshot = initialSnapshot;
	const listeners = new Set<() => void>();
	return {
		getSnapshot: () => snapshot,
		setSnapshot: (next) => {
			snapshot = next;
			for (const listener of listeners) listener();
		},
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
}

function agentSnapshot(...presentationIds: readonly string[]): WorkbenchAgentSnapshot {
	return {
		complete: true,
		presentations: new Map(
			presentationIds.map((presentationId) => [
				presentationId,
				{ agent: 'codex', binding: 'terminal:1', sequence: 1n, state: 'idle' },
			]),
		),
	};
}
