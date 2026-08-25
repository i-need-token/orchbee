import { describe, expect, it } from 'vitest';

import {
	transitionTerminalTabPresentationHandoff,
	type TerminalTabPresentationHandoffState,
} from '../transitionTerminalTabPresentationHandoff';

function observe(
	state: TerminalTabPresentationHandoffState | null,
	targetTabId: string | null,
	expectedPresentationIds: readonly string[],
	outgoingAvailable = true,
): TerminalTabPresentationHandoffState {
	return transitionTerminalTabPresentationHandoff(state, {
		expectedPresentationIds,
		outgoingAvailable,
		targetTabId,
		type: 'target-observed',
	});
}

function ready(
	state: TerminalTabPresentationHandoffState,
	presentationId: string,
	readiness: 'activation-ready' | 'display-ready',
	epoch = state.epoch,
	targetTabId = state.kind === 'settled' ? state.displayedTabId : state.targetTabId,
): TerminalTabPresentationHandoffState {
	if (targetTabId === null) throw new Error('Expected a readiness target Tab.');
	return transitionTerminalTabPresentationHandoff(state, {
		epoch,
		presentationId,
		readiness,
		targetTabId,
		type: 'presentation-ready',
	});
}

describe('transitionTerminalTabPresentationHandoff', () => {
	it('settles the initial target without manufacturing an outgoing layer', () => {
		expect(observe(null, 'tab-a', ['terminal-a'])).toEqual({
			displayedTabId: 'tab-a',
			epoch: 0,
			kind: 'settled',
		});
	});

	it('keeps the outgoing Tab until every target presentation is display ready', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		const preparing = observe(initial, 'tab-b', ['terminal-b-1', 'terminal-b-2']);
		const oneReady = ready(preparing, 'terminal-b-2', 'display-ready');

		expect(preparing).toMatchObject({ displayedTabId: 'tab-a', epoch: 1, kind: 'preparing' });
		expect(oneReady).toMatchObject({
			displayReadyPresentationIds: ['terminal-b-2'],
			displayedTabId: 'tab-a',
			kind: 'preparing',
		});
		expect(ready(oneReady, 'terminal-b-1', 'display-ready')).toEqual({
			activationReadyPresentationIds: [],
			displayReadyPresentationIds: ['terminal-b-2', 'terminal-b-1'],
			displayedTabId: 'tab-b',
			epoch: 1,
			expectedPresentationIds: ['terminal-b-1', 'terminal-b-2'],
			kind: 'activating',
			targetTabId: 'tab-b',
		});
	});

	it('does not settle cached target content before every presentation is activation ready', () => {
		let state = observe(observe(null, 'tab-a', ['terminal-a']), 'tab-b', [
			'terminal-b-1',
			'terminal-b-2',
		]);
		state = ready(state, 'terminal-b-1', 'display-ready');
		state = ready(state, 'terminal-b-2', 'display-ready');
		state = ready(state, 'terminal-b-2', 'activation-ready');

		expect(state).toMatchObject({
			activationReadyPresentationIds: ['terminal-b-2'],
			displayedTabId: 'tab-b',
			kind: 'activating',
		});
		expect(ready(state, 'terminal-b-1', 'activation-ready')).toEqual({
			displayedTabId: 'tab-b',
			epoch: 1,
			kind: 'settled',
		});
	});

	it('preserves partial readiness when the same target identity set is reordered', () => {
		let state = observe(observe(null, 'tab-a', ['terminal-a']), 'tab-b', [
			'terminal-b-1',
			'terminal-b-2',
		]);
		state = ready(state, 'terminal-b-1', 'activation-ready');
		state = ready(state, 'terminal-b-2', 'display-ready');
		const reordered = observe(state, 'tab-b', ['terminal-b-2', 'terminal-b-1']);

		expect(reordered).toBe(state);
		expect(ready(reordered, 'terminal-b-2', 'activation-ready')).toEqual({
			displayedTabId: 'tab-b',
			epoch: 1,
			kind: 'settled',
		});
	});

	it('retargets the same Tab without advancing its epoch or discarding readiness', () => {
		let state = observe(observe(null, 'tab-a', ['terminal-a']), 'tab-b', [
			'terminal-b-1',
			'terminal-b-2',
		]);
		state = ready(state, 'terminal-b-1', 'activation-ready');

		expect(observe(state, 'tab-b', ['terminal-b-1'])).toEqual({
			displayedTabId: 'tab-b',
			epoch: 1,
			kind: 'settled',
		});
	});

	it('retains current-target readiness that arrives before the identity becomes required', () => {
		const preparing = observe(observe(null, 'tab-a', ['terminal-a']), 'tab-b', ['terminal-b-1']);
		const earlyReady = ready(preparing, 'terminal-b-2', 'activation-ready');
		const expanded = observe(earlyReady, 'tab-b', ['terminal-b-1', 'terminal-b-2']);

		expect(expanded).toMatchObject({
			activationReadyPresentationIds: ['terminal-b-2'],
			displayReadyPresentationIds: ['terminal-b-2'],
			epoch: 1,
			kind: 'preparing',
		});
		expect(ready(expanded, 'terminal-b-1', 'activation-ready')).toEqual({
			displayedTabId: 'tab-b',
			epoch: 1,
			kind: 'settled',
		});
	});

	it('preserves readiness ledgers when an activating target expands', () => {
		let state = observe(observe(null, 'tab-a', ['terminal-a']), 'tab-b', ['terminal-b-1']);
		state = ready(state, 'terminal-b-1', 'display-ready');
		state = ready(state, 'terminal-b-2', 'activation-ready');
		const expanded = observe(state, 'tab-b', ['terminal-b-1', 'terminal-b-2']);

		expect(expanded).toMatchObject({
			activationReadyPresentationIds: ['terminal-b-2'],
			displayReadyPresentationIds: ['terminal-b-1', 'terminal-b-2'],
			displayedTabId: 'tab-b',
			epoch: 1,
			kind: 'activating',
		});
		expect(ready(expanded, 'terminal-b-1', 'activation-ready')).toEqual({
			displayedTabId: 'tab-b',
			epoch: 1,
			kind: 'settled',
		});
	});

	it('treats activation readiness as display readiness under reordered completion', () => {
		const preparing = observe(observe(null, 'tab-a', ['terminal-a']), 'tab-b', ['terminal-b']);

		expect(ready(preparing, 'terminal-b', 'activation-ready')).toEqual({
			displayedTabId: 'tab-b',
			epoch: 1,
			kind: 'settled',
		});
	});

	it('ignores duplicate, stale, and wrong-target readiness reports', () => {
		const preparing = observe(observe(null, 'tab-a', ['terminal-a']), 'tab-b', ['terminal-b']);
		const displayReady = ready(preparing, 'terminal-b', 'display-ready');

		expect(ready(displayReady, 'terminal-b', 'display-ready')).toBe(displayReady);
		expect(ready(preparing, 'terminal-b', 'activation-ready', 0)).toBe(preparing);
		expect(ready(preparing, 'terminal-b', 'activation-ready', preparing.epoch, 'tab-other')).toBe(
			preparing,
		);
	});

	it('supersedes a preparing target while retaining the actually displayed Tab', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		const preparingB = observe(initial, 'tab-b', ['terminal-b']);
		const preparingC = observe(preparingB, 'tab-c', ['terminal-c']);

		expect(preparingC).toMatchObject({
			displayedTabId: 'tab-a',
			epoch: 2,
			expectedPresentationIds: ['terminal-c'],
			kind: 'preparing',
			targetTabId: 'tab-c',
		});
		expect(ready(preparingC, 'terminal-b', 'activation-ready', preparingB.epoch)).toBe(preparingC);
	});

	it('uses a visually committed inert target as the outgoing layer for another switch', () => {
		let state = observe(observe(null, 'tab-a', ['terminal-a']), 'tab-b', ['terminal-b']);
		state = ready(state, 'terminal-b', 'display-ready');
		const preparingC = observe(state, 'tab-c', ['terminal-c']);

		expect(preparingC).toMatchObject({ displayedTabId: 'tab-b', epoch: 2, kind: 'preparing' });
	});

	it('prepares a return to the frozen displayed Tab under a new epoch', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		const preparingB = observe(initial, 'tab-b', ['terminal-b']);
		const returningA = observe(preparingB, 'tab-a', ['terminal-a']);

		expect(returningA).toMatchObject({
			displayedTabId: 'tab-a',
			epoch: 2,
			kind: 'preparing',
			targetTabId: 'tab-a',
		});
	});

	it('settles targets without terminal surfaces and tolerates a missing outgoing Tab', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);

		expect(observe(initial, 'tab-b', [])).toEqual({
			displayedTabId: 'tab-b',
			epoch: 1,
			kind: 'settled',
		});
		expect(observe(initial, 'tab-b', ['terminal-b'], false)).toMatchObject({
			displayedTabId: null,
			kind: 'preparing',
		});
	});

	it('drops a removed outgoing identity without restarting the same target barrier', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		const preparing = observe(initial, 'tab-b', ['terminal-b']);
		const withoutOutgoing = observe(preparing, 'tab-b', ['terminal-b'], false);

		expect(withoutOutgoing).toMatchObject({
			displayedTabId: null,
			epoch: 1,
			kind: 'preparing',
			targetTabId: 'tab-b',
		});
		expect(ready(withoutOutgoing, 'terminal-b', 'activation-ready')).toEqual({
			displayedTabId: 'tab-b',
			epoch: 1,
			kind: 'settled',
		});
	});

	it('deduplicates the trusted expected identity set deterministically', () => {
		const initial = observe(null, 'tab-a', ['terminal-a']);
		const preparing = observe(initial, 'tab-b', ['terminal-b', 'terminal-b']);

		expect(preparing).toMatchObject({ expectedPresentationIds: ['terminal-b'] });
		expect(ready(preparing, 'terminal-b', 'activation-ready')).toMatchObject({ kind: 'settled' });
	});
});
