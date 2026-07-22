import { describe, expect, it } from 'vitest';
import { TabStripObserverList } from '../TabStripObserverList';

describe('TabStripObserverList', () => {
	it('is the observer owner exercised by shared-primitives behavior tests', () => {
		expect(TabStripObserverList).toBeTypeOf('function');
	});
});
