import { describe, expect, it } from 'vitest';
import {
	areDraggedTabIdsConsecutive,
	joinClassNames,
	nextAvailableEntityId,
} from '../tab-strip-internals';

describe('tab-strip internals', () => {
	it('preserves deterministic identifiers and dragged-tab adjacency', () => {
		expect(nextAvailableEntityId('group', new Set(['group-1', 'group-2']))).toBe('group-3');
		expect(areDraggedTabIdsConsecutive([{ id: 'a' }, { id: 'b' }, { id: 'c' }], ['a', 'b'])).toBe(
			true,
		);
		expect(areDraggedTabIdsConsecutive([{ id: 'a' }, { id: 'b' }, { id: 'c' }], ['a', 'c'])).toBe(
			false,
		);
	});

	it('joins defined CSS Module class names in source order', () => {
		expect(joinClassNames('root', undefined, 'active')).toBe('root active');
	});
});
