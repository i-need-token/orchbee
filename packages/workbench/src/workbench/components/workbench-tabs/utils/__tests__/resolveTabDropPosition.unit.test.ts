import { describe, expect, it } from 'vitest';

import { resolveTabDropPosition } from '../resolveTabDropPosition';

const TAB_IDS = ['first', 'second', 'third', 'fourth'] as const;

describe('resolveTabDropPosition', () => {
	it('distinguishes both halves of a target when moving toward the end', () => {
		expect(resolveTabDropPosition(TAB_IDS, 'first', 'third', 'before')).toEqual({
			boundaryIndex: 2,
			targetIndex: 1,
		});
		expect(resolveTabDropPosition(TAB_IDS, 'first', 'third', 'after')).toEqual({
			boundaryIndex: 3,
			targetIndex: 2,
		});
	});

	it('distinguishes both halves of a target when moving toward the start', () => {
		expect(resolveTabDropPosition(TAB_IDS, 'fourth', 'second', 'before')).toEqual({
			boundaryIndex: 1,
			targetIndex: 1,
		});
		expect(resolveTabDropPosition(TAB_IDS, 'fourth', 'second', 'after')).toEqual({
			boundaryIndex: 2,
			targetIndex: 2,
		});
	});

	it('supports the two outer insertion boundaries', () => {
		expect(resolveTabDropPosition(TAB_IDS, 'fourth', 'first', 'before')).toEqual({
			boundaryIndex: 0,
			targetIndex: 0,
		});
		expect(resolveTabDropPosition(TAB_IDS, 'first', 'fourth', 'after')).toEqual({
			boundaryIndex: TAB_IDS.length,
			targetIndex: TAB_IDS.length - 1,
		});
	});

	it('does not report a position that preserves the current order', () => {
		expect(resolveTabDropPosition(TAB_IDS, 'second', 'first', 'after')).toBeUndefined();
		expect(resolveTabDropPosition(TAB_IDS, 'second', 'second', 'before')).toBeUndefined();
		expect(resolveTabDropPosition(TAB_IDS, 'second', 'second', 'after')).toBeUndefined();
		expect(resolveTabDropPosition(TAB_IDS, 'second', 'third', 'before')).toBeUndefined();
	});

	it('rejects unknown tab identifiers', () => {
		expect(resolveTabDropPosition(TAB_IDS, 'missing', 'third', 'before')).toBeUndefined();
		expect(resolveTabDropPosition(TAB_IDS, 'first', 'missing', 'after')).toBeUndefined();
	});
});
