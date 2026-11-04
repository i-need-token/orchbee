import type { ResizablePanelGroupHandle } from '@orchbee/ui/components';

import { isMissingPanelGroupError } from './isMissingPanelGroupError';

export function writePanelGroupLayout(
	group: ResizablePanelGroupHandle | null,
	layout: Readonly<Record<string, number>>,
): boolean {
	if (group === null) {
		return false;
	}

	try {
		group.setLayout({ ...layout });
		return true;
	} catch (error: unknown) {
		if (!isMissingPanelGroupError(error)) {
			throw error;
		}
		// A semantic layout update will resynchronize after the primitive finishes replacing its group.
		return false;
	}
}
