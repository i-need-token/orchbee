import path from 'node:path';

import { describe, expect, it, vi } from 'vitest';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench-client-protocol';
import { WorkbenchProjectWorkspaceRuntime } from '@orchbee/workbench/runtime';

import { DesktopClientSessionCoordinator } from '../../client-session';
import type { WorkbenchDocumentStore } from '../../workbench-document';
import {
	DesktopProjectWorktreeRuntime,
	type DesktopProjectWorktreeRuntimeDependencies,
} from '../DesktopProjectWorktreeRuntime';
import type {
	ProjectWorktreeOperationRecord,
	ProjectWorktreeOperationStore,
} from '../FileProjectWorktreeOperationStore';

describe('DesktopProjectWorktreeRuntime', () => {
	it('discovers Project repositories and commits Git plus Directory and Workspace', async () => {
		const fixture = await createFixture();
		const discovery = await fixture.runtime.discover('workspace-1', 'directory-1');
		expect(discovery).toEqual({
			kind: 'completed',
			value: {
				managedRoot: path.resolve('/managed/worktrees'),
				repositories: [
					{
						defaultBaseRef: 'main',
						label: 'repository',
						localBaseRefs: ['main', 'orchbee/workspace-1'],
						repositoryId: 'repository-1',
						sourceDirectoryId: 'directory-1',
						suggestedBranchName: 'orchbee/workspace-2',
						worktreeInventory: {
							kind: 'available',
							worktrees: [{ label: 'feature/external', worktreeId: 'worktree-external' }],
						},
					},
				],
			},
		});

		const creation = await fixture.runtime.create({
			baseRef: 'main',
			branchName: 'orchbee/workspace-2',
			createWorkspace: true,
			sourceWorkspaceId: 'workspace-1',
			repositoryId: 'repository-1',
			sourceDirectoryId: 'directory-1',
		});

		expect(creation).toEqual({
			kind: 'completed',
			value: {
				directoryId: `managed-worktree-${'1'.padStart(32, '0')}`,
				workspaceId: 'workspace-2',
			},
		});
		expect(fixture.git.createWorktree).toHaveBeenCalledWith(
			'/projects/repository',
			path.resolve('/managed/worktrees', `managed-worktree-${'1'.padStart(32, '0')}`),
			'orchbee/workspace-2',
			'main',
		);
		const graph = fixture.coordinator.canonicalDocument?.projectWorkspace;
		expect({
			directories: graph?.resourceScopes[0]?.directories,
			workspaces: graph?.workspaces,
		}).toMatchObject({
			directories: [
				{ id: 'directory-1' },
				{
					git: {
						branch: 'orchbee/workspace-2',
						repositoryId: 'repository-1',
						worktree: true,
					},
					kind: 'worktree',
					source: 'created',
				},
			],
			workspaces: [
				{ id: 'workspace-1' },
				{
					id: 'workspace-2',
					name: 'orchbee/workspace-2',
					primaryDirectoryId: `managed-worktree-${'1'.padStart(32, '0')}`,
				},
			],
		});
		expect(fixture.operations).toEqual([]);
		expect(fixture.git.compensateWorktree).not.toHaveBeenCalled();
	});

	it('revalidates and adopts an existing Worktree without mutating Git', async () => {
		const fixture = await createFixture();

		expect(
			await fixture.runtime.adopt({
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
				worktreeId: 'worktree-external',
			}),
		).toEqual({
			kind: 'completed',
			value: {
				directoryId: 'discovered-worktree-external',
				workspaceId: 'workspace-2',
			},
		});
		expect({
			directories:
				fixture.coordinator.canonicalDocument?.projectWorkspace.resourceScopes[0]?.directories,
			workspaces: fixture.coordinator.canonicalDocument?.projectWorkspace.workspaces,
		}).toMatchObject({
			directories: [
				{ id: 'directory-1' },
				{
					git: {
						branch: 'feature/external',
						head: 'def456',
						repositoryId: 'repository-1',
						worktree: true,
					},
					id: 'discovered-worktree-external',
					path: '/external/feature-external',
					source: 'discovered',
				},
			],
			workspaces: [
				{ id: 'workspace-1' },
				{
					id: 'workspace-2',
					name: 'feature/external',
					primaryDirectoryId: 'discovered-worktree-external',
				},
			],
		});
		expect(fixture.git.createWorktree).not.toHaveBeenCalled();
		expect(fixture.git.compensateWorktree).not.toHaveBeenCalled();
		expect(fixture.operations).toEqual([]);
	});

	it('rejects a stale Worktree identity without changing the canonical document', async () => {
		const fixture = await createFixture();
		const original = fixture.coordinator.canonicalDocument;

		expect(
			await fixture.runtime.adopt({
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
				worktreeId: 'worktree-stale',
			}),
		).toEqual({ code: 'worktree-not-found', kind: 'failed' });
		expect(fixture.coordinator.canonicalDocument).toBe(original);
		expect(fixture.git.createWorktree).not.toHaveBeenCalled();
		expect(fixture.operations).toEqual([]);
	});

	it('leaves Git and the canonical document unchanged when adoption commit fails', async () => {
		const fixture = await createFixture({ failDocumentCommit: true });
		const original = fixture.coordinator.canonicalDocument;

		expect(
			await fixture.runtime.adopt({
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
				worktreeId: 'worktree-external',
			}),
		).toEqual({ code: 'control-unavailable', kind: 'failed' });
		expect(fixture.coordinator.canonicalDocument).toBe(original);
		expect(fixture.git.createWorktree).not.toHaveBeenCalled();
		expect(fixture.git.compensateWorktree).not.toHaveBeenCalled();
		expect(fixture.operations).toEqual([]);
	});

	it('keeps existing Worktree inventories grouped by each represented repository', async () => {
		const fixture = await createFixture({ secondRepository: true });

		const discovery = await fixture.runtime.discover('workspace-1', 'directory-1');

		expect(discovery).toMatchObject({
			kind: 'completed',
			value: {
				repositories: [
					{
						label: 'repository',
						repositoryId: 'repository-1',
						worktreeInventory: {
							kind: 'available',
							worktrees: [{ label: 'feature/external' }],
						},
					},
					{
						label: 'ghostty',
						repositoryId: 'repository-2',
						worktreeInventory: {
							kind: 'available',
							worktrees: [{ label: 'feature/ghostty' }],
						},
					},
				],
			},
		});
	});

	it('reuses an equivalent Directory that became represented before adoption commits', async () => {
		const fixture = await createFixture({ existingExternalDirectory: true });

		expect(
			await fixture.runtime.adopt({
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
				worktreeId: 'worktree-external',
			}),
		).toEqual({
			kind: 'completed',
			value: { directoryId: 'existing-external', workspaceId: 'workspace-2' },
		});
		expect(
			fixture.coordinator.canonicalDocument?.projectWorkspace.resourceScopes[0]?.directories,
		).toHaveLength(2);
		expect(
			fixture.coordinator.canonicalDocument?.projectWorkspace.workspaces.filter(
				(workspace) =>
					workspace.projectId ===
					fixture.coordinator.canonicalDocument?.projectWorkspace.projects[0]?.id,
			)[1],
		).toMatchObject({ primaryDirectoryId: 'existing-external' });
	});

	it('retains the known Git Worktree when canonical document commit fails', async () => {
		const fixture = await createFixture({ failDocumentCommit: true });
		const original = fixture.coordinator.canonicalDocument;

		expect(
			await fixture.runtime.create({
				baseRef: 'main',
				branchName: 'orchbee/workspace-2',
				createWorkspace: true,
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
			}),
		).toEqual({ code: 'worktree-retained', kind: 'failed' });
		expect(fixture.git.compensateWorktree).not.toHaveBeenCalled();
		expect(fixture.coordinator.canonicalDocument).toBe(original);
		expect(fixture.operations).toMatchObject([{ phase: 'retained' }]);
	});

	it('retains the known Git Worktree when building the canonical mutation throws', async () => {
		const fixture = await createFixture({
			allocateIdentifier: (() => {
				let sequence = 0;
				return () => (++sequence === 1 ? '1'.padStart(32, '0') : 'invalid/identity');
			})(),
		});

		expect(
			await fixture.runtime.create({
				baseRef: 'main',
				branchName: 'orchbee/workspace-2',
				createWorkspace: true,
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
			}),
		).toEqual({ code: 'worktree-retained', kind: 'failed' });
		expect(fixture.git.compensateWorktree).not.toHaveBeenCalled();
		expect(fixture.operations).toMatchObject([{ phase: 'retained' }]);
	});

	it('retains known recovery results and allows other Worktrees in the repository', async () => {
		const pending: ProjectWorktreeOperationRecord = {
			baseCommit: 'abc123',
			baseRef: 'main',
			branchName: 'orchbee/workspace-2',
			createWorkspace: true,
			directoryId: 'managed-worktree-pending1234',
			operationId: 'project-worktree-pending1234',
			phase: 'git-created',
			sourceResourceScopeId: 'resource-scope-1',
			sourceTerminalContextId: 'terminal-context-2',
			sourceWorkspaceId: 'workspace-1',
			repositoryId: 'repository-1',
			sourceDirectoryId: 'directory-1',
			sourcePath: '/projects/repository',
			targetPath: '/managed/worktrees/managed-worktree-pending1234',
		};
		const fixture = await createFixture({ pending, recoveryFails: true });

		expect(fixture.operations).toMatchObject([{ phase: 'retained' }]);
		expect(
			await fixture.runtime.create({
				baseRef: 'main',
				branchName: 'orchbee/workspace-3',
				createWorkspace: true,
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
			}),
		).toMatchObject({ kind: 'completed', value: { workspaceId: 'workspace-2' } });
		expect(fixture.git.compensateWorktree).not.toHaveBeenCalled();
		expect(fixture.operations).toMatchObject([{ phase: 'retained' }]);
	});
	it('commits beside the source current group after async Git preparation without changing source resources', async () => {
		const fixture = await createFixture();
		await mutateFixture(fixture, {
			type: 'workspace-project-set',
			workspaceId: 'workspace-1',
			projectId: null,
		});
		const started = createDeferred();
		const finish = createDeferred();
		vi.mocked(fixture.git.createWorktree).mockImplementationOnce(async () => {
			started.resolve();
			await finish.promise;
			return 'created';
		});
		const pending = fixture.runtime.create({
			baseRef: 'main',
			branchName: 'feature/moved',
			createWorkspace: true,
			sourceWorkspaceId: 'workspace-1',
			repositoryId: 'repository-1',
			sourceDirectoryId: 'directory-1',
		});
		await started.promise;
		await mutateFixture(fixture, {
			type: 'workspace-project-created',
			workspaceId: 'workspace-1',
			name: 'Current group',
		});
		finish.resolve();
		expect(await pending).toMatchObject({
			kind: 'completed',
			value: { workspaceId: 'workspace-2' },
		});
		expect(
			fixture.coordinator.canonicalDocument?.projectWorkspace.workspaces.map((workspace) => [
				workspace.projectId,
				workspace.resourceScopeId,
			]),
		).toEqual([
			['project-2', 'resource-scope-1'],
			['project-2', 'resource-scope-1'],
		]);
		expect(fixture.git.compensateWorktree).not.toHaveBeenCalled();
		expect(fixture.operations).toEqual([]);
	});

	it('retains a created Worktree when its source closes and never resurrects the Workspace', async () => {
		const fixture = await createFixture();
		const started = createDeferred();
		const finish = createDeferred();
		vi.mocked(fixture.git.createWorktree).mockImplementationOnce(async () => {
			started.resolve();
			await finish.promise;
			return 'created';
		});
		const pending = fixture.runtime.create({
			baseRef: 'main',
			branchName: 'feature/closed',
			createWorkspace: true,
			sourceWorkspaceId: 'workspace-1',
			repositoryId: 'repository-1',
			sourceDirectoryId: 'directory-1',
		});
		await started.promise;
		await mutateFixture(fixture, { type: 'workspace-deleted', workspaceId: 'workspace-1' });
		const closed = fixture.coordinator.canonicalDocument;
		finish.resolve();
		expect(await pending).toEqual({ kind: 'failed', code: 'worktree-retained' });
		expect(fixture.coordinator.canonicalDocument).toBe(closed);
		expect(closed?.projectWorkspace.workspaces).toEqual([]);
		expect(fixture.operations).toMatchObject([
			{
				phase: 'retained',
				sourceWorkspaceId: 'workspace-1',
				sourceResourceScopeId: 'resource-scope-1',
			},
		]);
		expect(fixture.git.compensateWorktree).not.toHaveBeenCalled();
	});

	it('reopens an associated Worktree across groups without creating a second Workspace', async () => {
		const fixture = await createFixture();
		const input = {
			sourceWorkspaceId: 'workspace-1',
			repositoryId: 'repository-1',
			sourceDirectoryId: 'directory-1',
			worktreeId: 'worktree-external',
		};
		const first = await fixture.runtime.adopt(input);
		await mutateFixture(fixture, {
			type: 'workspace-project-created',
			workspaceId: 'workspace-2',
			name: 'Elsewhere',
		});
		const before = fixture.coordinator.canonicalDocument?.projectWorkspace;
		expect(await fixture.runtime.adopt(input)).toEqual(first);
		expect(fixture.coordinator.canonicalDocument?.projectWorkspace).toEqual(before);
		expect(fixture.coordinator.canonicalDocument?.projectWorkspace.workspaces).toHaveLength(2);
	});
});

