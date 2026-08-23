import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { realpath } from 'node:fs/promises';
import path from 'node:path';

export interface GitProjectRepositoryObservation {
	readonly branch: string | null;
	readonly head: string;
	readonly localBaseRefs: readonly string[];
	readonly repositoryId: string;
	readonly repositoryPath: string;
}

export type GitProjectRepositoryObservationResult =
	| { readonly kind: 'not-repository' }
	| { readonly code: 'discovery-failed' | 'git-unavailable'; readonly kind: 'failed' }
	| { readonly kind: 'observed'; readonly repository: GitProjectRepositoryObservation };

export type GitProjectWorktreeValidationResult =
	| { readonly baseCommit: string; readonly kind: 'ready' }
	| {
			readonly code:
				| 'base-not-found'
				| 'branch-already-exists'
				| 'git-unavailable'
				| 'invalid-branch';
			readonly kind: 'failed';
	  };

interface GitCommandResult {
	readonly exitCode: number | null;
	readonly spawnErrorCode: string | null;
	readonly stderr: string;
	readonly stdout: string;
}

export interface GitProjectWorktreeAdapterDependencies {
	readonly realpath: (filePath: string) => Promise<string>;
	readonly run: (
		workingDirectory: string,
		arguments_: readonly string[],
	) => Promise<GitCommandResult>;
}

const defaultDependencies: GitProjectWorktreeAdapterDependencies = {
	realpath,
	run: runGit,
};

export class GitProjectWorktreeAdapter {
	readonly #dependencies: GitProjectWorktreeAdapterDependencies;

	constructor(dependencies: GitProjectWorktreeAdapterDependencies = defaultDependencies) {
		this.#dependencies = dependencies;
	}

