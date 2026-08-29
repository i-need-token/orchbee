import { chmod, lstat, mkdir } from 'node:fs/promises';
import path from 'node:path';

import type {
	DesktopProjectWorktreeAdoptionResult,
	DesktopProjectWorktreeCreationResult,
	DesktopProjectWorktreeDiscovery,
	DesktopProjectWorktreeFailureCode,
} from '@orchbee/desktop-client-protocol';
import {
	WorkbenchProjectWorkspaceRuntime,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '@orchbee/workbench';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench/document';

import type { DesktopClientSessionCoordinator } from '../client-session';
import {
	GitProjectWorktreeAdapter,
	type GitProjectRepositoryObservation,
	type GitProjectWorktreeObservation,
} from './GitProjectWorktreeAdapter';
import type {
	ProjectWorktreeOperationRecord,
	ProjectWorktreeOperationStore,
} from './FileProjectWorktreeOperationStore';

export type DesktopProjectWorktreeRuntimeResult<Result> =
	| { readonly kind: 'completed'; readonly value: Result }
	| { readonly code: DesktopProjectWorktreeFailureCode; readonly kind: 'failed' };

export interface DesktopProjectWorktreeRuntimeOptions {
	readonly allocateIdentifier: () => string;
	readonly coordinator: DesktopClientSessionCoordinator;
	readonly defaultCwd: string;
	readonly managedRoot: string;
	readonly operationStore: ProjectWorktreeOperationStore;
	readonly terminalTitle: string;
}

export interface DesktopProjectWorktreeRuntimeDependencies {
	readonly chmod: (filePath: string, mode: number) => Promise<void>;
	readonly git: Pick<
		GitProjectWorktreeAdapter,
		| 'compensateWorktree'
		| 'createWorktree'
		| 'listWorktrees'
		| 'observeRepository'
		| 'validateCreation'
	>;
	readonly lstat: (filePath: string) => Promise<unknown>;
	readonly mkdir: (
		filePath: string,
		options: { readonly mode: number; readonly recursive: true },
	) => Promise<unknown>;
}

const defaultDependencies: DesktopProjectWorktreeRuntimeDependencies = {
	chmod,
	git: new GitProjectWorktreeAdapter(),
	lstat,
	mkdir,
};

export class DesktopProjectWorktreeRuntime {
	readonly #dependencies: DesktopProjectWorktreeRuntimeDependencies;
	readonly #options: DesktopProjectWorktreeRuntimeOptions;
	readonly #operations = new Map<string, ProjectWorktreeOperationRecord>();
	readonly #repositoryTails = new Map<string, Promise<void>>();
	#disposed = false;
	#startResult: 'ready' | 'unavailable' | undefined;

	constructor(
		options: DesktopProjectWorktreeRuntimeOptions,
		dependencies: DesktopProjectWorktreeRuntimeDependencies = defaultDependencies,
	) {
		this.#options = { ...options, managedRoot: path.resolve(options.managedRoot) };
		this.#dependencies = dependencies;
	}

	async start(): Promise<'ready' | 'unavailable'> {
		if (this.#disposed) return 'unavailable';
		if (this.#startResult !== undefined) return this.#startResult;
		try {
			await this.#dependencies.mkdir(this.#options.managedRoot, {
				mode: 0o700,
				recursive: true,
			});
			if (process.platform !== 'win32') {
				await this.#dependencies.chmod(this.#options.managedRoot, 0o700);
			}
		} catch {
			this.#startResult = 'unavailable';
			return this.#startResult;
		}
		const loaded = await this.#options.operationStore.load();
		if (loaded.kind !== 'ready') {
			this.#startResult = 'unavailable';
			return this.#startResult;
		}
		for (const operation of loaded.operations) {
			if (!this.#isOwnedTarget(operation.targetPath, operation.directoryId)) {
				this.#startResult = 'unavailable';
				return this.#startResult;
			}
			this.#operations.set(operation.operationId, operation);
		}
		await this.#recoverPendingOperations();
		this.#startResult = 'ready';
		return this.#startResult;
	}

	async discover(
		projectId: string,
		preferredDirectoryId: string | null,
	): Promise<DesktopProjectWorktreeRuntimeResult<DesktopProjectWorktreeDiscovery>> {
		if (this.#startResult !== 'ready' || this.#disposed) return failed('control-unavailable');
		const document = this.#options.coordinator.canonicalDocument;
		const project = document?.projectWorkspace.projects.find(
			(candidate) => candidate.id === projectId,
		);
		if (project === undefined) return failed('project-not-found');
		const preferredIds = [preferredDirectoryId, project.primaryDirectoryId].filter(
			(value): value is string => value !== null,
		);
		const orderedDirectories = [...project.directories].sort((left, right) => {
			const leftRank = preferredIds.indexOf(left.id);
			const rightRank = preferredIds.indexOf(right.id);
			return (
				(leftRank < 0 ? preferredIds.length : leftRank) -
				(rightRank < 0 ? preferredIds.length : rightRank)
			);
		});
		const repositories = new Map<
			string,
			{
				readonly label: string;
				readonly listing: Awaited<
					ReturnType<DesktopProjectWorktreeRuntimeDependencies['git']['listWorktrees']>
				>;
				readonly observation: GitProjectRepositoryObservation;
				readonly representedWorktreeIds: Set<string>;
				readonly sourceDirectoryId: string;
			}
		>();
		for (const directory of orderedDirectories) {
			if (!directory.available) continue;
			const observation = await this.#dependencies.git.observeRepository(directory.path);
			if (observation.kind === 'failed' && observation.code === 'git-unavailable') {
				return failed('git-unavailable');
			}
			if (observation.kind !== 'observed') continue;
			const repositoryId = observation.repository.repositoryId;
			const existingRepository = repositories.get(repositoryId);
			if (existingRepository === undefined) {
				repositories.set(repositoryId, {
					label: directory.name,
					listing: await this.#dependencies.git.listWorktrees(directory.path),
					observation: observation.repository,
					representedWorktreeIds: new Set([observation.repository.worktreeId]),
					sourceDirectoryId: directory.id,
				});
			} else {
				existingRepository.representedWorktreeIds.add(observation.repository.worktreeId);
			}
		}
		return {
			kind: 'completed',
			value: {
				managedRoot: this.#options.managedRoot,
				repositories: [...repositories.values()].map(
					({ label, listing, observation, representedWorktreeIds, sourceDirectoryId }) => ({
						defaultBaseRef: observation.branch ?? observation.head,
						label,
						localBaseRefs: [...observation.localBaseRefs],
						repositoryId: observation.repositoryId,
						sourceDirectoryId,
						suggestedBranchName: suggestedBranchName(observation.localBaseRefs),
						worktreeInventory:
							listing.kind === 'failed'
								? { kind: 'unavailable' as const }
								: {
										kind: 'available' as const,
										worktrees: listing.worktrees
											.filter((worktree) => !representedWorktreeIds.has(worktree.id))
											.map((worktree) => ({
												label: worktreeLabel(worktree),
												worktreeId: worktree.id,
											})),
									},
					}),
				),
			},
		};
	}

	adopt(input: {
		readonly projectId: string;
		readonly repositoryId: string;
		readonly sourceDirectoryId: string;
		readonly worktreeId: string;
	}): Promise<DesktopProjectWorktreeRuntimeResult<DesktopProjectWorktreeAdoptionResult>> {
		if (this.#startResult !== 'ready' || this.#disposed) {
			return Promise.resolve(failed('control-unavailable'));
		}
		return this.#enqueueRepository(input.repositoryId, async () => this.#adoptOwned(input));
	}

	create(input: {
		readonly baseRef: string;
		readonly branchName: string;
		readonly createWorkspace: boolean;
		readonly projectId: string;
		readonly repositoryId: string;
		readonly sourceDirectoryId: string;
	}): Promise<DesktopProjectWorktreeRuntimeResult<DesktopProjectWorktreeCreationResult>> {
		if (this.#startResult !== 'ready' || this.#disposed) {
			return Promise.resolve(failed('control-unavailable'));
		}
		return this.#enqueueRepository(input.repositoryId, async () => this.#createOwned(input));
	}

	async dispose(): Promise<void> {
		this.#disposed = true;
		await Promise.allSettled(this.#repositoryTails.values());
	}

	async #adoptOwned(input: {
		readonly projectId: string;
		readonly repositoryId: string;
		readonly sourceDirectoryId: string;
		readonly worktreeId: string;
	}): Promise<DesktopProjectWorktreeRuntimeResult<DesktopProjectWorktreeAdoptionResult>> {
		if (
			[...this.#operations.values()].some(
				(operation) => operation.repositoryId === input.repositoryId,
			)
		) {
			return failed('recovery-required');
		}
		const document = this.#options.coordinator.canonicalDocument;
		const project = document?.projectWorkspace.projects.find(
			(candidate) => candidate.id === input.projectId,
		);
		if (project === undefined) return failed('project-not-found');
		const sourceDirectory = project.directories.find(
			(directory) => directory.id === input.sourceDirectoryId,
		);
		if (sourceDirectory === undefined || !sourceDirectory.available) {
			return failed('source-unavailable');
		}
		const sourceObservation = await this.#dependencies.git.observeRepository(sourceDirectory.path);
		if (sourceObservation.kind === 'failed') return failed(sourceObservation.code);
		if (
			sourceObservation.kind !== 'observed' ||
			sourceObservation.repository.repositoryId !== input.repositoryId
		) {
			return failed('repository-not-found');
		}
		if (sourceObservation.repository.worktreeId === input.worktreeId) {
			return failed('worktree-not-found');
		}
		const listing = await this.#dependencies.git.listWorktrees(sourceDirectory.path);
		if (listing.kind === 'failed') return failed(listing.code);
		const selectedWorktree = listing.worktrees.find((worktree) => worktree.id === input.worktreeId);
		if (selectedWorktree === undefined) return failed('worktree-not-found');

		let equivalentDirectory = project.directories.find(
			(directory) => directory.path === selectedWorktree.path,
		);
		if (equivalentDirectory === undefined) {
			for (const directory of project.directories) {
				if (!directory.available || directory.id === sourceDirectory.id) continue;
				const observation = await this.#dependencies.git.observeRepository(directory.path);
				if (
					observation.kind === 'observed' &&
					observation.repository.repositoryId === input.repositoryId &&
					observation.repository.worktreeId === selectedWorktree.id
				) {
					equivalentDirectory = directory;
					break;
				}
			}
		}

		let directoryId = `discovered-${selectedWorktree.id}`;
		let workspaceId = '';
		let mutation: Awaited<ReturnType<DesktopClientSessionCoordinator['mutateCanonicalDocument']>>;
		try {
			mutation = await this.#options.coordinator.mutateCanonicalDocument((canonical) => {
				const currentProject = canonical.projectWorkspace.projects.find(
					(candidate) => candidate.id === input.projectId,
				);
				const currentSource = currentProject?.directories.find(
					(directory) => directory.id === input.sourceDirectoryId,
				);
				if (
					currentProject === undefined ||
					currentSource === undefined ||
					!currentSource.available ||
					currentSource.path !== sourceDirectory.path
				) {
					return { kind: 'rejected', reason: 'source-unavailable' };
				}
				const exactEquivalent = currentProject.directories.find(
					(directory) => directory.path === selectedWorktree.path,
				);
				const previousEquivalent =
					equivalentDirectory === undefined
						? undefined
						: currentProject.directories.find(
								(directory) =>
									directory.id === equivalentDirectory.id &&
									directory.path === equivalentDirectory.path,
							);
				const currentEquivalent = exactEquivalent ?? previousEquivalent;
				if (currentEquivalent !== undefined && !currentEquivalent.available) {
					return { kind: 'rejected', reason: 'source-unavailable' };
				}
				const runtime = this.#documentRuntime(canonical);
				workspaceId = `workspace-${runtime.getSnapshot().nextWorkspaceSequence}`;
				directoryId = currentEquivalent?.id ?? directoryId;
				const transition =
					currentEquivalent === undefined
						? runtime.transition({
								directory: {
									available: true,
									git: {
										branch: selectedWorktree.branch,
										head: selectedWorktree.head,
										repositoryId: input.repositoryId,
										worktree: true,
									},
									id: directoryId,
									kind: 'worktree',
									name: worktreeLabel(selectedWorktree),
									path: selectedWorktree.path,
									placement: { kind: 'local' },
									source: 'discovered',
								},
								projectId: input.projectId,
								sourceDirectoryId: input.sourceDirectoryId,
								terminalTitle: this.#options.terminalTitle,
								type: 'project-worktree-adopted',
								workspaceName: worktreeLabel(selectedWorktree),
							})
						: runtime.transition({
								directoryId: currentEquivalent.id,
								launchConfigurationId: null,
								name: worktreeLabel(selectedWorktree),
								projectId: input.projectId,
								terminalTitle: this.#options.terminalTitle,
								type: 'workspace-created',
							});
				if (transition.outcome !== 'applied') {
					return { kind: 'rejected', reason: transition.rejection ?? 'document-conflict' };
				}
				return {
					document: runtime.getSharedDocument(
						workbenchSharedDocumentRevision((BigInt(canonical.revision) + 1n).toString()),
					),
					kind: 'commit',
				};
			});
		} catch {
			return failed('document-conflict');
		}
		if (mutation.kind !== 'committed') {
			return failed(mutation.kind === 'unavailable' ? 'control-unavailable' : 'document-conflict');
		}
		return { kind: 'completed', value: { directoryId, workspaceId } };
	}

	async #createOwned(input: {
		readonly baseRef: string;
		readonly branchName: string;
		readonly createWorkspace: boolean;
		readonly projectId: string;
		readonly repositoryId: string;
		readonly sourceDirectoryId: string;
	}): Promise<DesktopProjectWorktreeRuntimeResult<DesktopProjectWorktreeCreationResult>> {
		if (
			[...this.#operations.values()].some(
				(operation) => operation.repositoryId === input.repositoryId,
			)
		) {
			return failed('recovery-required');
		}
		const document = this.#options.coordinator.canonicalDocument;
		const project = document?.projectWorkspace.projects.find(
			(candidate) => candidate.id === input.projectId,
		);
		if (project === undefined) return failed('project-not-found');
		const sourceDirectory = project.directories.find(
			(directory) => directory.id === input.sourceDirectoryId,
		);
		if (sourceDirectory === undefined || !sourceDirectory.available) {
			return failed('source-unavailable');
		}
		const observation = await this.#dependencies.git.observeRepository(sourceDirectory.path);
		if (observation.kind === 'failed') return failed(observation.code);
		if (
			observation.kind !== 'observed' ||
			observation.repository.repositoryId !== input.repositoryId
		) {
			return failed('repository-not-found');
		}
		const validation = await this.#dependencies.git.validateCreation(
			sourceDirectory.path,
			input.baseRef,
			input.branchName,
		);
		if (validation.kind === 'failed') return failed(validation.code);
		const token = this.#options.allocateIdentifier();
		if (!/^[A-Za-z0-9_-]{8,64}$/u.test(token)) return failed('creation-failed');
		const directoryId = `managed-worktree-${token}`;
		const targetPath = path.join(this.#options.managedRoot, directoryId);
		if (!this.#isOwnedTarget(targetPath, directoryId)) return failed('target-conflict');
		try {
			await this.#dependencies.lstat(targetPath);
			return failed('target-conflict');
		} catch (error: unknown) {
			if (!isNodeError(error) || error.code !== 'ENOENT') return failed('target-conflict');
		}
		const operationId = `project-worktree-${token}`;
		let record: ProjectWorktreeOperationRecord = {
			baseCommit: validation.baseCommit,
			baseRef: input.baseRef,
			branchName: input.branchName,
			createWorkspace: input.createWorkspace,
			directoryId,
			operationId,
			phase: 'prepared',
			projectId: input.projectId,
			repositoryId: input.repositoryId,
			sourceDirectoryId: input.sourceDirectoryId,
			sourcePath: sourceDirectory.path,
			targetPath,
		};
		try {
			await this.#replaceOperation(record);
		} catch {
			return failed('creation-failed');
		}
		const gitCreation = await this.#dependencies.git.createWorktree(
			record.sourcePath,
			record.targetPath,
			record.branchName,
			record.baseRef,
		);
		if (gitCreation !== 'created') {
			return this.#compensateFailure(record, 'creation-failed');
		}
		record = { ...record, phase: 'git-created' };
		try {
			await this.#replaceOperation(record);
		} catch {
			return this.#compensateFailure(record, 'creation-failed');
		}
		let workspaceId: string | null = null;
		let mutation: Awaited<ReturnType<DesktopClientSessionCoordinator['mutateCanonicalDocument']>>;
		try {
			mutation = await this.#options.coordinator.mutateCanonicalDocument((canonical) => {
				const currentProject = canonical.projectWorkspace.projects.find(
					(candidate) => candidate.id === record.projectId,
				);
				const currentSource = currentProject?.directories.find(
					(directory) => directory.id === record.sourceDirectoryId,
				);
				if (
					currentProject === undefined ||
					currentSource === undefined ||
					!currentSource.available ||
					currentSource.path !== record.sourcePath
				) {
					return { kind: 'rejected', reason: 'source-unavailable' };
				}
				const runtime = this.#documentRuntime(canonical);
				if (record.createWorkspace) {
					workspaceId = `workspace-${runtime.getSnapshot().nextWorkspaceSequence}`;
				}
				const transition = runtime.transition({
					createWorkspace: record.createWorkspace,
					directory: {
						available: true,
						git: {
							branch: record.branchName,
							head: record.baseCommit,
							repositoryId: record.repositoryId,
							worktree: true,
						},
						id: record.directoryId,
						kind: 'worktree',
						name: record.branchName,
						path: record.targetPath,
						placement: { kind: 'local' },
						source: 'created',
					},
					projectId: record.projectId,
					sourceDirectoryId: record.sourceDirectoryId,
					terminalTitle: this.#options.terminalTitle,
					type: 'project-worktree-created',
					workspaceName: record.branchName,
				});
				if (transition.outcome !== 'applied') {
					return { kind: 'rejected', reason: transition.rejection ?? 'document-conflict' };
				}
				return {
					document: runtime.getSharedDocument(
						workbenchSharedDocumentRevision((BigInt(canonical.revision) + 1n).toString()),
					),
					kind: 'commit',
				};
			});
		} catch {
			return this.#compensateFailure(record, 'document-conflict');
		}
		if (mutation.kind !== 'committed') {
			return this.#compensateFailure(
				record,
				mutation.kind === 'unavailable' ? 'control-unavailable' : 'document-conflict',
			);
		}
		await this.#removeOperation(record.operationId).catch(() => undefined);
		return { kind: 'completed', value: { directoryId, workspaceId } };
	}

	#documentRuntime(
		document: NonNullable<DesktopClientSessionCoordinator['canonicalDocument']>,
	): WorkbenchProjectWorkspaceRuntime {
		return new WorkbenchProjectWorkspaceRuntime({
			initialDocument: document,
			standaloneCwd: this.#options.defaultCwd,
			standaloneTerminalTitle: this.#options.terminalTitle,
			terminalPresentation: {
				createOperationId: () =>
					terminalPresentationOperationId(
						`desktop:worktree:operation:${this.#options.allocateIdentifier()}`,
					),
				createPresentationId: () =>
					terminalPresentationId(
						`desktop:worktree:presentation:${this.#options.allocateIdentifier()}`,
					),
				createRemovalId: () =>
					terminalPresentationRemovalId(
						`desktop:worktree:removal:${this.#options.allocateIdentifier()}`,
					),
				dispatchCommands: () => undefined,
				initialPresentationId: terminalPresentationId(
					`desktop:worktree:initial:${this.#options.allocateIdentifier()}`,
				),
				kind: 'managed',
			},
		});
	}

	async #compensateFailure(
		record: ProjectWorktreeOperationRecord,
		failureCode: DesktopProjectWorktreeFailureCode,
	): Promise<DesktopProjectWorktreeRuntimeResult<never>> {
		const compensation = await this.#dependencies.git.compensateWorktree(
			record.sourcePath,
			record.targetPath,
			record.branchName,
			record.baseCommit,
			record.phase !== 'prepared',
		);
		if (compensation === 'compensated') {
			try {
				await this.#removeOperation(record.operationId);
			} catch {
				return failed('compensation-failed');
			}
			return failed(failureCode);
		}
		await this.#replaceOperation({ ...record, phase: 'recovery-required' }).catch(() => undefined);
		return failed('compensation-failed');
	}

	async #recoverPendingOperations(): Promise<void> {
		for (const record of this.#operations.values()) {
			const committed = this.#options.coordinator.canonicalDocument?.projectWorkspace.projects.some(
				(project) =>
					project.id === record.projectId &&
					project.directories.some(
						(directory) =>
							directory.id === record.directoryId && directory.path === record.targetPath,
					),
			);
			if (committed === true) {
				await this.#removeOperation(record.operationId).catch(() => undefined);
				continue;
			}
			const compensation = await this.#dependencies.git.compensateWorktree(
				record.sourcePath,
				record.targetPath,
				record.branchName,
				record.baseCommit,
				record.phase !== 'prepared',
			);
			if (compensation === 'compensated') {
				await this.#removeOperation(record.operationId).catch(() => undefined);
			} else if (record.phase !== 'recovery-required') {
				await this.#replaceOperation({ ...record, phase: 'recovery-required' }).catch(
					() => undefined,
				);
			}
		}
	}

	async #replaceOperation(record: ProjectWorktreeOperationRecord): Promise<void> {
		const existing = this.#operations.get(record.operationId);
		this.#operations.set(record.operationId, record);
		try {
			await this.#persistOperations();
		} catch (error: unknown) {
			if (existing === undefined) this.#operations.delete(record.operationId);
			else this.#operations.set(record.operationId, existing);
			throw error;
		}
	}

	async #removeOperation(operationId: string): Promise<void> {
		const existing = this.#operations.get(operationId);
		this.#operations.delete(operationId);
		try {
			await this.#persistOperations();
		} catch (error: unknown) {
			if (existing !== undefined) this.#operations.set(operationId, existing);
			throw error;
		}
	}

	#persistOperations(): Promise<void> {
		return this.#options.operationStore.replace([...this.#operations.values()]);
	}

	#isOwnedTarget(targetPath: string, directoryId: string): boolean {
		return (
			path.resolve(targetPath) === path.join(this.#options.managedRoot, directoryId) &&
			path.dirname(path.resolve(targetPath)) === this.#options.managedRoot &&
			directoryId.startsWith('managed-worktree-')
		);
	}

	#enqueueRepository<Result>(
		repositoryId: string,
		operation: () => Promise<Result>,
	): Promise<Result> {
		const previous = this.#repositoryTails.get(repositoryId) ?? Promise.resolve();
		const result = previous.then(operation);
		const tail = result.then(
			() => undefined,
			() => undefined,
		);
		this.#repositoryTails.set(repositoryId, tail);
		void tail.finally(() => {
			if (this.#repositoryTails.get(repositoryId) === tail) {
				this.#repositoryTails.delete(repositoryId);
			}
		});
		return result;
	}
}

function suggestedBranchName(localBaseRefs: readonly string[]): string {
	const existing = new Set(localBaseRefs);
	let sequence = 1;
	while (existing.has(`orchbee/workspace-${sequence}`)) sequence += 1;
	return `orchbee/workspace-${sequence}`;
}

function worktreeLabel(
	worktree: Pick<GitProjectWorktreeObservation, 'branch' | 'id' | 'path'>,
): string {
	return (worktree.branch ?? path.basename(worktree.path)) || worktree.id;
}

function failed(code: DesktopProjectWorktreeFailureCode) {
	return { code, kind: 'failed' } as const;
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
