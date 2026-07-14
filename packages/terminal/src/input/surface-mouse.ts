export interface SurfaceMouseModifiers {
	readonly alt: boolean;
	readonly control: boolean;
	readonly meta: boolean;
	readonly shift: boolean;
}

export type SurfaceMousePlatform = 'macos' | 'other';
export type SurfaceMouseCursor = 'crosshair' | 'default' | 'text';

export interface SurfaceMouseCursorInput {
	readonly hidden: boolean;
	readonly modifiers: SurfaceMouseModifiers;
	readonly mouseTracking: boolean;
	readonly overLink: boolean;
	readonly platform: SurfaceMousePlatform;
}

const SURFACE_MOUSE_MODIFIER_CODES = new Set([
	'AltLeft',
	'AltRight',
	'ControlLeft',
	'ControlRight',
	'MetaLeft',
	'MetaRight',
	'ShiftLeft',
	'ShiftRight',
]);

export function isSurfaceMouseModifierKey(code: string): boolean {
	return SURFACE_MOUSE_MODIFIER_CODES.has(code);
}

export function classifySurfaceMousePlatform(platform: string): SurfaceMousePlatform {
	return /Mac|iPhone|iPad/u.test(platform) ? 'macos' : 'other';
}

export function resolveSurfaceMouseCursor(
	input: SurfaceMouseCursorInput,
): SurfaceMouseCursor | null {
	if (input.overLink || input.hidden) return null;
	const rectangle =
		input.platform === 'macos'
			? input.modifiers.alt
			: input.modifiers.alt && (input.modifiers.control || input.modifiers.meta);
	if (input.mouseTracking) {
		if (input.modifiers.shift && rectangle) return 'crosshair';
		return input.modifiers.shift ? 'text' : 'default';
	}
	return rectangle ? 'crosshair' : 'text';
}