	async observeRepository(directoryPath: string): Promise<GitProjectRepositoryObservationResult> {
		const commonDirectory = await this.#dependencies.run(directoryPath, [
			'rev-parse',
			'--git-common-dir',
		]);
		if (commonDirectory.spawnErrorCode !== null) {
			return { code: 'git-unavailable', kind: 'failed' };
		}
		if (commonDirectory.exitCode !== 0) return { kind: 'not-repository' };
		const commonValue = oneLine(commonDirectory.stdout);
		if (commonValue === null) return { code: 'discovery-failed', kind: 'failed' };
		let repositoryPath: string;
		try {
			repositoryPath = await this.#dependencies.realpath(path.resolve(directoryPath, commonValue));
		} catch {
			return { code: 'discovery-failed', kind: 'failed' };
		}
		const [headResult, branchResult, refsResult] = await Promise.all([
			this.#dependencies.run(directoryPath, ['rev-parse', '--verify', 'HEAD']),
			this.#dependencies.run(directoryPath, ['symbolic-ref', '--quiet', '--short', 'HEAD']),
			this.#dependencies.run(directoryPath, [
				'for-each-ref',
				'--format=%(refname:short)',
				'refs/heads',
			]),
		]);
		if (
			headResult.spawnErrorCode !== null ||
			branchResult.spawnErrorCode !== null ||
			refsResult.spawnErrorCode !== null
		) {
			return { code: 'git-unavailable', kind: 'failed' };
		}
		const head = oneLine(headResult.stdout);
		if (headResult.exitCode !== 0 || head === null || refsResult.exitCode !== 0) {
			return { code: 'discovery-failed', kind: 'failed' };
		}
		const branch = branchResult.exitCode === 0 ? oneLine(branchResult.stdout) : null;
		return {
			kind: 'observed',
			repository: {
				branch,
				head,
				localBaseRefs: uniqueNonEmptyLines(refsResult.stdout),
				repositoryId: repositoryId(repositoryPath),
				repositoryPath,
			},
		};
	}

	async validateCreation(
		sourcePath: string,
		baseRef: string,
		branchName: string,
	): Promise<GitProjectWorktreeValidationResult> {
		const branchValidation = await this.#dependencies.run(sourcePath, [
			'check-ref-format',
			'--branch',
			branchName,
		]);
		if (branchValidation.spawnErrorCode !== null) {
			return { code: 'git-unavailable', kind: 'failed' };
		}
		if (branchValidation.exitCode !== 0) return { code: 'invalid-branch', kind: 'failed' };
		const branchLookup = await this.#dependencies.run(sourcePath, [
			'show-ref',
			'--verify',
			'--quiet',
			`refs/heads/${branchName}`,
		]);
		if (branchLookup.spawnErrorCode !== null) {
			return { code: 'git-unavailable', kind: 'failed' };
		}
		if (branchLookup.exitCode === 0) {
			return { code: 'branch-already-exists', kind: 'failed' };
		}
		if (branchLookup.exitCode !== 1) return { code: 'git-unavailable', kind: 'failed' };
		const baseLookup = await this.#dependencies.run(sourcePath, [
			'rev-parse',
			'--verify',
			'--end-of-options',
			`${baseRef}^{commit}`,
		]);
		if (baseLookup.spawnErrorCode !== null) {
			return { code: 'git-unavailable', kind: 'failed' };
		}
		const baseCommit = oneLine(baseLookup.stdout);
		return baseLookup.exitCode === 0 && baseCommit !== null
			? { baseCommit, kind: 'ready' }
			: { code: 'base-not-found', kind: 'failed' };
	}

	async createWorktree(
		sourcePath: string,
		targetPath: string,
		branchName: string,
		baseRef: string,
	): Promise<'created' | 'failed'> {
		const result = await this.#dependencies.run(sourcePath, [
			'worktree',
			'add',
			'-b',
			branchName,
			'--',
			targetPath,
			baseRef,
		]);
		return result.exitCode === 0 ? 'created' : 'failed';
	}

	async compensateWorktree(
		sourcePath: string,
		targetPath: string,
		branchName: string,
		expectedCommit: string,
		branchOwned: boolean,
	): Promise<'compensated' | 'failed'> {
		const registration = await this.#observeTargetRegistration(sourcePath, targetPath, branchName);
		if (registration === 'failed' || registration === 'conflicting') return 'failed';
		let ownsBranch = branchOwned;
		if (registration === 'matching') {
			const safeToRemove = await this.#isUnmodifiedOwnedBranch(
				sourcePath,
				targetPath,
				branchName,
				expectedCommit,
			);
			if (!safeToRemove) return 'failed';
			const removal = await this.#dependencies.run(sourcePath, [
				'worktree',
				'remove',
				'--',
				targetPath,
			]);
			if (removal.spawnErrorCode !== null) return 'failed';
			if (removal.exitCode !== 0) {
				const prune = await this.#dependencies.run(sourcePath, ['worktree', 'prune']);
				if (prune.exitCode !== 0) return 'failed';
				const afterPrune = await this.#observeTargetRegistration(
					sourcePath,
					targetPath,
					branchName,
				);
				if (afterPrune !== 'absent') return 'failed';
			}
			ownsBranch = true;
		}
		const branchLookup = await this.#dependencies.run(sourcePath, [
			'show-ref',
			'--verify',
			'--quiet',
			`refs/heads/${branchName}`,
		]);
		if (branchLookup.spawnErrorCode !== null) return 'failed';
		if (branchLookup.exitCode === 1) return 'compensated';
		if (branchLookup.exitCode !== 0) return 'failed';
		if (!ownsBranch) return 'failed';
		const branchCommit = await this.#dependencies.run(sourcePath, [
			'rev-parse',
			'--verify',
			'--end-of-options',
			`refs/heads/${branchName}^{commit}`,
		]);
		if (
			branchCommit.spawnErrorCode !== null ||
			branchCommit.exitCode !== 0 ||
			oneLine(branchCommit.stdout) !== expectedCommit
		) {
			return 'failed';
		}
		const branchRemoval = await this.#dependencies.run(sourcePath, [
			'branch',
			'-D',
			'--',
			branchName,
		]);
		return branchRemoval.exitCode === 0 ? 'compensated' : 'failed';
	}

	async #isUnmodifiedOwnedBranch(
		sourcePath: string,
		targetPath: string,
		branchName: string,
		expectedCommit: string,
	): Promise<boolean> {
		const branchCommit = await this.#dependencies.run(sourcePath, [
			'rev-parse',
			'--verify',
			'--end-of-options',
			`refs/heads/${branchName}^{commit}`,
		]);
		if (
			branchCommit.spawnErrorCode !== null ||
			branchCommit.exitCode !== 0 ||
			oneLine(branchCommit.stdout) !== expectedCommit
		) {
			return false;
		}
		const [status, ignoredFiles] = await Promise.all([
			this.#dependencies.run(targetPath, ['status', '--porcelain=v1', '--untracked-files=all']),
			this.#dependencies.run(targetPath, [
				'ls-files',
				'--others',
				'--ignored',
				'--exclude-standard',
				'-z',
			]),
		]);
		return (
			status.spawnErrorCode === null &&
			status.exitCode === 0 &&
			status.stdout.length === 0 &&
			ignoredFiles.spawnErrorCode === null &&
			ignoredFiles.exitCode === 0 &&
			ignoredFiles.stdout.length === 0
		);
	}

	async #observeTargetRegistration(
		sourcePath: string,
		targetPath: string,
		branchName: string,
	): Promise<'absent' | 'conflicting' | 'failed' | 'matching'> {
		const listing = await this.#dependencies.run(sourcePath, ['worktree', 'list', '--porcelain']);
		if (listing.spawnErrorCode !== null || listing.exitCode !== 0) return 'failed';
		const expectedPath = await this.#canonicalPath(targetPath);
		for (const record of listing.stdout.split(/\r?\n\r?\n/u)) {
			const lines = record.split(/\r?\n/u);
			const worktreeLine = lines.find((line) => line.startsWith('worktree '));
			if (worktreeLine === undefined) continue;
			const registeredPath = worktreeLine.slice('worktree '.length);
			if ((await this.#canonicalPath(registeredPath)) !== expectedPath) continue;
			return lines.includes(`branch refs/heads/${branchName}`) ? 'matching' : 'conflicting';
		}
		return 'absent';
	}

	async #canonicalPath(filePath: string): Promise<string> {
		try {
			return await this.#dependencies.realpath(filePath);
		} catch {
			return path.resolve(filePath);
		}
	}
}

