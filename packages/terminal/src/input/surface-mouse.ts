export interface SurfaceMouseModifiers {
	readonly alt: boolean;
	readonly control: boolean;
	readonly meta: boolean;
	readonly shift: boolean;
}

export type SurfaceMousePlatform = 'macos' | 'other';
export type SurfaceMouseCursor = 'crosshair' | 'default' | 'pointer' | 'text';

export interface SurfaceMouseCursorInput {
	readonly applicationMouseOwned: boolean;
	readonly linkActive: boolean;
	readonly modifiers: SurfaceMouseModifiers;
	readonly platform: SurfaceMousePlatform;
	readonly selectionOwned: boolean;
}

export function classifySurfaceMousePlatform(platform: string): SurfaceMousePlatform {
	return /Mac|iPhone|iPad/u.test(platform) ? 'macos' : 'other';
}

export function resolveSurfaceMouseCursor(input: SurfaceMouseCursorInput): SurfaceMouseCursor {
	if (input.linkActive) return 'pointer';
	if (input.applicationMouseOwned) return 'default';
	const rectangle =
		input.platform === 'macos'
			? input.modifiers.alt
			: input.modifiers.alt && (input.modifiers.control || input.modifiers.meta);
	if (input.selectionOwned && rectangle) return 'crosshair';
	return input.selectionOwned ? 'text' : 'default';
}
