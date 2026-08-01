import type { WorkbenchConfirmationAnchor } from '../types/workbench-confirmation';

const CONFIRMATION_ANCHOR_CAPABILITY = 'workbench.confirmation-anchor';

export function readWorkbenchConfirmationAnchor(
	capabilities: Readonly<Record<string, unknown>> | undefined,
): WorkbenchConfirmationAnchor | null {
	const value = capabilities?.[CONFIRMATION_ANCHOR_CAPABILITY];
	if (
		typeof value !== 'object' ||
		value === null ||
		!('x' in value) ||
		!('y' in value) ||
		typeof value.x !== 'number' ||
		typeof value.y !== 'number'
	) {
		return null;
	}
	return { x: value.x, y: value.y };
}

export function workbenchConfirmationCapabilities(
	anchor: WorkbenchConfirmationAnchor,
): Readonly<Record<string, unknown>> {
	return { [CONFIRMATION_ANCHOR_CAPABILITY]: anchor };
}
