import { z } from 'zod';

import { FILE_OVERLAY_PIP_CORNERS, type FileOverlayPipPosition } from './file-overlay-pip';

const FILE_OVERLAY_PIP_POSITION_STORAGE_KEY = 'orchbee:file-overlay:pip-position:v1';
/** Farther from a corner than any window reaches, so no position this device kept. */
const MAXIMUM_PIP_OFFSET = 100_000;
const StoredFileOverlayPipPositionSchema = z.strictObject({
	corner: z.enum(FILE_OVERLAY_PIP_CORNERS),
	x: z.number().int().min(0).max(MAXIMUM_PIP_OFFSET),
	y: z.number().int().min(0).max(MAXIMUM_PIP_OFFSET),
});

export type FileOverlayPipStorage = Pick<Storage, 'getItem' | 'setItem'>;

/**
 * Where a drag last left picture-in-picture on this device, the same for every Workspace; null,
 * when there is no position it can trust, places it at the terminal area's bottom-right corner.
 */
export function readFileOverlayPipPosition(
	storage: FileOverlayPipStorage | undefined,
): FileOverlayPipPosition | null {
	if (storage === undefined) return null;

	try {
		const serializedValue = storage.getItem(FILE_OVERLAY_PIP_POSITION_STORAGE_KEY);
		if (serializedValue === null) return null;
		const result = StoredFileOverlayPipPositionSchema.safeParse(JSON.parse(serializedValue));
		return result.success ? result.data : null;
	} catch {
		return null;
	}
}

/** Keeps where a drag left picture-in-picture; one the record cannot hold keeps the stored one. */
export function writeFileOverlayPipPosition(
	storage: FileOverlayPipStorage | undefined,
	position: FileOverlayPipPosition,
): void {
	if (storage === undefined) return;
	const result = StoredFileOverlayPipPositionSchema.safeParse(position);
	if (!result.success) return;

	try {
		storage.setItem(FILE_OVERLAY_PIP_POSITION_STORAGE_KEY, JSON.stringify(result.data));
	} catch {
		// Device-local layout persistence is best effort and must not block the interface.
	}
}