async function createFixture(
	options: {
		readonly allocateIdentifier?: () => string;
		readonly existingExternalDirectory?: boolean;
		readonly failDocumentCommit?: boolean;
		readonly pending?: ProjectWorktreeOperationRecord;
		readonly recoveryFails?: boolean;
		readonly secondRepository?: boolean;
	} = {},
) {
	const initialDocument = projectDocument(
		options.secondRepository === true,
		options.existingExternalDirectory === true,
	);
	const store: WorkbenchDocumentStore = {
		commit: vi.fn<WorkbenchDocumentStore['commit']>(async (_expected, document) =>
			options.failDocumentCommit === true
				? { kind: 'unavailable' as const, reason: 'write-failed' as const }
				: { kind: 'committed' as const, revision: document.revision },
		),
		load: vi.fn<WorkbenchDocumentStore['load']>(async () => ({
			document: initialDocument,
			kind: 'available' as const,
		})),
	};
	const coordinator = new DesktopClientSessionCoordinator({
		createClientSessionId: () => 'client-1',
		defaultCwd: '/system',
		getTerminalEvents: () => [],
		initialPresentationId: terminalPresentationId('initial'),
		initializationDocument: initialDocument,
		reconcileDocument: async () => undefined,
		store,
		terminalHostName: 'host',
		validateDocument: () => true,
	});
	await coordinator.start();
	let operations: ProjectWorktreeOperationRecord[] =
		options.pending === undefined ? [] : [options.pending];
	const operationStore: ProjectWorktreeOperationStore = {
		load: vi.fn<ProjectWorktreeOperationStore['load']>(async () => ({
			kind: 'ready' as const,
			operations,
		})),
		replace: vi.fn<ProjectWorktreeOperationStore['replace']>(async (next) => {
			operations = [...next];
		}),
	};
	const git: DesktopProjectWorktreeRuntimeDependencies['git'] = {
		compensateWorktree: vi
			.fn<DesktopProjectWorktreeRuntimeDependencies['git']['compensateWorktree']>()
			.mockResolvedValue(options.recoveryFails === true ? 'failed' : 'compensated'),
		createWorktree: vi
			.fn<DesktopProjectWorktreeRuntimeDependencies['git']['createWorktree']>()
			.mockResolvedValue('created'),
		listWorktrees: vi
			.fn<DesktopProjectWorktreeRuntimeDependencies['git']['listWorktrees']>()
			.mockImplementation(async (directoryPath) => ({
				kind: 'listed',
				worktrees: [
					{
						branch: directoryPath.includes('ghostty') ? 'stable' : 'main',
						head: 'abc123',
						id: directoryPath.includes('ghostty') ? 'worktree-ghostty-main' : 'worktree-main',
						path: directoryPath,
					},
					{
						branch: directoryPath.includes('ghostty') ? 'feature/ghostty' : 'feature/external',
						head: 'def456',
						id: directoryPath.includes('ghostty')
							? 'worktree-ghostty-external'
							: 'worktree-external',
						path: directoryPath.includes('ghostty')
							? '/external/feature-ghostty'
							: '/external/feature-external',
					},
				],
			})),
		observeRepository: vi
			.fn<DesktopProjectWorktreeRuntimeDependencies['git']['observeRepository']>()
			.mockImplementation(async (directoryPath) => ({
				kind: 'observed',
				repository: {
					branch: directoryPath.includes('ghostty') ? 'stable' : 'main',
					head: 'abc123',
					localBaseRefs: directoryPath.includes('ghostty')
						? ['stable']
						: ['main', 'orchbee/workspace-1'],
					repositoryId: directoryPath.includes('ghostty') ? 'repository-2' : 'repository-1',
					repositoryPath: `${directoryPath}/.git`,
					worktreeId: directoryPath.includes('ghostty') ? 'worktree-ghostty-main' : 'worktree-main',
				},
			})),
		validateCreation: vi
			.fn<DesktopProjectWorktreeRuntimeDependencies['git']['validateCreation']>()
			.mockResolvedValue({ baseCommit: 'abc123', kind: 'ready' }),
	};
	let identifierSequence = 0;
	const runtime = new DesktopProjectWorktreeRuntime(
		{
			allocateIdentifier:
				options.allocateIdentifier ?? (() => `${++identifierSequence}`.padStart(32, '0')),
			coordinator,
			defaultCwd: '/system',
			managedRoot: path.resolve('/managed/worktrees'),
			operationStore,
			terminalTitle: 'Terminal',
		},
		{
			chmod: async () => undefined,
			git,
			lstat: async () => {
				throw Object.assign(new Error('missing'), { code: 'ENOENT' });
			},
			mkdir: async () => undefined,
		},
	);
	expect(await runtime.start()).toBe('ready');
	return {
		coordinator,
		git,
		get operations() {
			return operations;
		},
		runtime,
	};
}

