// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAgentSource,
} from '../../../workbench';
import { useAgentSettled } from '../useAgentSettled';

/** A source whose one terminal reports `state` from the incarnation `binding`. */
function agents(): {
	readonly source: WorkbenchAgentSource;
	report(state: WorkbenchAgentObservation['state'], binding?: string): void;
} {
	let snapshot: WorkbenchAgentSnapshot = { complete: true, presentations: new Map() };
	const listeners = new Set<() => void>();
	return {
		report(state, binding = 'first') {
			snapshot = {
				complete: true,
				presentations: new Map([
					['presentation', { agent: 'claude', binding, sequence: 1n, state }],
				]),
			};
			for (const listener of listeners) listener();
		},
		source: {
			getSnapshot: () => snapshot,
			subscribe(listener) {
				listeners.add(listener);
				return () => listeners.delete(listener);
			},
		},
	};
}

afterEach(cleanup);

describe('useAgentSettled', () => {
	it('refreshes when an Agent stops working, and not for a baseline or a replacement terminal', () => {
		const { report, source } = agents();
		report('working');
		const onSettled = vi.fn<() => void>();
		renderHook(() => useAgentSettled(source, onSettled));
		expect(onSettled).not.toHaveBeenCalled();

		act(() => report('idle'));
		expect(onSettled).toHaveBeenCalledTimes(1);
		act(() => report('blocked'));
		act(() => report('working'));
		act(() => report('done'));
		expect(onSettled).toHaveBeenCalledTimes(2);

		act(() => report('working'));
		act(() => report('idle', 'second'));
		expect(onSettled).toHaveBeenCalledTimes(2);
	});
});
