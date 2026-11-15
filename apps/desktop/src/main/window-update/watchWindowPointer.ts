import { watch } from 'node:fs';

const POINTER_FILE_NAME = 'active.json';

/**
 * Calls `onChange` whenever `desktop\active.json` may have changed. The pointer is replaced by a
 * rename, which ends a watch on the file itself, so this watches its folder. A folder that cannot be
 * watched, or a watch that fails later, reports nothing more; the offer then keeps its last value.
 * Returns what ends the watch.
 */
export function watchWindowPointer(desktopRoot: string, onChange: () => void): () => void {
	try {
		const watcher = watch(desktopRoot, { persistent: false }, (_event, fileName) => {
			if (fileName === null || fileName.toLowerCase() === POINTER_FILE_NAME) onChange();
		});
		watcher.on('error', () => watcher.close());
		return () => watcher.close();
	} catch {
		return () => undefined;
	}
}