function projectDocument(secondRepository = false, existingExternalDirectory = false) {
	let identity = 0;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Terminal',
		terminalPresentation: {
			createOperationId: () => terminalPresentationOperationId(`project-operation-${++identity}`),
			createPresentationId: () => terminalPresentationId(`project-presentation-${++identity}`),
			createRemovalId: () => terminalPresentationRemovalId(`project-removal-${++identity}`),
			dispatchCommands: () => undefined,
			initialPresentationId: terminalPresentationId('project-initial'),
			kind: 'managed',
		},
	});
	runtime.transition({
		directory: {
			available: true,
			git: null,
			id: 'directory-1',
			kind: 'folder',
			name: 'repository',
			path: '/projects/repository',
			placement: { kind: 'local' },
			source: 'selected',
		},
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: 'Main Workspace',
		projectName: 'Project',
		terminalTitle: 'Terminal',
		type: 'project-created',
	});
	if (secondRepository) {
		runtime.transition({
			directory: {
				available: true,
				git: null,
				id: 'directory-2',
				kind: 'folder',
				name: 'ghostty',
				path: '/projects/ghostty',
				placement: { kind: 'local' },
				source: 'selected',
			},
			resourceScopeId: 'resource-scope-1',
			type: 'directory-added',
		});
	}
	if (existingExternalDirectory) {
		runtime.transition({
			directory: {
				available: true,
				git: {
					branch: 'feature/external',
					head: 'def456',
					repositoryId: 'repository-1',
					worktree: true,
				},
				id: 'existing-external',
				kind: 'worktree',
				name: 'feature/external',
				path: '/external/feature-external',
				placement: { kind: 'local' },
				source: 'discovered',
			},
			resourceScopeId: 'resource-scope-1',
			type: 'directory-added',
		});
	}
	return runtime.getSharedDocument(workbenchSharedDocumentRevision('1'));
}

