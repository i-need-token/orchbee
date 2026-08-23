import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import { GitProjectWorktreeAdapter } from '../GitProjectWorktreeAdapter';

const temporaryDirectories: string[] = [];

afterEach(() => {
	for (const directory of temporaryDirectories.splice(0)) {
		rmSync(directory, { force: true, recursive: true });
	}
});

describe('GitProjectWorktreeAdapter', () => {
	it('discovers local repository state and creates and compensates one Worktree', async () => {
		const root = temporaryDirectory();
		const repository = path.join(root, 'repository');
		git(root, ['init', '-b', 'main', repository]);
		git(repository, ['config', 'user.email', 'test@orchbee.local']);
		git(repository, ['config', 'user.name', 'Orchbee Test']);
		writeFileSync(path.join(repository, 'README.md'), 'orchbee\n', 'utf8');
		git(repository, ['add', 'README.md']);
		git(repository, ['commit', '-m', 'initial']);
		const adapter = new GitProjectWorktreeAdapter();

		const observation = await adapter.observeRepository(repository);
		expect(observation).toMatchObject({
			kind: 'observed',
			repository: {
				branch: 'main',
				localBaseRefs: ['main'],
				repositoryId: expect.stringMatching(/^repository-[0-9a-f]{32}$/u),
			},
		});
		const validation = await adapter.validateCreation(repository, 'main', 'orchbee/workspace-1');
		expect(validation).toMatchObject({ kind: 'ready' });
		const target = path.join(root, 'managed', 'managed-worktree-1');
		expect(await adapter.createWorktree(repository, target, 'orchbee/workspace-1', 'main')).toBe(
			'created',
		);
		expect(git(target, ['branch', '--show-current'])).toBe('orchbee/workspace-1');
		expect(
			await adapter.compensateWorktree(
				repository,
				target,
				'orchbee/workspace-1',
				validation.kind === 'ready' ? validation.baseCommit : '',
				true,
			),
		).toBe('compensated');
		expect(git(repository, ['branch', '--list', 'orchbee/workspace-1'])).toBe('');
	});

	it('distinguishes invalid, existing, and unavailable bases before mutation', async () => {
		const root = temporaryDirectory();
		const repository = path.join(root, 'repository');
		git(root, ['init', '-b', 'main', repository]);
		git(repository, ['config', 'user.email', 'test@orchbee.local']);
		git(repository, ['config', 'user.name', 'Orchbee Test']);
		writeFileSync(path.join(repository, 'file.txt'), 'one\n', 'utf8');
		git(repository, ['add', 'file.txt']);
		git(repository, ['commit', '-m', 'initial']);
		const adapter = new GitProjectWorktreeAdapter();

		expect(await adapter.validateCreation(repository, 'main', 'bad branch')).toEqual({
			code: 'invalid-branch',
			kind: 'failed',
		});
		expect(await adapter.validateCreation(repository, 'main', 'main')).toEqual({
			code: 'branch-already-exists',
			kind: 'failed',
		});
		expect(await adapter.validateCreation(repository, 'missing', 'feature')).toEqual({
			code: 'base-not-found',
			kind: 'failed',
		});
	});

	it('retains recovery responsibility for an unowned branch that appears after validation', async () => {
		const root = temporaryDirectory();
		const repository = createRepository(root);
		const adapter = new GitProjectWorktreeAdapter();
		const validation = await adapter.validateCreation(repository, 'main', 'external-branch');
		if (validation.kind !== 'ready') throw new Error('Expected the new branch to validate.');
		git(repository, ['branch', 'external-branch', 'main']);

		expect(
			await adapter.compensateWorktree(
				repository,
				path.join(root, 'managed', 'never-created'),
				'external-branch',
				validation.baseCommit,
				false,
			),
		).toBe('failed');
		expect(git(repository, ['branch', '--list', 'external-branch'])).toContain('external-branch');
	});

	it('retains a created Worktree when ignored files appear before recovery', async () => {
		const root = temporaryDirectory();
		const repository = createRepository(root);
		writeFileSync(path.join(repository, '.gitignore'), 'ignored.txt\n', 'utf8');
		git(repository, ['add', '.gitignore']);
		git(repository, ['commit', '-m', 'ignore generated file']);
		const adapter = new GitProjectWorktreeAdapter();
		const validation = await adapter.validateCreation(repository, 'main', 'ignored-worktree');
		if (validation.kind !== 'ready') throw new Error('Expected the new branch to validate.');
		const target = path.join(root, 'managed', 'ignored-worktree');
		expect(await adapter.createWorktree(repository, target, 'ignored-worktree', 'main')).toBe(
			'created',
		);
		writeFileSync(path.join(target, 'ignored.txt'), 'preserve me\n', 'utf8');

		expect(
			await adapter.compensateWorktree(
				repository,
				target,
				'ignored-worktree',
				validation.baseCommit,
				true,
			),
		).toBe('failed');
		expect(git(target, ['branch', '--show-current'])).toBe('ignored-worktree');
	});

	it('retains a created Worktree when its branch or files changed before recovery', async () => {
		const root = temporaryDirectory();
		const repository = createRepository(root);
		const adapter = new GitProjectWorktreeAdapter();
		const validation = await adapter.validateCreation(repository, 'main', 'changed-worktree');
		if (validation.kind !== 'ready') throw new Error('Expected the new branch to validate.');
		const target = path.join(root, 'managed', 'changed-worktree');
		expect(await adapter.createWorktree(repository, target, 'changed-worktree', 'main')).toBe(
			'created',
		);
		writeFileSync(path.join(target, 'untracked.txt'), 'preserve me\n', 'utf8');

		expect(
			await adapter.compensateWorktree(
				repository,
				target,
				'changed-worktree',
				validation.baseCommit,
				true,
			),
		).toBe('failed');
		expect(git(target, ['branch', '--show-current'])).toBe('changed-worktree');
	});
});

function temporaryDirectory(): string {
	const directory = mkdtempSync(path.join(tmpdir(), 'orchbee-worktree-git-'));
	temporaryDirectories.push(directory);
	return directory;
}

function createRepository(root: string): string {
	const repository = path.join(root, 'repository');
	git(root, ['init', '-b', 'main', repository]);
	git(repository, ['config', 'user.email', 'test@orchbee.local']);
	git(repository, ['config', 'user.name', 'Orchbee Test']);
	writeFileSync(path.join(repository, 'README.md'), 'orchbee\n', 'utf8');
	git(repository, ['add', 'README.md']);
	git(repository, ['commit', '-m', 'initial']);
	return repository;
}

function git(workingDirectory: string, arguments_: readonly string[]): string {
	return execFileSync('git', ['-C', workingDirectory, ...arguments_], {
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'pipe'],
	}).trim();
}
