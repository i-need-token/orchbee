import { describe, expect, it } from 'vitest';

import {
	classifySurfaceMousePlatform,
	resolveSurfaceMouseCursor,
	type SurfaceMouseCursorInput,
	type SurfaceMouseModifiers,
} from '../surface-mouse';

const noModifiers: SurfaceMouseModifiers = {
	alt: false,
	control: false,
	meta: false,
	shift: false,
};

describe('surface mouse cursor', () => {
	it('keeps fixed application and selection ownership ahead of modifier changes', () => {
		expect(
			resolveSurfaceMouseCursor(
				input({
					applicationMouseOwned: true,
					modifiers: { alt: true, control: true, meta: false, shift: true },
					selectionOwned: false,
				}),
			),
		).toBe('default');
		expect(
			resolveSurfaceMouseCursor(
				input({
					modifiers: { alt: false, control: false, meta: false, shift: false },
					selectionOwned: true,
				}),
			),
		).toBe('text');
	});

	it('uses the platform rectangle modifier policy for terminal selection', () => {
		expect(
			resolveSurfaceMouseCursor(input({ modifiers: { ...noModifiers, alt: true, control: true } })),
		).toBe('crosshair');
		expect(resolveSurfaceMouseCursor(input({ modifiers: { ...noModifiers, alt: true } }))).toBe(
			'text',
		);
		expect(
			resolveSurfaceMouseCursor(
				input({ modifiers: { ...noModifiers, alt: true }, platform: 'macos' }),
			),
		).toBe('crosshair');
	});

	it('gives an enabled link the pointer and classifies Apple browser platforms', () => {
		expect(resolveSurfaceMouseCursor(input({ linkActive: true }))).toBe('pointer');
		expect(classifySurfaceMousePlatform('MacIntel')).toBe('macos');
		expect(classifySurfaceMousePlatform('iPhone')).toBe('macos');
		expect(classifySurfaceMousePlatform('Linux x86_64')).toBe('other');
	});
});

function input(overrides: Partial<SurfaceMouseCursorInput>): SurfaceMouseCursorInput {
	return {
		applicationMouseOwned: false,
		linkActive: false,
		modifiers: noModifiers,
		platform: 'other',
		selectionOwned: true,
		...overrides,
	};
}
