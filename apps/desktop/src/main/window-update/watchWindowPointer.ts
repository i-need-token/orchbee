import { watch } from 'node:fs';

const POINTER_FILE_NAME = 'active.json';

/**
 * Calls `onChange` whenever `desktop\active.json` may have changed. The pointer is replaced by a
 * rename, which ends a watch on the file itself, so this watches its folder. Returns what ends the
 * watch, or null when the folder cannot be watched now, as before a macOS window's first start has
 * made the window store; a watch that fails later reports nothing more, and the offer then keeps
 * its last value.
 */
export function watchWindowPointer(desktopRoot: string, onChange: () => void): (() => void) | null {
	try {
		const watcher = watch(desktopRoot, { persistent: false }, (_event, fileName) => {
			if (fileName === null || fileName.toLowerCase() === POINTER_FILE_NAME) onChange();
		});
		watcher.on('error', () => watcher.close());
		return () => watcher.close();
	} catch {
		return null;
	}
}
