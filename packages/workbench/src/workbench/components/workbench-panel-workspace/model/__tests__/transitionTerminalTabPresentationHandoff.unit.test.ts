import { describe, expect, it } from 'vitest';

import {
	transitionTerminalTabPresentationHandoff,
	type TerminalTabPresentationHandoffState,
} from '../transitionTerminalTabPresentationHandoff';

function observe(
	state: TerminalTabPresentationHandoffState | null,
	targetTabId: string | null,
	expectedPresentationIds: readonly string[],
	options: {
		readonly outgoingAvailable?: boolean;
		readonly targetRevision?: string | null;
	} = {},
): TerminalTabPresentationHandoffState {
	let targetRevision = options.targetRevision;
	if (targetRevision === undefined) {
		targetRevision = targetTabId === null ? null : `${targetTabId}-layout-1`;
	}
	return transitionTerminalTabPresentationHandoff(state, {
		expectedPresentationIds,
		outgoingAvailable: options.outgoingAvailable ?? true,
		targetRevision,
		targetTabId,
		type: 'target-observed',
	});
}

function ready(
	state: TerminalTabPresentationHandoffState,
	presentationId: string,
	readiness: 'activation-ready' | 'display-ready',
	options: {
		readonly epoch?: number;
		readonly targetRevision?: string;
		readonly targetTabId?: string;
	} = {},
): TerminalTabPresentationHandoffState {
	if (state.kind === 'settled') throw new Error('Expected a pending readiness target.');
	return transitionTerminalTabPresentationHandoff(state, {
		epoch: options.epoch ?? state.epoch,
		presentationId,
		readiness,
		targetRevision: options.targetRevision ?? state.targetRevision,
		targetTabId: options.targetTabId ?? state.targetTabId,
		type: 'presentation-ready',
	});
}

