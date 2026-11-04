import type { PanelResizeAdjustment, PanelResizeMultiplier } from './resolvePanelResize';

interface PanelResizeKeyInput {
	readonly altKey: boolean;
	readonly code: string;
	readonly ctrlKey: boolean;
	readonly metaKey: boolean;
}

interface PanelResizeKeyResolution {
	readonly adjustment: PanelResizeAdjustment;
	readonly multiplier: PanelResizeMultiplier;
}

export function resolvePanelResizeKey(input: PanelResizeKeyInput): PanelResizeKeyResolution | null {
	if (input.altKey || input.metaKey) return null;
	const adjustment = adjustmentForCode(input.code);
	if (adjustment === undefined) return null;
	return {
		adjustment,
		multiplier: adjustment === 'reset' || !input.ctrlKey ? 1 : 2,
	};
}

function adjustmentForCode(code: string): PanelResizeAdjustment | undefined {
	switch (code) {
		case 'KeyN':
			return 'narrower';
		case 'KeyR':
			return 'reset';
		case 'KeyS':
			return 'shorter';
		case 'KeyT':
			return 'taller';
		case 'KeyW':
			return 'wider';
		default:
			return undefined;
	}
}
