import { chmod, mkdir, open, rename, unlink } from 'node:fs/promises';
import path from 'node:path';

/**
 * Replaces a device record under `internal/state/`: flushes `encoded` into a temporary file beside
 * it and atomically renames that over the record, so a reader sees the old record or the new one.
 * On POSIX the directory and the record are owner-only.
 */
export async function writeStateRecordFile(
	filePath: string,
	encoded: Buffer,
	randomToken: () => string,
): Promise<void> {
	const directory = path.dirname(filePath);
	await mkdir(directory, { mode: 0o700, recursive: true });
	if (process.platform !== 'win32') await chmod(directory, 0o700);
	const temporaryPath = `${filePath}.${randomToken()}.tmp`;
	let temporaryCreated = false;
	try {
		const temporary = await open(temporaryPath, 'wx', 0o600);
		temporaryCreated = true;
		try {
			await temporary.writeFile(encoded);
			await temporary.sync();
		} finally {
			await temporary.close();
		}
		await rename(temporaryPath, filePath);
	} catch (error: unknown) {
		if (temporaryCreated) await unlink(temporaryPath).catch(() => undefined);
		throw error;
	}
	if (process.platform !== 'win32') {
		await chmod(filePath, 0o600);
		const directoryHandle = await open(directory, 'r');
		try {
			await directoryHandle.sync();
		} finally {
			await directoryHandle.close();
		}
	}
}
