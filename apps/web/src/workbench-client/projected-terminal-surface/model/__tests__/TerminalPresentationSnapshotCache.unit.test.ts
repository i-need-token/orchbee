import { describe, expect, it } from 'vitest';

import { TerminalPresentationSnapshotCache } from '../TerminalPresentationSnapshotCache';

interface Snapshot {
	readonly byteLength: number;
	readonly label: string;
}

describe('TerminalPresentationSnapshotCache', () => {
	it('evicts the least recently used snapshots within its exact byte budget', () => {
		const cache = new TerminalPresentationSnapshotCache<Snapshot>(10);
		const first = { byteLength: 4, label: 'first' };
		const second = { byteLength: 4, label: 'second' };
		const third = { byteLength: 4, label: 'third' };

		expect(cache.set('first', first)).toBe(true);
		expect(cache.set('second', second)).toBe(true);
		expect(cache.get('first')).toBe(first);
		expect(cache.set('third', third)).toBe(true);

		expect(cache.get('second')).toBeUndefined();
		expect(cache.get('first')).toBe(first);
		expect(cache.get('third')).toBe(third);
		expect(cache.byteLength).toBe(8);
		expect(cache.size).toBe(2);
	});

	it('replaces one identity without double-counting and rejects oversized values', () => {
		const cache = new TerminalPresentationSnapshotCache<Snapshot>(5);

		expect(cache.set('terminal', { byteLength: 3, label: 'old' })).toBe(true);
		expect(cache.set('terminal', { byteLength: 5, label: 'new' })).toBe(true);
		expect(cache.byteLength).toBe(5);
		expect(cache.get('terminal')?.label).toBe('new');
		expect(cache.set('terminal', { byteLength: 6, label: 'oversized' })).toBe(false);
		expect(cache.get('terminal')).toBeUndefined();
		expect(cache.byteLength).toBe(0);
	});

	it('clears all retained bytes and rejects invalid budgets and sizes', () => {
		expect(() => new TerminalPresentationSnapshotCache<Snapshot>(-1)).toThrow(RangeError);
		const cache = new TerminalPresentationSnapshotCache<Snapshot>(5);

		expect(cache.set('invalid', { byteLength: Number.NaN, label: 'invalid' })).toBe(false);
		expect(cache.set('valid', { byteLength: 2, label: 'valid' })).toBe(true);
		cache.clear();

		expect(cache.byteLength).toBe(0);
		expect(cache.size).toBe(0);
	});
});
