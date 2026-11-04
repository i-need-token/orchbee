import { describe, expect, it } from 'vitest';

import { resolvePanelResizeKey } from '../resolvePanelResizeKey';

const key = (code: string, overrides: Partial<Parameters<typeof resolvePanelResizeKey>[0]> = {}) =>
	resolvePanelResizeKey({ altKey: false, code, ctrlKey: false, metaKey: false, ...overrides });

describe('resolvePanelResizeKey', () => {
	it.each([
		['KeyW', 'wider'],
		['KeyN', 'narrower'],
		['KeyT', 'taller'],
		['KeyS', 'shorter'],
		['KeyR', 'reset'],
	] as const)('maps %s to %s', (code, adjustment) => {
		expect(key(code)).toEqual({ adjustment, multiplier: 1 });
	});

	it('uses the larger step only for directional Control chords', () => {
		expect(key('KeyW', { ctrlKey: true })).toEqual({ adjustment: 'wider', multiplier: 2 });
		expect(key('KeyR', { ctrlKey: true })).toEqual({ adjustment: 'reset', multiplier: 1 });
	});

	it('leaves unrelated and operating-system chords unhandled', () => {
		expect(key('Escape')).toBeNull();
		expect(key('KeyW', { altKey: true })).toBeNull();
		expect(key('KeyW', { metaKey: true })).toBeNull();
	});
});
