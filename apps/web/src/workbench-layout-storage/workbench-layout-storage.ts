import { z } from 'zod';

const DEFAULT_LEFT_PANEL_WIDTH = 250;
export const MINIMUM_LEFT_PANEL_WIDTH = 200;
export const MAXIMUM_LEFT_PANEL_WIDTH = 400;

const WORKBENCH_LEFT_PANEL_WIDTH_STORAGE_KEY = 'orchbee:workbench:left-panel-width:v1';
const StoredLeftPanelWidthSchema = z.strictObject({
	width: z.number().int().min(MINIMUM_LEFT_PANEL_WIDTH).max(MAXIMUM_LEFT_PANEL_WIDTH),
});

export type WorkbenchLayoutStorage = Pick<Storage, 'getItem' | 'setItem'>;

export function readWorkbenchLeftPanelWidth(storage?: WorkbenchLayoutStorage): number {
	if (storage === undefined) return DEFAULT_LEFT_PANEL_WIDTH;

	try {
		const serializedValue = storage.getItem(WORKBENCH_LEFT_PANEL_WIDTH_STORAGE_KEY);
		if (serializedValue === null) return DEFAULT_LEFT_PANEL_WIDTH;
		const result = StoredLeftPanelWidthSchema.safeParse(JSON.parse(serializedValue));
		return result.success ? result.data.width : DEFAULT_LEFT_PANEL_WIDTH;
	} catch {
		return DEFAULT_LEFT_PANEL_WIDTH;
	}
}

export function writeWorkbenchLeftPanelWidth(
	storage: WorkbenchLayoutStorage | undefined,
	width: number,
): void {
	if (storage === undefined) return;
	const result = StoredLeftPanelWidthSchema.safeParse({ width: Math.round(width) });
	if (!result.success) return;

	try {
		storage.setItem(WORKBENCH_LEFT_PANEL_WIDTH_STORAGE_KEY, JSON.stringify(result.data));
	} catch {
		// Device-local layout persistence is best effort and must not block the interface.
	}
}
