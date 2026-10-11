import { describe, expect, it, vi } from 'vitest';

import { NodeHostBuildWatch } from '../NodeHostBuildWatch';

describe('NodeHostBuildWatch', () => {
	it.each([
		{ failure: new Error('watcher close failed'), kind: 'Error' },
		{ failure: undefined, kind: 'undefined' },
		{ failure: null, kind: 'null' },
	])('retains a $kind failure after terminating and draining the build', async ({ failure }) => {
		const effects: string[] = [];
		const watch = new NodeHostBuildWatch(
			{
				terminate: async () => {
					effects.push('terminate');
				},
			},
			{
				close: () => {
					effects.push('close watcher');
					throw failure;
				},
			},
			() => {
				effects.push('clear debounce');
			},
			async () => {
				effects.push('drain');
			},
		);

		await expect(watch.close()).rejects.toBe(failure);
		expect(effects).toEqual(['clear debounce', 'close watcher', 'terminate', 'drain']);
	});

	it('preserves the first null failure when process termination also fails', async () => {
		const drain = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
		const watch = new NodeHostBuildWatch(
			{
				terminate: async () => {
					throw new Error('later process termination failure');
				},
			},
			{
				close: () => {
					throw null;
				},
			},
			() => undefined,
			drain,
		);

		await expect(watch.close()).rejects.toBeNull();
		expect(drain).toHaveBeenCalledOnce();
	});
});
