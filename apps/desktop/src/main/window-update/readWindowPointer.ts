import { open } from 'node:fs/promises';
import path from 'node:path';

import { parseWindowPointer } from './window-update-offer';

/** Readers take no lock and read at most this much, as the command launcher does. */
const MAXIMUM_POINTER_BYTES = 16 * 1024;
const POINTER_FILE_NAME = 'active.json';

/**
 * The active window version `desktop\active.json` names, or null when it is missing, larger than
 * 16 KiB, not UTF-8, or not a format-1 pointer.
 */
export async function readWindowPointer(desktopRoot: string): Promise<string | null> {
	try {
		const handle = await open(path.join(desktopRoot, POINTER_FILE_NAME), 'r');
		try {
			const buffer = Buffer.alloc(MAXIMUM_POINTER_BYTES + 1);
			const { bytesRead } = await handle.read(buffer, 0, buffer.length, 0);
			if (bytesRead > MAXIMUM_POINTER_BYTES) return null;
			return parseWindowPointer(
				new TextDecoder('utf-8', { fatal: true }).decode(buffer.subarray(0, bytesRead)),
			);
		} finally {
			await handle.close();
		}
	} catch {
		return null;
	}
}
