import {
	AgentRuntimeSnapshotSchema,
	type AgentRuntimeSnapshot,
	type AgentSnapshotSource,
} from '@orchbee/agent';
import { describe, expect, it, vi } from 'vitest';

import type { TerminalTrailingBlankRowsPreferenceSource } from '../../../terminal-interaction-settings';
import { createTerminalTrailingBlankRowsPresentationSource } from '../terminal-trailing-blank-rows-presentation-source';

describe('terminal trailing blank rows presentation source', () => {
	it('enables presentation only for a detected non-ended Agent with the preference on', () => {
		const agent = createAgentSource(emptyAgentSnapshot());
		const preference = createPreferenceSource(true);
		const source = createTerminalTrailingBlankRowsPresentationSource({
			agentObservationSource: agent,
			preferenceSource: preference,
			presentationId: 'presentation:target',
		});
		const listener = vi.fn<(enabled: boolean) => void>();
		const unsubscribe = source.subscribe(listener);

		expect(source.current()).toBe(false);
		agent.setSnapshot(agentSnapshot('presentation:target', 'live'));
		preference.set(false);
		expect(source.current()).toBe(false);
		preference.set(true);
		agent.setSnapshot(agentSnapshot('presentation:other', 'live'));
		agent.setSnapshot(agentSnapshot('presentation:target', 'live'));
		agent.setSnapshot(agentSnapshot('presentation:target', 'ended'));
		expect(source.current()).toBe(false);
		expect(listener.mock.calls).toEqual([[true], [false], [true], [false], [true], [false]]);

		unsubscribe();
		preference.set(false);
		expect(listener).toHaveBeenCalledTimes(6);
	});

	it('stays disabled for ordinary terminals and missing application capabilities', () => {
		const agent = createAgentSource(agentSnapshot('presentation:other', 'live'));
		const preference = createPreferenceSource(true);

		expect(
			createTerminalTrailingBlankRowsPresentationSource({
				agentObservationSource: agent,
				preferenceSource: preference,
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
		const agent = createAgentSource(agentSnapshot('presentation:target', 'live'));
		agent.subscribe = () => {
			throw new Error('injected observation failure');
		};
		const preference = createPreferenceSource(true);
		const source = createTerminalTrailingBlankRowsPresentationSource({
			agentObservationSource: agent,
			preferenceSource: preference,
			presentationId: 'presentation:target',
		});
		const listener = vi.fn<(enabled: boolean) => void>();

		expect(source.current()).toBe(true);
		expect(() => source.subscribe(listener)).not.toThrow();
		expect(source.current()).toBe(false);
		expect(listener).toHaveBeenCalledWith(false);
	});
});

interface MutableAgentSource extends AgentSnapshotSource {
	setSnapshot(snapshot: AgentRuntimeSnapshot): void;
	subscribe(listener: () => void): () => void;
}

function createAgentSource(initialSnapshot: AgentRuntimeSnapshot): MutableAgentSource {
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

interface MutablePreferenceSource extends TerminalTrailingBlankRowsPreferenceSource {
	set(enabled: boolean): void;
}

function createPreferenceSource(initial: boolean): MutablePreferenceSource {
	let enabled = initial;
	const listeners = new Set<(next: boolean) => void>();
	return {
		current: () => enabled,
		set: (next) => {
			enabled = next;
			for (const listener of listeners) listener(next);
		},
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
}

function agentSnapshot(presentationId: string, lifecycle: 'ended' | 'live'): AgentRuntimeSnapshot {
	return AgentRuntimeSnapshotSchema.parse({
		revision: lifecycle === 'live' ? 1 : 2,
		sessions: [
			{
				activityHint: null,
				capabilities: ['sessionLifecycle'],
				executionLocation: { executionLocationId: 'location:1' },
				lastOutcome: null,
				lifecycle,
				observation: {
					fidelity: 'rich',
					freshness: 'fresh',
					generation: 1,
					sequence: 1,
					source: 'native',
				},
				parentSessionId: null,
				pendingRequests: [],
				presentationAttachments: [
					{
						generation: 'generation:1',
						navigable: true,
						presentationId,
						visibility: 'visible',
					},
				],
				providerId: 'codex',
				sessionId: 'session:1',
				turn: { kind: 'idle' },
			},
		],
	});
}

function emptyAgentSnapshot(): AgentRuntimeSnapshot {
	return AgentRuntimeSnapshotSchema.parse({ revision: 0, sessions: [] });
}
