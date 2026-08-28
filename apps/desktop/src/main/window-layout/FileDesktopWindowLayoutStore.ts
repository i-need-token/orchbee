import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, readFile, rename, stat, unlink } from 'node:fs/promises';
import path from 'node:path';

import {
	parseDesktopWindowLayout,
	type DesktopWindowLayout,
	type DesktopWindowLayoutLoadResult,
	type DesktopWindowLayoutStore,
} from './desktop-window-layout';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export interface FileDesktopWindowLayoutStoreOptions {
	readonly randomToken?: () => string;
}

export class FileDesktopWindowLayoutStore implements DesktopWindowLayoutStore {
	readonly #filePath: string;
	#operationTail = Promise.resolve();
	readonly #randomToken: () => string;

	constructor(filePath: string, options: FileDesktopWindowLayoutStoreOptions = {}) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
	}

	async flush(): Promise<void> {
		await this.#operationTail;
	}

	async load(): Promise<DesktopWindowLayoutLoadResult> {
		await this.#operationTail;
		return readLayoutFile(this.#filePath);
	}

	save(layout: DesktopWindowLayout): Promise<boolean> {
		const canonical = parseDesktopWindowLayout(layout);
		if (canonical === null) return Promise.resolve(false);
		const result = this.#operationTail.then(async () => {
			try {
				await this.#writeLayout(canonical);
				return true;
			} catch {
				return false;
			}
		});
		this.#operationTail = result.then(() => undefined);
		return result;
	}

	async #writeLayout(layout: DesktopWindowLayout): Promise<void> {
		const encoded = Buffer.from(`${JSON.stringify(layout)}\n`, 'utf8');
		const directory = path.dirname(this.#filePath);
		await mkdir(directory, { mode: 0o700, recursive: true });
		if (process.platform !== 'win32') await chmod(directory, 0o700);
		const temporaryPath = `${this.#filePath}.${this.#randomToken()}.tmp`;
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
			await rename(temporaryPath, this.#filePath);
		} catch (error: unknown) {
			if (temporaryCreated) await unlink(temporaryPath).catch(() => undefined);
			throw error;
		}
		if (process.platform !== 'win32') {
			await chmod(this.#filePath, 0o600);
			const directoryHandle = await open(directory, 'r');
			try {
				await directoryHandle.sync();
			} finally {
				await directoryHandle.close();
			}
		}
	}
}

async function readLayoutFile(filePath: string): Promise<DesktopWindowLayoutLoadResult> {
	try {
		const metadata = await stat(filePath);
		if (!metadata.isFile()) return { kind: 'unavailable', reason: 'invalid' };
		const encoded = await readFile(filePath);
		let value: unknown;
		try {
			value = JSON.parse(utf8Decoder.decode(encoded));
		} catch {
			return { kind: 'unavailable', reason: 'invalid' };
		}
		const layout = parseDesktopWindowLayout(value);
		return layout === null
			? { kind: 'unavailable', reason: 'invalid' }
			: { kind: 'available', layout };
	} catch (error: unknown) {
		if (isNodeError(error) && error.code === 'ENOENT') return { kind: 'missing' };
		return { kind: 'unavailable', reason: 'read-failed' };
	}
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
