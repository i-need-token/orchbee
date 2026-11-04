import { z } from 'zod';

import type { WorkbenchLayoutStorage } from './workbench-layout-storage';

const DEFAULT_WORKBENCH_SIDEBAR_VISIBILITY = true;
const WORKBENCH_SIDEBAR_VISIBILITY_STORAGE_KEY = 'orchbee:workbench:left-panel-visibility:v1';
const StoredWorkbenchSidebarVisibilitySchema = z.strictObject({
	visible: z.boolean(),
});

export function readWorkbenchSidebarVisibility(storage?: WorkbenchLayoutStorage): boolean {
	if (storage === undefined) return DEFAULT_WORKBENCH_SIDEBAR_VISIBILITY;

	try {
		const serializedValue = storage.getItem(WORKBENCH_SIDEBAR_VISIBILITY_STORAGE_KEY);
		if (serializedValue === null) return DEFAULT_WORKBENCH_SIDEBAR_VISIBILITY;
		const result = StoredWorkbenchSidebarVisibilitySchema.safeParse(JSON.parse(serializedValue));
		return result.success ? result.data.visible : DEFAULT_WORKBENCH_SIDEBAR_VISIBILITY;
	} catch {
		return DEFAULT_WORKBENCH_SIDEBAR_VISIBILITY;
	}
}

export function writeWorkbenchSidebarVisibility(
	storage: WorkbenchLayoutStorage | undefined,
	visible: boolean,
): void {
	if (storage === undefined) return;

	try {
		storage.setItem(WORKBENCH_SIDEBAR_VISIBILITY_STORAGE_KEY, JSON.stringify({ visible }));
	} catch {
		// Device-local layout persistence is best effort and must not block the interface.
	}
}
