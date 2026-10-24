import { mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import { afterEach, describe, expect, it } from 'vitest';

import { FileWorkbenchDocumentStore } from '../FileWorkbenchDocumentStore';

function document(revision: string): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextResourceScopeSequence: 1,
			resourceScopes: [],
			workspaces: [],
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision(revision),
		terminalContexts: [
			{
				nextTabSequence: 1,
				panelWorkspace: {
					nextPanelSequence: 1,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 1,
					pendingRemovals: [],
					tabs: [],
				},
				tabCollection: {
					groups: [],
					tabs: [],
				},
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories.splice(0).map(async (directory) => rm(directory, { recursive: true })),
	);
});

describe('FileWorkbenchDocumentStore', () => {
	it('serializes exact-next commits through one private atomic file', async () => {
		const directory = await createTemporaryDirectory();
		const storePath = path.join(directory, 'state', 'workbench-shared-document-v1.json');
		let token = 1;
		const store = new FileWorkbenchDocumentStore(storePath, {
			randomToken: () => `temporary-${token++}`,
		});

		expect(await store.load()).toEqual({ kind: 'missing' });
		expect(
			await Promise.all([
				store.commit(null, document('0')),
				store.commit(workbenchSharedDocumentRevision('0'), document('1')),
			]),
		).toEqual([
			{ kind: 'committed', revision: '0' },
			{ kind: 'committed', revision: '1' },
		]);
		expect(await store.load()).toEqual({ document: document('1'), kind: 'available' });
		expect((await readFile(storePath, 'utf8')).endsWith('\n')).toBe(true);
		expect(await readdir(path.dirname(storePath))).toEqual(['workbench-shared-document-v1.json']);
		const fileMode = process.platform === 'win32' ? 0o600 : (await stat(storePath)).mode & 0o777;
		const directoryMode =
			process.platform === 'win32' ? 0o700 : (await stat(path.dirname(storePath))).mode & 0o777;
		expect(fileMode).toBe(0o600);
		expect(directoryMode).toBe(0o700);
	});

	it('lets only the first concurrent writer satisfy one expected revision', async () => {
		const directory = await createTemporaryDirectory();
		const store = new FileWorkbenchDocumentStore(path.join(directory, 'document.json'));
		await store.commit(null, document('0'));

		expect(
			await Promise.all([
				store.commit(workbenchSharedDocumentRevision('0'), document('1')),
				store.commit(workbenchSharedDocumentRevision('0'), document('1')),
			]),
		).toEqual([
			{ kind: 'committed', revision: '1' },
			{ actualRevision: '1', kind: 'conflict' },
		]);
	});

	it('fails closed for invalid current data and never overwrites it', async () => {
		const directory = await createTemporaryDirectory();
		const storePath = path.join(directory, 'document.json');
		await writeFile(storePath, '{not-json', 'utf8');
		const store = new FileWorkbenchDocumentStore(storePath);

		expect(await store.load()).toEqual({ kind: 'unavailable', reason: 'invalid' });
		expect(await store.commit(null, document('0'))).toEqual({
			kind: 'unavailable',
			reason: 'invalid',
		});
		expect(await readFile(storePath, 'utf8')).toBe('{not-json');
	});

	it('reports a write failure and preserves a pre-rename current file', async () => {
		const directory = await createTemporaryDirectory();
		const storePath = path.join(directory, 'document.json');
		await writeFile(storePath, `${JSON.stringify(document('0'))}\n`, 'utf8');
		await writeFile(`${storePath}.collision.tmp`, 'occupied', 'utf8');
		const store = new FileWorkbenchDocumentStore(storePath, {
			randomToken: () => 'collision',
		});

		expect(await store.commit(workbenchSharedDocumentRevision('0'), document('1'))).toEqual({
			kind: 'unavailable',
			reason: 'write-failed',
		});
		expect(await store.load()).toEqual({ document: document('0'), kind: 'available' });
	});

	it('reports a post-rename commit as indeterminate and reconciles by loading the current document', async () => {
		const directory = await createTemporaryDirectory();
		const storePath = path.join(directory, 'workbench-shared-document-v1.json');
		const store = new FileWorkbenchDocumentStore(storePath, {
			synchronizeParentDirectory: async () => {
				throw new Error('injected post-rename failure');
			},
		});

		expect(await store.commit(null, document('0'))).toEqual({
			kind: 'unavailable',
			reason: 'write-indeterminate',
		});
		expect(await store.load()).toEqual({ document: document('0'), kind: 'available' });
	});
});

async function createTemporaryDirectory(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-document-store-'));
	temporaryDirectories.push(directory);
	return directory;
}
