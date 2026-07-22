import { describe, expect, it } from 'vitest';
import { DropArrowState } from '../drop-arrow';

describe('DropArrowState', () => {
	it('is the state owner exercised by shared-primitives behavior tests', () => {
		expect(DropArrowState).toBeTypeOf('function');
	});
});
