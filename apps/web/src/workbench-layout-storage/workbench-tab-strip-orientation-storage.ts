import { TabStripOrientation } from '@orchbee/tab-strip';
import { z } from 'zod';

import type { WorkbenchLayoutStorage } from './workbench-layout-storage';

const WORKBENCH_TAB_STRIP_ORIENTATION_STORAGE_KEY = 'orchbee:workbench:tab-strip-orientation:v1';
const StoredWorkbenchTabStripOrientationSchema = z.strictObject({
	orientation: z.enum([TabStripOrientation.Horizontal, TabStripOrientation.Vertical]),
});

export function readWorkbenchTabStripOrientation(
	storage?: WorkbenchLayoutStorage,
): TabStripOrientation {
	if (storage === undefined) return TabStripOrientation.Horizontal;

	try {
		const serializedValue = storage.getItem(WORKBENCH_TAB_STRIP_ORIENTATION_STORAGE_KEY);
		if (serializedValue === null) return TabStripOrientation.Horizontal;
		const result = StoredWorkbenchTabStripOrientationSchema.safeParse(JSON.parse(serializedValue));
		return result.success ? result.data.orientation : TabStripOrientation.Horizontal;
	} catch {
		return TabStripOrientation.Horizontal;
	}
}

export function writeWorkbenchTabStripOrientation(
	storage: WorkbenchLayoutStorage | undefined,
	orientation: TabStripOrientation,
): void {
	if (storage === undefined) return;

	try {
		storage.setItem(WORKBENCH_TAB_STRIP_ORIENTATION_STORAGE_KEY, JSON.stringify({ orientation }));
	} catch {
		// Device-local layout persistence is best effort and must not block the interface.
	}
}
