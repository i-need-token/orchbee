import { z } from 'zod';

import {
	DEFAULT_FILE_BAR_LAYOUT,
	FILE_BAR_VIEW_IDS,
	FILE_BAR_WIDTH,
	type FileBarLayout,
} from './file-bar-layout';

const FILE_BAR_LAYOUT_STORAGE_KEY = 'orchbee:file-bar:layout:v1';
const StoredFileBarLayoutSchema = z.strictObject({
	activeView: z.enum(FILE_BAR_VIEW_IDS),
	// Each view at most once, and never every view: the menu keeps the last toggle.
	hiddenToggles: z
		.array(z.enum(FILE_BAR_VIEW_IDS))
		.refine(
			(views) => new Set(views).size === views.length && views.length < FILE_BAR_VIEW_IDS.length,
		),
	visible: z.boolean(),
	width: z.number().int().min(FILE_BAR_WIDTH.minimum).max(FILE_BAR_WIDTH.maximum),
});

export type FileBarLayoutStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** The layout this device last committed, or the defaults when there is none it can trust. */
export function readFileBarLayout(storage: FileBarLayoutStorage | undefined): FileBarLayout {
	if (storage === undefined) return DEFAULT_FILE_BAR_LAYOUT;

	try {
		const serializedValue = storage.getItem(FILE_BAR_LAYOUT_STORAGE_KEY);
		if (serializedValue === null) return DEFAULT_FILE_BAR_LAYOUT;
		const result = StoredFileBarLayoutSchema.safeParse(JSON.parse(serializedValue));
		return result.success ? result.data : DEFAULT_FILE_BAR_LAYOUT;
	} catch {
		return DEFAULT_FILE_BAR_LAYOUT;
	}
}

/** Keeps a committed layout; one the record cannot hold leaves the stored layout as it was. */
export function writeFileBarLayout(
	storage: FileBarLayoutStorage | undefined,
	layout: FileBarLayout,
): void {
	if (storage === undefined) return;
	const result = StoredFileBarLayoutSchema.safeParse(layout);
	if (!result.success) return;

	try {
		storage.setItem(FILE_BAR_LAYOUT_STORAGE_KEY, JSON.stringify(result.data));
	} catch {
		// Device-local layout persistence is best effort and must not block the interface.
	}
}
