import { mkdtemp, mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it, vi } from 'vitest';

import { FileEnvironmentRegistryStore } from '../FileEnvironmentRegistryStore';
import {
	DESKTOP_ENVIRONMENT_REGISTRY_LIMITS,
	EnvironmentRecordSchema,
	EnvironmentRegistryDocumentSchema,
	createEmptyEnvironmentRegistryDocument,
	type EnvironmentRecord,
} from '../environment-registry-contract';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories
			.splice(0)
			.map(async (directory) => rm(directory, { force: true, recursive: true })),
	);
});

describe('FileEnvironmentRegistryStore', () => {
	it('treats a missing file as empty but blocks malformed, duplicate, and oversized records', async () => {
		const filePath = await temporaryFilePath();
		const store = new FileEnvironmentRegistryStore(filePath);
		await expect(store.load()).resolves.toEqual({
			document: createEmptyEnvironmentRegistryDocument(),
			kind: 'ready',
		});

		await writeFile(filePath, '{"schemaVersion":2}\n', 'utf8');
		await expect(store.load()).resolves.toEqual({ kind: 'unavailable', reason: 'invalid-record' });
		const duplicate = environment(1, 1);
		await writeFile(
			filePath,
			`${JSON.stringify({ environments: [duplicate, duplicate], revision: 2, schemaVersion: 2 })}\n`,
			'utf8',
		);
		await expect(store.load()).resolves.toEqual({ kind: 'unavailable', reason: 'invalid-record' });
		await writeFile(filePath, Buffer.from([0xff]));
		await expect(store.load()).resolves.toEqual({ kind: 'unavailable', reason: 'invalid-record' });
		await writeFile(
			filePath,
			Buffer.alloc(DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumDocumentBytes + 2, 0x20),
		);
		await expect(store.load()).resolves.toEqual({ kind: 'unavailable', reason: 'invalid-record' });
	});

	it('atomically writes the canonical document with owner-only permissions', async () => {
		const filePath = await temporaryFilePath();
		const store = new FileEnvironmentRegistryStore(filePath, { randomToken: () => 'first' });
		const document = documentWith(environment(1, 1));

		await expect(store.commit(0, document)).resolves.toEqual({ kind: 'committed' });
		await expect(store.load()).resolves.toEqual({ document, kind: 'ready' });
		await expect(readFile(filePath, 'utf8')).resolves.toBe(`${JSON.stringify(document)}\n`);
		let posixModes: { readonly directory: number; readonly file: number } | null = null;
		if (process.platform !== 'win32') {
			posixModes = {
				directory: (await stat(path.dirname(filePath))).mode & 0o777,
				file: (await stat(filePath)).mode & 0o777,
			};
		}
		expect(posixModes).toEqual(
			process.platform === 'win32' ? null : { directory: 0o700, file: 0o600 },
		);
	});

	it('serializes competing commits and returns the current document to the stale caller', async () => {
		const filePath = await temporaryFilePath();
		let token = 0;
		const store = new FileEnvironmentRegistryStore(filePath, {
			randomToken: () => `token-${++token}`,
		});
		const first = documentWith(environment(1, 1));
		const second = documentWith(environment(2, 2));

		const [firstResult, secondResult] = await Promise.all([
			store.commit(0, first),
			store.commit(0, second),
		]);

		expect(firstResult).toEqual({ kind: 'committed' });
		expect(secondResult).toEqual({ document: first, kind: 'conflict' });
		await expect(store.load()).resolves.toEqual({ document: first, kind: 'ready' });
	});

	it('preserves the prior document when temporary-file admission fails', async () => {
		const filePath = await temporaryFilePath();
		const first = new FileEnvironmentRegistryStore(filePath, { randomToken: () => 'first' });
		const firstDocument = documentWith(environment(1, 1));
		await first.commit(0, firstDocument);
		await writeFile(`${filePath}.collision.tmp`, 'occupied', 'utf8');
		const failing = new FileEnvironmentRegistryStore(filePath, {
			randomToken: () => 'collision',
		});
		const secondDocument = EnvironmentRegistryDocumentSchema.parse({
			environments: [firstDocument.environments[0], environment(2, 2)],
			revision: 2,
			schemaVersion: 2,
		});

		await expect(failing.commit(1, secondDocument)).resolves.toEqual({
			kind: 'unavailable',
			reason: 'write-failed',
		});
		await expect(first.load()).resolves.toEqual({ document: firstDocument, kind: 'ready' });
	});

	it('reports a post-rename directory-sync failure as indeterminate without lying about commit', async () => {
		const filePath = await temporaryFilePath();
		const synchronizeDirectory = vi.fn<(directory: string) => Promise<void>>(async () => {
			throw new Error('injected directory sync failure');
		});
		const store = new FileEnvironmentRegistryStore(filePath, {
			platform: 'linux',
			randomToken: () => 'indeterminate',
			synchronizeDirectory,
		});
		const document = documentWith(environment(1, 1));

		await expect(store.commit(0, document)).resolves.toEqual({
			kind: 'indeterminate',
			reason: 'durability-unconfirmed',
		});
		expect(synchronizeDirectory).toHaveBeenCalledWith(path.dirname(filePath));
		await expect(store.load()).resolves.toEqual({ document, kind: 'ready' });
	});

	it('does not overwrite an invalid current file or silently recreate a missing admitted revision', async () => {
		const filePath = await temporaryFilePath();
		const store = new FileEnvironmentRegistryStore(filePath, { randomToken: () => 'blocked' });
		const next = documentWith(environment(1, 1));
		await writeFile(filePath, '{"invalid":true}\n', 'utf8');
		await expect(store.commit(0, next)).resolves.toEqual({
			kind: 'unavailable',
			reason: 'invalid-record',
		});
		await rm(filePath);
		await expect(store.commit(1, next)).resolves.toEqual({
			kind: 'unavailable',
			reason: 'current-record-missing',
		});
	});

	it('rejects a directory at the registry path as an invalid record', async () => {
		const filePath = await temporaryFilePath();
		await mkdir(filePath);
		const store = new FileEnvironmentRegistryStore(filePath);

		await expect(store.load()).resolves.toEqual({ kind: 'unavailable', reason: 'invalid-record' });
	});

	it('migrates a valid version-1 registry without rewriting or deleting the legacy file', async () => {
		const filePath = await temporaryFilePath();
		const legacyFilePath = path.join(path.dirname(filePath), 'environment-registry-v1.json');
		const legacyEnvironment = environment(1, 1);
		const legacyDocument = {
			environments: [
				{
					...legacyEnvironment,
					provider: {
						...legacyEnvironment.provider,
						target: {
							configurationFilePath: null,
							destination: 'build-1',
						},
					},
				},
			],
			revision: 1,
			schemaVersion: 1,
		};
		await writeFile(legacyFilePath, `${JSON.stringify(legacyDocument)}\n`, 'utf8');
		const store = new FileEnvironmentRegistryStore(filePath, { legacyFilePath });

		const loaded = await store.load();
		expect(loaded).toMatchObject({
			document: { revision: 1, schemaVersion: 2 },
			kind: 'ready',
		});
		await expect(readFile(legacyFilePath, 'utf8')).resolves.toBe(
			`${JSON.stringify(legacyDocument)}\n`,
		);
		await expect(readFile(filePath, 'utf8')).rejects.toMatchObject({ code: 'ENOENT' });
		if (loaded.kind !== 'ready') throw new Error('Expected the migrated registry.');
		const nextDocument = EnvironmentRegistryDocumentSchema.parse({
			environments: [...loaded.document.environments, environment(2, 2)],
			revision: 2,
			schemaVersion: 2,
		});
		await expect(store.commit(1, nextDocument)).resolves.toEqual({ kind: 'committed' });
		expect(
			EnvironmentRegistryDocumentSchema.parse(JSON.parse(await readFile(filePath, 'utf8'))),
		).toEqual(nextDocument);
		await expect(readFile(legacyFilePath, 'utf8')).resolves.toBe(
			`${JSON.stringify(legacyDocument)}\n`,
		);
	});

	it('does not fall back to a valid version-1 registry when version 2 is invalid', async () => {
		const filePath = await temporaryFilePath();
		const legacyFilePath = path.join(path.dirname(filePath), 'environment-registry-v1.json');
		await writeFile(filePath, '{"schemaVersion":2,"invalid":true}\n', 'utf8');
		await writeFile(
			legacyFilePath,
			`${JSON.stringify({
				environments: [],
				revision: 0,
				schemaVersion: 1,
			})}\n`,
			'utf8',
		);
		const store = new FileEnvironmentRegistryStore(filePath, { legacyFilePath });

		await expect(store.load()).resolves.toEqual({ kind: 'unavailable', reason: 'invalid-record' });
	});
});

function documentWith(record: EnvironmentRecord) {
	return EnvironmentRegistryDocumentSchema.parse({
		environments: [record],
		revision: 1,
		schemaVersion: 2,
	});
}

function environment(identity: number, target: number): EnvironmentRecord {
	return EnvironmentRecordSchema.parse({
		environmentId: `environment:${hexIdentity(identity)}`,
		name: `Environment ${identity}`,
		provider: {
			kind: 'ssh',
			runtimeInstanceId: hexIdentity(identity + 10),
			target: { configurationFilePath: null, destination: `build-${target}` },
		},
	});
}

function hexIdentity(value: number): string {
	return value.toString(16).padStart(32, '0');
}

async function temporaryFilePath(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-environment-registry-'));
	temporaryDirectories.push(directory);
	const privateDirectory = path.join(directory, 'private');
	await mkdir(privateDirectory);
	return path.join(privateDirectory, 'environment-registry-v2.json');
}