describe('transitionTerminalTabPresentationHandoff', () => {
	it('settles the initial target without manufacturing an outgoing layer', () => {
		expect(observe(null, 'tab-a', ['terminal-a'])).toEqual({
			displayedRevision: 'tab-a-layout-1',
			displayedTabId: 'tab-a',
			epoch: 0,
			kind: 'settled',
		});
	});

	it('treats a same-Tab layout revision as a new atomic handoff', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		const preparing = observe(initial, 'tab-a', ['terminal-a', 'terminal-b'], {
			targetRevision: 'tab-a-layout-2',
		});

		expect(preparing).toMatchObject({
			displayedRevision: 'tab-a-layout-1',
			displayedTabId: 'tab-a',
			epoch: 1,
			kind: 'preparing',
			targetRevision: 'tab-a-layout-2',
			targetTabId: 'tab-a',
		});
		const oneReady = ready(preparing, 'terminal-a', 'activation-ready');
		expect(oneReady).toMatchObject({
			activationReadyPresentationIds: ['terminal-a'],
			displayReadyPresentationIds: ['terminal-a'],
			kind: 'preparing',
		});
		expect(ready(oneReady, 'terminal-b', 'activation-ready')).toEqual({
			displayedRevision: 'tab-a-layout-2',
			displayedTabId: 'tab-a',
			epoch: 1,
			kind: 'settled',
		});
	});

	it('keeps the outgoing revision until every target is display ready and inert until activation', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		let state = observe(initial, 'tab-b', ['terminal-b-1', 'terminal-b-2']);
		state = ready(state, 'terminal-b-2', 'display-ready');
		expect(state).toMatchObject({ displayedTabId: 'tab-a', kind: 'preparing' });
		state = ready(state, 'terminal-b-1', 'display-ready');
		expect(state).toMatchObject({
			displayedRevision: 'tab-b-layout-1',
			displayedTabId: 'tab-b',
			kind: 'activating',
		});
		state = ready(state, 'terminal-b-1', 'activation-ready');
		expect(state).toMatchObject({ kind: 'activating' });
		expect(ready(state, 'terminal-b-2', 'activation-ready')).toMatchObject({
			displayedRevision: 'tab-b-layout-1',
			displayedTabId: 'tab-b',
			kind: 'settled',
		});
	});

	it('preserves readiness when only the current target subset order changes', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		let state = observe(initial, 'tab-b', ['terminal-b-1', 'terminal-b-2']);
		state = ready(state, 'terminal-b-1', 'activation-ready');
		const reordered = observe(state, 'tab-b', ['terminal-b-2', 'terminal-b-1']);

		expect(reordered).toBe(state);
		expect(ready(reordered, 'terminal-b-2', 'activation-ready')).toMatchObject({
			kind: 'settled',
		});
	});

	it('retargets a pending subset without advancing the layout epoch', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		let state = observe(initial, 'tab-b', ['terminal-b-1', 'terminal-b-2']);
		state = ready(state, 'terminal-b-1', 'activation-ready');

		expect(observe(state, 'tab-b', ['terminal-b-1'])).toEqual({
			displayedRevision: 'tab-b-layout-1',
			displayedTabId: 'tab-b',
			epoch: 1,
			kind: 'settled',
		});
	});

	it('treats activation readiness as display readiness under reordered completion', () => {
		const preparing = observe(observe(null, 'tab-a', ['terminal-a']), 'tab-b', ['terminal-b']);

		expect(ready(preparing, 'terminal-b', 'activation-ready')).toMatchObject({
			displayedRevision: 'tab-b-layout-1',
			kind: 'settled',
		});
	});

	it('ignores duplicate, stale, wrong-Tab, and wrong-revision readiness', () => {
		const preparing = observe(observe(null, 'tab-a', ['terminal-a']), 'tab-b', ['terminal-b']);
		const displayReady = ready(preparing, 'terminal-b', 'display-ready');

		expect(ready(displayReady, 'terminal-b', 'display-ready')).toBe(displayReady);
		expect(ready(preparing, 'terminal-b', 'activation-ready', { epoch: 0 })).toBe(preparing);
		expect(ready(preparing, 'terminal-b', 'activation-ready', { targetTabId: 'tab-other' })).toBe(
			preparing,
		);
		expect(
			ready(preparing, 'terminal-b', 'activation-ready', {
				targetRevision: 'tab-b-layout-old',
			}),
		).toBe(preparing);
	});

	it('supersedes from either barrier while retaining the actually displayed revision', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		const preparingB = observe(initial, 'tab-b', ['terminal-b']);
		const preparingC = observe(preparingB, 'tab-c', ['terminal-c']);
		expect(preparingC).toMatchObject({
			displayedRevision: 'tab-a-layout-1',
			epoch: 2,
			kind: 'preparing',
		});

		const activatingB = ready(preparingB, 'terminal-b', 'display-ready');
		const fromActivation = observe(activatingB, 'tab-c', ['terminal-c']);
		expect(fromActivation).toMatchObject({
			displayedRevision: 'tab-b-layout-1',
			epoch: 2,
			kind: 'preparing',
		});
	});

	it('prepares a return to the frozen displayed revision under one new epoch', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		const preparingB = observe(initial, 'tab-b', ['terminal-b']);
		const returningA = observe(preparingB, 'tab-a', ['terminal-a']);

		expect(returningA).toMatchObject({
			displayedRevision: 'tab-a-layout-1',
			epoch: 2,
			kind: 'preparing',
			targetRevision: 'tab-a-layout-1',
		});
	});

	it('settles no-surface targets and can continue after its outgoing snapshot disappears', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		expect(observe(initial, 'tab-b', [])).toEqual({
			displayedRevision: 'tab-b-layout-1',
			displayedTabId: 'tab-b',
			epoch: 1,
			kind: 'settled',
		});

		const preparing = observe(initial, 'tab-b', ['terminal-b']);
		const withoutOutgoing = observe(preparing, 'tab-b', ['terminal-b'], {
			outgoingAvailable: false,
		});
		expect(withoutOutgoing).toMatchObject({
			displayedRevision: null,
			displayedTabId: null,
			epoch: 1,
			kind: 'preparing',
		});
		expect(ready(withoutOutgoing, 'terminal-b', 'activation-ready')).toMatchObject({
			kind: 'settled',
		});
	});

	it('deduplicates the trusted expected identity set deterministically', () => {
		const preparing = observe(observe(null, 'tab-a', ['terminal-a']), 'tab-b', [
			'terminal-b',
			'terminal-b',
		]);

		expect(preparing).toMatchObject({ expectedPresentationIds: ['terminal-b'] });
	});
});
