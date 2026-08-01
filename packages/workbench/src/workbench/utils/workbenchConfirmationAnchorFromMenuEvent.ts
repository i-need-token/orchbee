import type * as React from 'react';

import type { WorkbenchConfirmationAnchor } from '../types/workbench-confirmation';

export function workbenchConfirmationAnchorFromMenuEvent(
	event: React.MouseEvent<HTMLElement>,
): WorkbenchConfirmationAnchor {
	if (event.detail > 0) {
		return { x: event.clientX, y: event.clientY };
	}
	const bounds = event.currentTarget.getBoundingClientRect();
	return { x: bounds.right, y: bounds.top + bounds.height / 2 };
}
