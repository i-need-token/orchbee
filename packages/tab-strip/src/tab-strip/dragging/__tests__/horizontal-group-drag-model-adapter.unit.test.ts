import { describe, expect, it } from 'vitest';
import {
	applyHorizontalGroupDragCommands,
	assertHorizontalGroupDragRefinement,
	createArmedHorizontalGroupDrag,
} from '../horizontal-group-drag-model-adapter';

describe('horizontal group drag model adapter', () => {
	it('exports the adapter operations exercised by drag behavior tests', () => {
		expect(applyHorizontalGroupDragCommands).toBeTypeOf('function');
		expect(assertHorizontalGroupDragRefinement).toBeTypeOf('function');
		expect(createArmedHorizontalGroupDrag).toBeTypeOf('function');
	});
});