function repositoryId(repositoryPath: string): string {
	return `repository-${createHash('sha256').update(repositoryPath).digest('hex').slice(0, 32)}`;
}

function oneLine(value: string): string | null {
	const lines = uniqueNonEmptyLines(value);
	return lines.length === 1 ? (lines[0] ?? null) : null;
}

function uniqueNonEmptyLines(value: string): string[] {
	return [
		...new Set(
			value
				.split(/\r?\n/u)
				.map((line) => line.trim())
				.filter(Boolean),
		),
	];
}

function runGit(
	workingDirectory: string,
	arguments_: readonly string[],
): Promise<GitCommandResult> {
	return new Promise((resolve) => {
		execFile(
			'git',
			['--no-optional-locks', '-C', workingDirectory, ...arguments_],
			{
				env: { ...process.env, GIT_TERMINAL_PROMPT: '0' },
				maxBuffer: 4 * 1024 * 1024,
				timeout: 30_000,
			},
			(error, stdout, stderr) => {
				let exitCode: number | null = 0;
				if (error !== null) {
					exitCode = typeof error.code === 'number' ? error.code : null;
				}
				resolve({
					exitCode,
					spawnErrorCode: error !== null && typeof error.code === 'string' ? error.code : null,
					stderr,
					stdout,
				});
			},
		);
	});
}
