import { describe, expect, it } from 'vitest';
import { rectRight } from '../rectRight';

describe('rectRight', () => {
	it('adds a rectangle origin and width', () => {
		expect(rectRight({ x: 3, y: 0, width: 7, height: 1 })).toBe(10);
	});
});
