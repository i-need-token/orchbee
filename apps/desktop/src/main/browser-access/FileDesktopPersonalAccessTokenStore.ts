import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, readFile, rename, stat, unlink } from 'node:fs/promises';
import path from 'node:path';

import {
	PersonalAccessTokenVerifierSchema,
	type PersonalAccessTokenVerifier,
} from '@orchbee/remote-client-protocol';
import { z } from 'zod';

import type { DesktopPersonalAccessTokenStore } from './DesktopPersonalAccessTokenController';

const MAXIMUM_STORE_BYTES = 4 * 1024;
const PersonalAccessTokenStoreSchema = z.strictObject({
	verifier: PersonalAccessTokenVerifierSchema,
	version: z.literal(1),
});

interface FileDesktopPersonalAccessTokenStoreOptions {
	readonly openTemporaryFile?:
		| ((temporaryPath: string) => Promise<PersonalAccessTokenTemporaryFile>)
		| undefined;
	readonly randomToken?: (() => string) | undefined;
	readonly replaceFile?: ((temporaryPath: string, filePath: string) => Promise<void>) | undefined;
}

interface PersonalAccessTokenTemporaryFile {
	chmod(mode: number): Promise<void>;
	close(): Promise<void>;
	sync(): Promise<void>;
	writeFile(data: string, encoding: 'utf8'): Promise<void>;
}

export class FileDesktopPersonalAccessTokenStore implements DesktopPersonalAccessTokenStore {
	readonly #filePath: string;
	readonly #openTemporaryFile: (temporaryPath: string) => Promise<PersonalAccessTokenTemporaryFile>;
	readonly #randomToken: () => string;
	readonly #replaceFile: (temporaryPath: string, filePath: string) => Promise<void>;

	constructor(filePath: string, options: FileDesktopPersonalAccessTokenStoreOptions = {}) {
		this.#filePath = filePath;
		this.#openTemporaryFile =
			options.openTemporaryFile ?? (async (temporaryPath) => open(temporaryPath, 'wx', 0o600));
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
		this.#replaceFile = options.replaceFile ?? rename;
	}

	async load(): Promise<PersonalAccessTokenVerifier | null> {
		let metadata;
		try {
			metadata = await stat(this.#filePath);
		} catch (error: unknown) {
			if (isMissingFileError(error)) return null;
			throw error;
		}
		if (!metadata.isFile() || metadata.size > MAXIMUM_STORE_BYTES) {
			throw new Error('The Personal Access Token verifier store is invalid.');
		}
		const encoded = await readFile(this.#filePath, 'utf8');
		let value: unknown;
		try {
			value = JSON.parse(encoded);
		} catch (error: unknown) {
			throw new Error('The Personal Access Token verifier store contains invalid JSON.', {
				cause: error,
			});
		}
		return PersonalAccessTokenStoreSchema.parse(value).verifier;
	}

	async save(verifier: PersonalAccessTokenVerifier): Promise<void> {
		const document = PersonalAccessTokenStoreSchema.parse({ verifier, version: 1 });
		const encoded = `${JSON.stringify(document)}\n`;
		const directory = path.dirname(this.#filePath);
		await mkdir(directory, { mode: 0o700, recursive: true });
		if (process.platform !== 'win32') await chmod(directory, 0o700);
		const temporaryPath = `${this.#filePath}.${this.#randomToken()}.tmp`;
		let temporary: PersonalAccessTokenTemporaryFile | undefined;
		let temporaryCreated = false;
		try {
			temporary = await this.#openTemporaryFile(temporaryPath);
			temporaryCreated = true;
			if (process.platform !== 'win32') await temporary.chmod(0o600);
			await temporary.writeFile(encoded, 'utf8');
			await temporary.sync();
			await temporary.close();
			temporary = undefined;
			await this.#replaceFile(temporaryPath, this.#filePath);
			temporaryCreated = false;
		} catch (error: unknown) {
			await temporary?.close().catch(() => undefined);
			if (temporaryCreated) await unlink(temporaryPath).catch(() => undefined);
			throw error;
		}
	}
}

function isMissingFileError(error: unknown): boolean {
	return typeof error === 'object' && error !== null && 'code' in error && error.code === 'ENOENT';
}
