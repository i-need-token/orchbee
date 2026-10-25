import { describe, expect, it } from 'vitest';

import {
	tabCollectionEaseInOut,
	tabCollectionEaseOut,
} from '../tab-collection-animating-layout-manager';

describe('TabCollectionAnimatingLayoutManager', () => {
	it('uses Chromium BoundsAnimator ease-out progress', () => {
		expect(tabCollectionEaseOut(0)).toBe(0);
		expect(tabCollectionEaseOut(0.5)).toBe(0.75);
		expect(tabCollectionEaseOut(1)).toBe(1);
	});

	it('uses Chromium collection ease-in-out progress', () => {
		expect(tabCollectionEaseInOut(0)).toBe(0);
		expect(tabCollectionEaseInOut(0.25)).toBe(0.125);
		expect(tabCollectionEaseInOut(0.5)).toBe(0.5);
		expect(tabCollectionEaseInOut(0.75)).toBe(0.875);
		expect(tabCollectionEaseInOut(1)).toBe(1);
	});
});
