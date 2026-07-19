import type { WorkbenchLayoutStorage } from './workbench-layout-storage';

const DEFAULT_WORKBENCH_SIDEBAR_VISIBILITY = true;
const WORKBENCH_SIDEBAR_VISIBILITY_STORAGE_KEY = 'orchbee:workbench:left-panel-visibility:v1';

function isStoredWorkbenchSidebarVisibility(
	value: unknown,
): value is { readonly visible: boolean } {
	return (
		typeof value === 'object' &&
		value !== null &&
		!Array.isArray(value) &&
		Object.keys(value).length === 1 &&
		'visible' in value &&
		typeof value.visible === 'boolean'
	);
}

export function readWorkbenchSidebarVisibility(storage?: WorkbenchLayoutStorage): boolean {
	if (storage === undefined) return DEFAULT_WORKBENCH_SIDEBAR_VISIBILITY;

	try {
		const serializedValue = storage.getItem(WORKBENCH_SIDEBAR_VISIBILITY_STORAGE_KEY);
		if (serializedValue === null) return DEFAULT_WORKBENCH_SIDEBAR_VISIBILITY;
		const value: unknown = JSON.parse(serializedValue);
		return isStoredWorkbenchSidebarVisibility(value)
			? value.visible
			: DEFAULT_WORKBENCH_SIDEBAR_VISIBILITY;
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
