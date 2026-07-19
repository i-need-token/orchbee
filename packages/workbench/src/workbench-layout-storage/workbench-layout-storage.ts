const DEFAULT_LEFT_PANEL_WIDTH = 250;
export const MINIMUM_LEFT_PANEL_WIDTH = 200;
export const MAXIMUM_LEFT_PANEL_WIDTH = 400;

const WORKBENCH_LEFT_PANEL_WIDTH_STORAGE_KEY = 'orchbee:workbench:left-panel-width:v1';

export type WorkbenchLayoutStorage = Pick<Storage, 'getItem' | 'setItem'>;

function isStoredLeftPanelWidth(value: unknown): value is { readonly width: number } {
	if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
	if (Object.keys(value).length !== 1 || !('width' in value)) return false;
	return (
		typeof value.width === 'number' &&
		Number.isInteger(value.width) &&
		value.width >= MINIMUM_LEFT_PANEL_WIDTH &&
		value.width <= MAXIMUM_LEFT_PANEL_WIDTH
	);
}

export function readWorkbenchLeftPanelWidth(storage?: WorkbenchLayoutStorage): number {
	if (storage === undefined) return DEFAULT_LEFT_PANEL_WIDTH;

	try {
		const serializedValue = storage.getItem(WORKBENCH_LEFT_PANEL_WIDTH_STORAGE_KEY);
		if (serializedValue === null) return DEFAULT_LEFT_PANEL_WIDTH;
		const value: unknown = JSON.parse(serializedValue);
		return isStoredLeftPanelWidth(value) ? value.width : DEFAULT_LEFT_PANEL_WIDTH;
	} catch {
		return DEFAULT_LEFT_PANEL_WIDTH;
	}
}

export function writeWorkbenchLeftPanelWidth(
	storage: WorkbenchLayoutStorage | undefined,
	width: number,
): void {
	if (storage === undefined) return;
	const roundedWidth = Math.round(width);
	if (roundedWidth < MINIMUM_LEFT_PANEL_WIDTH || roundedWidth > MAXIMUM_LEFT_PANEL_WIDTH) {
		return;
	}

	try {
		storage.setItem(
			WORKBENCH_LEFT_PANEL_WIDTH_STORAGE_KEY,
			JSON.stringify({ width: roundedWidth }),
		);
	} catch {
		// Device-local layout persistence is best effort and must not block the interface.
	}
}
