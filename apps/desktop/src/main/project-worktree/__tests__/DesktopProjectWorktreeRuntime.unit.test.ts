import path from 'node:path';

import {
	WorkbenchProjectWorkspaceRuntime,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '@orchbee/workbench';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench/document';
import { describe, expect, it, vi } from 'vitest';

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
		const discovery = await fixture.runtime.discover('project-1', 'directory-1');
		expect(discovery).toEqual({
			kind: 'completed',
			value: {
				managedRoot: '/managed/worktrees',
				repositories: [
					{
						defaultBaseRef: 'main',
						label: 'repository',
						localBaseRefs: ['main', 'orchbee/workspace-1'],
						repositoryId: 'repository-1',
						sourceDirectoryId: 'directory-1',
						suggestedBranchName: 'orchbee/workspace-2',
					},
				],
			},
		});

		const creation = await fixture.runtime.create({
			baseRef: 'main',
			branchName: 'orchbee/workspace-2',
			createWorkspace: true,
			projectId: 'project-1',
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
			path.join('/managed/worktrees', `managed-worktree-${'1'.padStart(32, '0')}`),
			'orchbee/workspace-2',
			'main',
		);
		const project = fixture.coordinator.canonicalDocument?.projectWorkspace.projects[0];
		expect(project).toMatchObject({
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

	it('compensates the exact Git mutation when canonical document commit fails', async () => {
		const fixture = await createFixture({ failDocumentCommit: true });
		const original = fixture.coordinator.canonicalDocument;

		expect(
			await fixture.runtime.create({
				baseRef: 'main',
				branchName: 'orchbee/workspace-2',
				createWorkspace: true,
				projectId: 'project-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
			}),
		).toEqual({ code: 'control-unavailable', kind: 'failed' });
		expect(fixture.git.compensateWorktree).toHaveBeenCalledWith(
			'/projects/repository',
			expect.stringContaining('/managed/worktrees/managed-worktree-'),
			'orchbee/workspace-2',
			'abc123',
			true,
		);
		expect(fixture.coordinator.canonicalDocument).toBe(original);
		expect(fixture.operations).toEqual([]);
	});

	it('compensates Git when building the canonical mutation throws', async () => {
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
				projectId: 'project-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
			}),
		).toEqual({ code: 'document-conflict', kind: 'failed' });
		expect(fixture.git.compensateWorktree).toHaveBeenCalledOnce();
		expect(fixture.operations).toEqual([]);
	});

	it('retains failed recovery and blocks another mutation for the same repository', async () => {
		const pending: ProjectWorktreeOperationRecord = {
			baseCommit: 'abc123',
			baseRef: 'main',
			branchName: 'orchbee/workspace-2',
			createWorkspace: true,
			directoryId: 'managed-worktree-pending1234',
			operationId: 'project-worktree-pending1234',
			phase: 'git-created',
			projectId: 'project-1',
			repositoryId: 'repository-1',
			sourceDirectoryId: 'directory-1',
			sourcePath: '/projects/repository',
			targetPath: '/managed/worktrees/managed-worktree-pending1234',
		};
		const fixture = await createFixture({ pending, recoveryFails: true });

		expect(fixture.operations).toMatchObject([{ phase: 'recovery-required' }]);
		expect(
			await fixture.runtime.create({
				baseRef: 'main',
				branchName: 'orchbee/workspace-3',
				createWorkspace: true,
				projectId: 'project-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
			}),
		).toEqual({ code: 'recovery-required', kind: 'failed' });
	});
});

async function createFixture(
	options: {
		readonly allocateIdentifier?: () => string;
		readonly failDocumentCommit?: boolean;
		readonly pending?: ProjectWorktreeOperationRecord;
		readonly recoveryFails?: boolean;
	} = {},
) {
	const initialDocument = projectDocument();
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
		observeRepository: vi
			.fn<DesktopProjectWorktreeRuntimeDependencies['git']['observeRepository']>()
			.mockResolvedValue({
				kind: 'observed',
				repository: {
					branch: 'main',
					head: 'abc123',
					localBaseRefs: ['main', 'orchbee/workspace-1'],
					repositoryId: 'repository-1',
					repositoryPath: '/projects/repository/.git',
				},
			}),
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
			managedRoot: '/managed/worktrees',
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

function projectDocument() {
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
			source: 'selected',
		},
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: 'Main Workspace',
		projectName: 'Project',
		terminalTitle: 'Terminal',
		type: 'project-created',
	});
	return runtime.getSharedDocument(workbenchSharedDocumentRevision('1'));
}
