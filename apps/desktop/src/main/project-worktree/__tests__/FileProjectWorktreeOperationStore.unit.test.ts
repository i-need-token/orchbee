import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import {
	FileProjectWorktreeOperationStore,
	type ProjectWorktreeOperationRecord,
} from '../FileProjectWorktreeOperationStore';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories
			.splice(0)
			.map(async (directory) => rm(directory, { force: true, recursive: true })),
	);
});

describe('FileProjectWorktreeOperationStore', () => {
	it('distinguishes a missing ledger from invalid persisted data', async () => {
		const filePath = await temporaryFilePath();
		const store = new FileProjectWorktreeOperationStore(filePath);

		await expect(store.load()).resolves.toEqual({ kind: 'ready', operations: [] });
		await writeFile(filePath, '{"version":2}\n', 'utf8');
		await expect(store.load()).resolves.toEqual({ kind: 'unavailable', reason: 'invalid' });
	});

	it('atomically replaces and restores the canonical pending operations', async () => {
		const filePath = await temporaryFilePath();
		const store = new FileProjectWorktreeOperationStore(filePath, () => 'first');
		const operation = pendingOperation();

		await store.replace([operation]);

		await expect(store.load()).resolves.toEqual({ kind: 'ready', operations: [operation] });
		expect(JSON.parse(await readFile(filePath, 'utf8'))).toEqual({
			operations: [operation],
			version: 1,
		});
	});

	it('rejects a historical ledger and leaves the persisted bytes unchanged', async () => {
		const filePath = await temporaryFilePath();
		const {
			sourceWorkspaceId: _workspace,
			sourceResourceScopeId: _scope,
			sourceTerminalContextId: _context,
			...operation
		} = pendingOperation();
		const oldRecord = { version: 1, operations: [{ ...operation, projectId: 'project-1' }] };
		const original = JSON.stringify(oldRecord);
		await writeFile(filePath, original);
		expect(await new FileProjectWorktreeOperationStore(filePath).load()).toEqual({
			kind: 'unavailable',
			reason: 'invalid',
		});
		expect(await readFile(filePath, 'utf8')).toBe(original);
	});

	it('preserves the prior ledger when atomic replacement cannot create its temporary file', async () => {
		const filePath = await temporaryFilePath();
		const operation = pendingOperation();
		const first = new FileProjectWorktreeOperationStore(filePath, () => 'first');
		await first.replace([operation]);
		await writeFile(`${filePath}.collision.tmp`, 'occupied', 'utf8');
		const failing = new FileProjectWorktreeOperationStore(filePath, () => 'collision');

		await expect(failing.replace([])).rejects.toMatchObject({ code: 'EEXIST' });
		await expect(first.load()).resolves.toEqual({ kind: 'ready', operations: [operation] });
	});
});

function pendingOperation(): ProjectWorktreeOperationRecord {
	return {
		baseCommit: 'abc123',
		baseRef: 'main',
		branchName: 'orchbee/workspace-1',
		createWorkspace: true,
		directoryId: 'managed-worktree-operation1234',
		operationId: 'project-worktree-operation1234',
		phase: 'git-created',
		sourceResourceScopeId: 'resource-scope-1',
		sourceTerminalContextId: 'terminal-context-2',
		sourceWorkspaceId: 'workspace-1',
		repositoryId: 'repository-1',
		sourceDirectoryId: 'directory-1',
		sourcePath: '/projects/orchbee',
		targetPath: '/managed/worktrees/managed-worktree-operation1234',
	};
}

async function temporaryFilePath(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-project-worktree-'));
	temporaryDirectories.push(directory);
	return path.join(directory, 'project-worktree-operations-v1.json');
}
