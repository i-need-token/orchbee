import { mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { CreationIdSchema, HostEpochSchema } from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it } from 'vitest';

import { createDefaultTerminalProfile } from '../../host';
import { FileTerminalBindingStore } from '../FileTerminalBindingStore';
import {
	createTerminalBindingStoreDocument,
	TerminalBindingIdSchema,
	TerminalBindingSchema,
	type TerminalBinding,
} from '../terminal-binding-store';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories.splice(0).map(async (directory) => rm(directory, { recursive: true })),
	);
});

describe('FileTerminalBindingStore', () => {
	it('serializes targeted writes without losing unrelated bindings', async () => {
		const directory = await createTemporaryDirectory();
		const storePath = path.join(directory, 'recovery', 'bindings-v2.json');
		const store = new FileTerminalBindingStore(storePath, { randomToken: () => 'temporary' });
		const first = createBinding(1);
		const second = createBinding(2);

		expect(await store.load()).toEqual({ kind: 'missing' });
		await Promise.all([store.put(first), store.put(second)]);
		const requested = TerminalBindingSchema.parse({
			...first,
			state: { hostEpoch: HostEpochSchema.parse('aa'.repeat(16)), kind: 'create-requested' },
		});
		await store.put(requested);
		await store.remove(second.bindingId);

		expect(await store.load()).toEqual({
			document: { bindings: [requested], version: 2 },
			kind: 'available',
		});
		expect((await readFile(storePath, 'utf8')).endsWith('\n')).toBe(true);
		expect(await readdir(path.dirname(storePath))).toEqual(['bindings-v2.json']);
		const fileMode = process.platform === 'win32' ? 0o600 : (await stat(storePath)).mode & 0o777;
		const directoryMode =
			process.platform === 'win32' ? 0o700 : (await stat(path.dirname(storePath))).mode & 0o777;
		expect(fileMode).toBe(0o600);
		expect(directoryMode).toBe(0o700);
	});

	it('fails closed for corrupt current data and does not overwrite it', async () => {
		const directory = await createTemporaryDirectory();
		const storePath = path.join(directory, 'bindings-v2.json');
		await writeFile(storePath, '{not-json', 'utf8');
		const store = new FileTerminalBindingStore(storePath);

		expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
		await expect(store.put(createBinding(2))).rejects.toThrow('unavailable');
		expect(await readFile(storePath, 'utf8')).toBe('{not-json');
	});

	it('loads a valid collection larger than the former whole-file byte cap', async () => {
		const directory = await createTemporaryDirectory();
		const storePath = path.join(directory, 'bindings-v2.json');
		const document = createTerminalBindingStoreDocument(
			Array.from({ length: 257 }, (_, index) => createBinding(index + 1)),
		);
		const encoded = JSON.stringify(document);
		expect(encoded.length).toBeGreaterThan(128 * 1024);
		await writeFile(storePath, encoded, 'utf8');

		const loaded = await new FileTerminalBindingStore(storePath).load();
		expect(loaded.kind).toBe('available');
		if (loaded.kind !== 'available') throw new Error('Expected the large binding store to load.');
		expect(loaded.document.bindings).toHaveLength(257);
	});
});

async function createTemporaryDirectory(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-binding-store-'));
	temporaryDirectories.push(directory);
	return directory;
}

function createBinding(index: number): TerminalBinding {
	return TerminalBindingSchema.parse({
		bindingId: TerminalBindingIdSchema.parse(`binding-${index.toString().padStart(4, '0')}`),
		creationId: CreationIdSchema.parse(index.toString(16).padStart(32, '0')),
		launch: {
			arguments: [],
			cellPixelHeight: 16,
			cellPixelWidth: 8,
			columns: 80,
			currentDirectory: '/users/orchbee',
			environment: [{ name: 'TERM', value: 'xterm-256color' }],
			executable: '/bin/sh',
			rows: 24,
			terminalProfile: createDefaultTerminalProfile(),
		},
		state: { kind: 'planned' },
	});
}
