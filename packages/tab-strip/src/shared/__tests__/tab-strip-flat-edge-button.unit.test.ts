import { describe, expect, it } from 'vitest';
import {
	getComboButtonFlatEdges,
	getFlatEdgeButtonCornerRadii,
	getFlatEdgeButtonPreferredSize,
} from '../tab-strip-flat-edge-button';

describe('tab-strip flat-edge calculations', () => {
	it('exports the calculations exercised by shared-primitives behavior tests', () => {
		expect(getComboButtonFlatEdges).toBeTypeOf('function');
		expect(getFlatEdgeButtonCornerRadii).toBeTypeOf('function');
		expect(getFlatEdgeButtonPreferredSize).toBeTypeOf('function');
	});
});
