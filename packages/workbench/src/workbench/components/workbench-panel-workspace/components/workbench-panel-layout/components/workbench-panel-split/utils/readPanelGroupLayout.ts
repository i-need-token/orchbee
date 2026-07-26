import type { ResizablePanelGroupHandle } from '@orchbee/ui/components';

import { isMissingPanelGroupError } from './isMissingPanelGroupError';

export function readPanelGroupLayout(
	group: ResizablePanelGroupHandle | null,
): Readonly<Record<string, number>> | null {
	if (group === null) {
		return null;
	}

	try {
		return group.getLayout();
	} catch (error: unknown) {
		if (!isMissingPanelGroupError(error)) {
			throw error;
		}
		// The primitive can retain a handle briefly after its internal group registry is replaced.
		return null;
	}
}