async function mutateFixture(
	fixture: Awaited<ReturnType<typeof createFixture>>,
	event: Parameters<WorkbenchProjectWorkspaceRuntime['transition']>[0],
) {
	let identity = 0;
	const result = await fixture.coordinator.mutateCanonicalDocument((canonical) => {
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			initialDocument: canonical,
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Shell',
			terminalPresentation: {
				kind: 'managed',
				initialPresentationId: terminalPresentationId('mutation-initial'),
				createPresentationId: () => terminalPresentationId(`mutation-${++identity}`),
				createOperationId: () => terminalPresentationOperationId(`mutation-${++identity}`),
				createRemovalId: () => terminalPresentationRemovalId(`mutation-${++identity}`),
				dispatchCommands: () => undefined,
			},
		});
		expect(runtime.transition(event).outcome).toBe('applied');
		return {
			kind: 'commit',
			document: runtime.getSharedDocument(
				workbenchSharedDocumentRevision((BigInt(canonical.revision) + 1n).toString()),
			),
		};
	});
	expect(result.kind).toBe('committed');
}

function createDeferred(): { readonly promise: Promise<void>; readonly resolve: () => void } {
	let resolve: () => void = () => undefined;
	const promise = new Promise<void>((nextResolve) => {
		resolve = nextResolve;
	});
	return { promise, resolve };
}
