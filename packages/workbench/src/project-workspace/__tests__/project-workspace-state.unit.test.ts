import { array, assert, integer, property, uniqueArray } from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	createProjectWorkspaceState,
	transitionProjectWorkspace,
	type ProjectWorkspaceState,
	type WorkbenchDirectory,
	type WorkbenchFolderDirectory,
	type WorkbenchWorktreeDirectory,
} from '../project-workspace-state';

function directory(id: string, kind: 'worktree'): WorkbenchWorktreeDirectory;
function directory(id: string, kind?: 'folder'): WorkbenchFolderDirectory;
function directory(id: string, kind: WorkbenchDirectory['kind'] = 'folder'): WorkbenchDirectory {
	const base = {
		available: true,
		id,
		kind,
		name: id,
		path: `/projects/${id}`,
		placement: { kind: 'local' as const },
		source: kind === 'worktree' ? ('discovered' as const) : ('selected' as const),
	};
	return kind === 'worktree'
		? {
				...base,
				git: {
					branch: id,
					head: `${id}-head`,
					repositoryId: 'repository-1',
					worktree: true,
				},
				kind,
			}
		: { ...base, git: null, kind };
}

function repositoryDirectory(id: string): WorkbenchFolderDirectory {
	return {
		available: true,
		git: {
			branch: 'main',
			head: 'abc123',
			repositoryId: 'repository-1',
			worktree: false,
		},
		id,
		kind: 'folder',
		name: id,
		path: `/projects/${id}`,
		placement: { kind: 'local' },
		source: 'selected',
	};
}

function remoteDirectory(
	id: string,
	environmentId = 'environment:00000000000000000000000000000001',
	path = '/srv/orchbee',
): WorkbenchFolderDirectory {
	return {
		available: false,
		git: null,
		id,
		kind: 'folder',
		name: 'orchbee',
		path,
		placement: { environmentId, kind: 'environment' },
		source: 'selected',
	};
}

function createProject(
	state: ProjectWorkspaceState,
	primaryDirectory: WorkbenchDirectory = directory('directory-a'),
) {
	return transitionProjectWorkspace(state, {
		directory: primaryDirectory,
		launchConfigurations: [],
		primaryWorkspaceName: 'Primary',
		primaryWorkspaceLaunchConfigurationId: null,
		projectName: 'Project',
		terminalTitle: 'Terminal 1',
		type: 'project-created',
	}).state;
}

function createProjects(count: number): ProjectWorkspaceState {
	let state = createProjectWorkspaceState();
	for (let index = 1; index <= count; index += 1) {
		state = createProject(state, directory(`directory-${index}`));
	}
	return state;
}

function createWorkspaces(count: number): ProjectWorkspaceState {
	let state = createProject(createProjectWorkspaceState());
	for (let index = 2; index <= count; index += 1) {
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-a',
			launchConfigurationId: null,
			name: `Workspace ${index}`,
			resourceScopeId: 'resource-scope-1',
			projectId: 'project-1',
			terminalTitle: `Terminal ${index}`,
			type: 'workspace-created',
		}).state;
	}
	return state;
}

describe('project-workspace-state', () => {
	it('registers an idempotent remote Project without Workspace or terminal effect', () => {
		const initial = createProjectWorkspaceState();
		const created = transitionProjectWorkspace(initial, {
			directory: remoteDirectory('environment-directory:00000000000000000000000000000001'),
			projectName: 'orchbee',
			type: 'environment-project-registered',
		});

		expect(created).toMatchObject({
			commands: [],
			outcome: 'applied',
			state: {
				activeLocation: { kind: 'standalone' },
				nextProjectSequence: 2,
				nextTerminalContextSequence: 2,
				nextWorkspaceSequence: 1,
				projects: [{ id: 'project-1' }],
				resourceScopes: [
					{
						id: 'resource-scope-1',
						primaryDirectoryId: 'environment-directory:00000000000000000000000000000001',
					},
				],
				workspaces: [],
			},
		});

		const duplicate = transitionProjectWorkspace(created.state, {
			directory: remoteDirectory('environment-directory:ffffffffffffffffffffffffffffffff'),
			projectName: 'renamed',
			type: 'environment-project-registered',
		});
		expect(duplicate).toEqual({
			commands: [],
			outcome: 'unchanged',
			rejection: null,
			state: created.state,
		});

		const otherEnvironment = transitionProjectWorkspace(created.state, {
			directory: remoteDirectory(
				'environment-directory:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
				'environment:00000000000000000000000000000002',
			),
			projectName: 'orchbee',
			type: 'environment-project-registered',
		});
		expect(otherEnvironment.state.projects).toHaveLength(2);
	});

	it('attaches same-Environment remote folders to an existing Project without other effects', () => {
		const initial = createProject(createProjectWorkspaceState());
		const firstDirectory = remoteDirectory(
			'environment-directory:00000000000000000000000000000001',
		);
		const first = transitionProjectWorkspace(initial, {
			directory: firstDirectory,
			resourceScopeId: 'resource-scope-1',
			type: 'environment-directory-registered',
		});

		expect(first).toMatchObject({ commands: [], outcome: 'applied', rejection: null });
		expect(first.state).toEqual({
			...initial,
			projects: [{ ...initial.projects[0] }],
			resourceScopes: [
				{
					...initial.resourceScopes[0],
					directories: [...(initial.resourceScopes[0]?.directories ?? []), firstDirectory],
				},
			],
		});

		const duplicate = transitionProjectWorkspace(first.state, {
			directory: remoteDirectory('environment-directory:ffffffffffffffffffffffffffffffff'),
			resourceScopeId: 'resource-scope-1',
			type: 'environment-directory-registered',
		});
		expect(duplicate).toEqual({
			commands: [],
			outcome: 'unchanged',
			rejection: null,
			state: first.state,
		});

		const secondDirectory = remoteDirectory(
			'environment-directory:00000000000000000000000000000002',
			undefined,
			'/srv/second',
		);
		const second = transitionProjectWorkspace(first.state, {
			directory: secondDirectory,
			resourceScopeId: 'resource-scope-1',
			type: 'environment-directory-registered',
		});
		expect(second).toMatchObject({ commands: [], outcome: 'applied', rejection: null });
		expect(second.state.resourceScopes[0]?.directories).toEqual([
			...(first.state.resourceScopes[0]?.directories ?? []),
			secondDirectory,
		]);
	});

	it('rejects cross-Project remote locations and mixed Environment ownership', () => {
		let state = createProjects(2);
		const firstDirectory = remoteDirectory(
			'environment-directory:00000000000000000000000000000001',
		);
		state = transitionProjectWorkspace(state, {
			directory: firstDirectory,
			resourceScopeId: 'resource-scope-1',
			type: 'environment-directory-registered',
		}).state;

		expect(
			transitionProjectWorkspace(state, {
				directory: remoteDirectory('environment-directory:00000000000000000000000000000002'),
				resourceScopeId: 'resource-scope-2',
				type: 'environment-directory-registered',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'directory-already-exists', state });

		expect(
			transitionProjectWorkspace(state, {
				directory: remoteDirectory(
					'environment-directory:00000000000000000000000000000003',
					'environment:00000000000000000000000000000002',
					'/srv/second',
				),
				resourceScopeId: 'resource-scope-1',
				type: 'environment-directory-registered',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'directory-environment-conflict', state });
	});

	it('rejects forged local or available Environment folder attachment', () => {
		const state = createProject(createProjectWorkspaceState());
		const remote = remoteDirectory('environment-directory:00000000000000000000000000000001');

		for (const candidateDirectory of [
			{ ...remote, available: true },
			{ ...remote, placement: { kind: 'local' as const } },
		]) {
			expect(
				transitionProjectWorkspace(state, {
					directory: candidateDirectory,
					resourceScopeId: 'resource-scope-1',
					type: 'environment-directory-registered',
				}),
			).toMatchObject({ outcome: 'rejected', rejection: 'directory-invalid', state });
		}
	});

	it('registers one remote Workspace with an empty terminal context command', () => {
		const remote = remoteDirectory('environment-directory:00000000000000000000000000000001');
		const projectRegistered = transitionProjectWorkspace(createProjectWorkspaceState(), {
			directory: remote,
			projectName: 'orchbee',
			type: 'environment-project-registered',
		});
		const workspaceRegistered = transitionProjectWorkspace(projectRegistered.state, {
			directoryId: remote.id,
			resourceScopeId: 'resource-scope-1',
			projectId: 'project-1',
			type: 'environment-workspace-registered',
			workspaceName: 'orchbee',
		});

		expect(workspaceRegistered).toMatchObject({
			commands: [
				{
					terminalContextId: 'terminal-context-2',
					type: 'terminal-context-created-empty',
				},
			],
			outcome: 'applied',
			state: {
				activeLocation: { kind: 'standalone' },
				nextTerminalContextSequence: 3,
				nextWorkspaceSequence: 2,
				workspaces: [
					{
						id: 'workspace-1',
						launchConfigurationId: null,
						name: 'orchbee',
						primaryDirectoryId: remote.id,
						terminalContextId: 'terminal-context-2',
					},
				],
			},
		});
		expect(
			transitionProjectWorkspace(workspaceRegistered.state, {
				directoryId: remote.id,
				resourceScopeId: 'resource-scope-1',
				projectId: 'project-1',
				type: 'environment-workspace-registered',
				workspaceName: 'renamed',
			}),
		).toEqual({
			commands: [],
			outcome: 'unchanged',
			rejection: null,
			state: workspaceRegistered.state,
		});
	});

	it('rejects remote Workspace registration against unknown or local placement', () => {
		const local = createProject(createProjectWorkspaceState());
		expect(
			transitionProjectWorkspace(local, {
				directoryId: 'directory-a',
				resourceScopeId: 'resource-scope-1',
				projectId: 'project-1',
				type: 'environment-workspace-registered',
				workspaceName: 'forged',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'directory-unavailable' });
		expect(
			transitionProjectWorkspace(local, {
				directoryId: 'missing',
				resourceScopeId: 'resource-scope-1',
				projectId: 'project-1',
				type: 'environment-workspace-registered',
				workspaceName: 'missing',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'directory-not-found' });
	});

	it('converges generated repeated remote registrations by Environment and canonical path', () => {
		assert(
			property(
				array(integer({ min: 0, max: 63 }), { minLength: 1, maxLength: 64 }),
				(admissions) => {
					let state = createProjectWorkspaceState();
					const admittedKeys = new Set<string>();
					for (const [index, value] of admissions.entries()) {
						const environmentSequence = value % 4;
						const pathSequence = Math.floor(value / 4) % 4;
						const environmentId = `environment:${environmentSequence.toString(16).padStart(32, '0')}`;
						const path = `/srv/project-${pathSequence}`;
						const key = JSON.stringify([environmentId, path]);
						const alreadyAdmitted = admittedKeys.has(key);
						const result = transitionProjectWorkspace(state, {
							directory: remoteDirectory(
								`environment-directory:${(index + 1).toString(16).padStart(32, '0')}`,
								environmentId,
								path,
							),
							projectName: `project-${pathSequence}`,
							type: 'environment-project-registered',
						});

						expect(result.commands).toEqual([]);
						expect(result.outcome).toBe(alreadyAdmitted ? 'unchanged' : 'applied');
						if (!alreadyAdmitted) admittedKeys.add(key);
						state = result.state;
					}

					expect(state.projects).toHaveLength(admittedKeys.size);
					expect(state.nextProjectSequence).toBe(admittedKeys.size + 1);
					expect(state.nextTerminalContextSequence).toBe(2);
					expect(state.nextWorkspaceSequence).toBe(1);
					expect(state.workspaces.length === 0).toBe(true);
				},
			),
			{ numRuns: 200, seed: 20_260_828 },
		);
	});

	it('keeps remote placement unavailable and outside local Workspace transitions', () => {
		const remote = remoteDirectory('environment-directory:00000000000000000000000000000001');
		const registered = transitionProjectWorkspace(createProjectWorkspaceState(), {
			directory: remote,
			projectName: 'orchbee',
			type: 'environment-project-registered',
		}).state;

		expect(
			transitionProjectWorkspace(registered, {
				directoryId: remote.id,
				launchConfigurationId: null,
				name: 'Remote workspace',
				resourceScopeId: 'resource-scope-1',
				projectId: 'project-1',
				terminalTitle: 'Terminal',
				type: 'workspace-created',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'directory-unavailable' });
		expect(
			transitionProjectWorkspace(registered, {
				directory: { ...remote, placement: { kind: 'local' } },
				resourceScopeId: 'resource-scope-1',
				type: 'directory-updated',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'directory-update-not-allowed' });
		expect(
			transitionProjectWorkspace(createProjectWorkspaceState(), {
				directory: { ...remote, available: true },
				projectName: 'orchbee',
				type: 'environment-project-registered',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'directory-invalid' });
	});

	it('adds a managed Worktree Directory and optional Workspace atomically', () => {
		const sourceDirectory = repositoryDirectory('directory-a');
		const state = createProject(createProjectWorkspaceState(), sourceDirectory);
		const result = transitionProjectWorkspace(state, {
			createWorkspace: true,
			directory: {
				available: true,
				git: {
					branch: 'orchbee/workspace-1',
					head: 'abc123',
					repositoryId: 'repository-1',
					worktree: true,
				},
				id: 'managed-worktree-1',
				kind: 'worktree',
				name: 'orchbee/workspace-1',
				path: '/managed/worktrees/managed-worktree-1',
				placement: { kind: 'local' },
				source: 'created',
			},
			sourceWorkspaceId: 'workspace-1',
			projectName: 'Repository',
			sourceDirectoryId: 'directory-a',
			terminalTitle: 'Terminal 2',
			type: 'project-worktree-created',
			workspaceName: 'orchbee/workspace-1',
		});

		expect(result).toMatchObject({
			commands: [
				{
					cwd: '/managed/worktrees/managed-worktree-1',
					terminalContextId: 'terminal-context-3',
					terminalTitle: 'Terminal 2',
					type: 'terminal-context-created',
				},
			],
			outcome: 'applied',
		});
		expect(result.state.resourceScopes[0]).toMatchObject({
			directories: [{ id: 'directory-a' }, { id: 'managed-worktree-1', source: 'created' }],
		});
		expect(
			result.state.workspaces.filter(
				(workspace) => workspace.projectId === result.state.projects[0]?.id,
			),
		).toMatchObject([
			{ id: 'workspace-1' },
			{
				id: 'workspace-2',
				name: 'orchbee/workspace-1',
				primaryDirectoryId: 'managed-worktree-1',
				terminalContextId: 'terminal-context-3',
			},
		]);
	});

	it('can register a managed Worktree Directory without creating a Workspace', () => {
		const sourceDirectory = repositoryDirectory('directory-a');
		const state = createProject(createProjectWorkspaceState(), sourceDirectory);
		const result = transitionProjectWorkspace(state, {
			createWorkspace: false,
			directory: {
				available: true,
				git: {
					branch: 'orchbee/workspace-1',
					head: 'abc123',
					repositoryId: 'repository-1',
					worktree: true,
				},
				id: 'managed-worktree-1',
				kind: 'worktree',
				name: 'orchbee/workspace-1',
				path: '/managed/worktrees/managed-worktree-1',
				placement: { kind: 'local' },
				source: 'created',
			},
			sourceWorkspaceId: 'workspace-1',
			projectName: 'Repository',
			sourceDirectoryId: 'directory-a',
			terminalTitle: 'Terminal 2',
			type: 'project-worktree-created',
			workspaceName: 'orchbee/workspace-1',
		});

		expect(result).toMatchObject({ commands: [], outcome: 'applied' });
		expect(result.state.resourceScopes[0]?.directories).toHaveLength(2);
		expect(
			result.state.workspaces.filter(
				(workspace) => workspace.projectId === result.state.projects[0]?.id,
			),
		).toHaveLength(1);
		expect(result.state.nextTerminalContextSequence).toBe(state.nextTerminalContextSequence);
		expect(result.state.nextWorkspaceSequence).toBe(state.nextWorkspaceSequence);
		expect(result.state.activeLocation).toBe(state.activeLocation);
	});

	it('rejects a managed Worktree that does not belong to the selected source repository', () => {
		const sourceDirectory = repositoryDirectory('directory-a');
		const state = createProject(createProjectWorkspaceState(), sourceDirectory);
		const result = transitionProjectWorkspace(state, {
			createWorkspace: true,
			directory: {
				available: true,
				git: {
					branch: 'orchbee/workspace-1',
					head: 'abc123',
					repositoryId: 'repository-2',
					worktree: true,
				},
				id: 'managed-worktree-1',
				kind: 'worktree',
				name: 'orchbee/workspace-1',
				path: '/managed/worktrees/managed-worktree-1',
				placement: { kind: 'local' },
				source: 'created',
			},
			sourceWorkspaceId: 'workspace-1',
			projectName: 'Repository',
			sourceDirectoryId: 'directory-a',
			terminalTitle: 'Terminal 2',
			type: 'project-worktree-created',
			workspaceName: 'orchbee/workspace-1',
		});

		expect(result).toEqual({
			commands: [],
			outcome: 'rejected',
			rejection: 'directory-invalid',
			state,
		});
	});

	it('adopts a detached existing Worktree and creates its Workspace atomically', () => {
		const sourceDirectory = repositoryDirectory('directory-a');
		const state = createProject(createProjectWorkspaceState(), sourceDirectory);
		const result = transitionProjectWorkspace(state, {
			directory: {
				available: true,
				git: {
					branch: null,
					head: 'def456',
					repositoryId: 'repository-1',
					worktree: true,
				},
				id: 'discovered-worktree-1',
				kind: 'worktree',
				name: 'detached-checkout',
				path: '/external/detached-checkout',
				placement: { kind: 'local' },
				source: 'discovered',
			},
			sourceWorkspaceId: 'workspace-1',
			projectName: 'Repository',
			sourceDirectoryId: 'directory-a',
			terminalTitle: 'Terminal 2',
			type: 'project-worktree-adopted',
			workspaceName: 'detached-checkout',
		});

		expect(result).toMatchObject({
			commands: [
				{
					cwd: '/external/detached-checkout',
					terminalContextId: 'terminal-context-3',
					terminalTitle: 'Terminal 2',
					type: 'terminal-context-created',
				},
			],
			outcome: 'applied',
		});
		expect(result.state).toMatchObject({
			activeLocation: {
				kind: 'workspace',

				workspaceId: 'workspace-1',
			},
			resourceScopes: [
				{
					directories: [
						{ id: 'directory-a' },
						{ id: 'discovered-worktree-1', source: 'discovered' },
					],
				},
			],
			workspaces: [
				{ id: 'workspace-1' },
				{
					id: 'workspace-2',
					name: 'detached-checkout',
					primaryDirectoryId: 'discovered-worktree-1',
				},
			],
		});
	});

	it('rejects an adopted Worktree with a created source or another repository', () => {
		const sourceDirectory = repositoryDirectory('directory-a');
		const state = createProject(createProjectWorkspaceState(), sourceDirectory);
		for (const candidateDirectory of [
			{
				...directory('discovered-worktree-1', 'worktree'),
				source: 'created' as const,
			},
			{
				...directory('discovered-worktree-2', 'worktree'),
				git: {
					branch: 'feature/two',
					head: 'def456',
					repositoryId: 'repository-2',
					worktree: true as const,
				},
			},
		]) {
			expect(
				transitionProjectWorkspace(state, {
					directory: candidateDirectory,
					sourceWorkspaceId: 'workspace-1',
					projectName: 'Repository',
					sourceDirectoryId: 'directory-a',
					terminalTitle: 'Terminal 2',
					type: 'project-worktree-adopted',
					workspaceName: candidateDirectory.name,
				}),
			).toEqual({
				commands: [],
				outcome: 'rejected',
				rejection: 'directory-invalid',
				state,
			});
		}
	});

	it('creates a Project, Primary Workspace, active context, and shell command atomically', () => {
		const initial = createProjectWorkspaceState();
		const result = transitionProjectWorkspace(initial, {
			directory: directory('directory-a'),
			launchConfigurations: [],
			primaryWorkspaceName: 'Primary',
			primaryWorkspaceLaunchConfigurationId: null,
			projectName: 'alpha',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});

		expect(result).toMatchObject({
			commands: [
				{
					cwd: '/projects/directory-a',
					terminalContextId: 'terminal-context-2',
					terminalTitle: 'Terminal 1',
					type: 'terminal-context-created',
				},
			],
			outcome: 'applied',
			rejection: null,
		});
		expect(result.state.activeLocation).toEqual({
			kind: 'workspace',

			workspaceId: 'workspace-1',
		});
		expect(result.state.projects[0]).toMatchObject({ id: 'project-1' });
		expect(result.state.resourceScopes[0]).toMatchObject({
			launchConfigurations: [],
			primaryDirectoryId: 'directory-a',
		});
		expect(
			result.state.workspaces.filter(
				(workspace) => workspace.projectId === result.state.projects[0]?.id,
			),
		).toMatchObject([
			{
				id: 'workspace-1',
				launchConfigurationId: null,
				name: 'Primary',
				primaryDirectoryId: 'directory-a',
				terminalContextId: 'terminal-context-2',
			},
		]);
	});

	it('rejects Project creation from an unavailable Directory without consuming identities', () => {
		const initial = createProjectWorkspaceState();
		const result = transitionProjectWorkspace(initial, {
			directory: { ...directory('directory-a'), available: false },
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'alpha',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});

		expect(result).toEqual({
			commands: [],
			outcome: 'rejected',
			rejection: 'directory-unavailable',
			state: initial,
		});
	});

	it('rejects a missing initial launch configuration without consuming identities', () => {
		const initial = createProjectWorkspaceState();
		const result = transitionProjectWorkspace(initial, {
			directory: directory('directory-a'),
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: 'launch-missing',
			primaryWorkspaceName: 'Primary',
			projectName: 'alpha',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});

		expect(result).toEqual({
			commands: [],
			outcome: 'rejected',
			rejection: 'launch-configuration-not-found',
			state: initial,
		});
	});

	it('toggles Project disclosure without changing the active location', () => {
		const state = createProject(createProjectWorkspaceState());
		const result = transitionProjectWorkspace(state, {
			projectId: 'project-1',
			type: 'project-expansion-toggled',
		});

		expect(result.state.expandedProjectIds).toEqual([]);
		expect(result.state.activeLocation).toBe(state.activeLocation);
		expect(result.commands).toEqual([]);
	});

	it('reorders Projects before a stable identity without changing active or disclosure state', () => {
		const state = createProjects(3);
		const result = transitionProjectWorkspace(state, {
			beforeProjectId: 'project-1',
			projectId: 'project-3',
			type: 'project-reordered',
		});

		expect(result.outcome).toBe('applied');
		expect(result.state.projects.map((project) => project.id)).toEqual([
			'project-3',
			'project-1',
			'project-2',
		]);
		expect(result.state.activeLocation).toBe(state.activeLocation);
		expect(result.state.expandedProjectIds).toBe(state.expandedProjectIds);
		expect(result.commands).toEqual([]);
	});

	it('moves a Project to the end and leaves self or already-satisfied placement unchanged', () => {
		const state = createProjects(3);
		const moved = transitionProjectWorkspace(state, {
			beforeProjectId: null,
			projectId: 'project-1',
			type: 'project-reordered',
		});
		expect(moved.state.projects.map((project) => project.id)).toEqual([
			'project-2',
			'project-3',
			'project-1',
		]);

		for (const event of [
			{
				beforeProjectId: 'project-1',
				projectId: 'project-1',
				type: 'project-reordered' as const,
			},
			{
				beforeProjectId: 'project-2',
				projectId: 'project-1',
				type: 'project-reordered' as const,
			},
		]) {
			expect(transitionProjectWorkspace(state, event)).toEqual({
				commands: [],
				outcome: 'unchanged',
				rejection: null,
				state,
			});
		}
	});

	it('rejects stale Project reorder identities without partial mutation', () => {
		const state = createProjects(2);
		for (const event of [
			{
				beforeProjectId: 'project-1',
				projectId: 'project-missing',
				type: 'project-reordered' as const,
			},
			{
				beforeProjectId: 'project-missing',
				projectId: 'project-1',
				type: 'project-reordered' as const,
			},
		]) {
			expect(transitionProjectWorkspace(state, event)).toEqual({
				commands: [],
				outcome: 'rejected',
				rejection: 'project-not-found',
				state,
			});
		}
	});

	it('preserves the complete Project identity permutation for generated reorder positions', () => {
		assert(
			property(
				integer({ min: 1, max: 8 }),
				integer({ min: 0, max: 40 }),
				integer({ min: 0, max: 40 }),
				(projectCount, sourceSeed, targetSeed) => {
					const state = createProjects(projectCount);
					const sourceIndex = sourceSeed % projectCount;
					const targetSlot = targetSeed % (projectCount + 1);
					const sourceProject = state.projects[sourceIndex];
					if (sourceProject === undefined) throw new Error('Generated source Project is missing.');
					const beforeProjectId =
						targetSlot === projectCount ? null : (state.projects[targetSlot]?.id ?? null);
					const result = transitionProjectWorkspace(state, {
						beforeProjectId,
						projectId: sourceProject.id,
						type: 'project-reordered',
					});
					const beforeIds = state.projects.map((project) => project.id);
					const afterIds = result.state.projects.map((project) => project.id);

					expect([...afterIds].sort()).toEqual([...beforeIds].sort());
					expect(new Set(afterIds).size).toBe(projectCount);
					expect(result.state.activeLocation).toBe(state.activeLocation);
					expect(result.state.expandedProjectIds).toBe(state.expandedProjectIds);
				},
			),
			{ numRuns: 200, seed: 20_260_731 },
		);
	});

	it('reorders Workspaces only inside their owning Project', () => {
		const state = createWorkspaces(3);
		const project = state.resourceScopes[0];
		if (project === undefined) throw new Error('Expected the owning Project.');
		const result = transitionProjectWorkspace(state, {
			beforeWorkspaceId: 'workspace-1',

			type: 'workspace-reordered',
			workspaceId: 'workspace-3',
		});

		expect(result.outcome).toBe('applied');
		expect(
			result.state.workspaces
				.filter((workspace) => workspace.projectId === result.state.projects[0]?.id)
				.map((workspace) => workspace.id),
		).toEqual(['workspace-3', 'workspace-1', 'workspace-2']);
		expect(result.state.resourceScopes[0]?.directories).toBe(project.directories);
		expect(result.state.resourceScopes[0]?.launchConfigurations).toBe(project.launchConfigurations);
		expect(result.state.activeLocation).toBe(state.activeLocation);
		expect(result.state.expandedProjectIds).toBe(state.expandedProjectIds);
		expect(result.commands).toEqual([]);
	});

	it('moves a Workspace to the end and leaves self or satisfied placement unchanged', () => {
		const state = createWorkspaces(3);
		const moved = transitionProjectWorkspace(state, {
			beforeWorkspaceId: null,

			type: 'workspace-reordered',
			workspaceId: 'workspace-1',
		});
		expect(
			moved.state.workspaces
				.filter((workspace) => workspace.projectId === moved.state.projects[0]?.id)
				.map((workspace) => workspace.id),
		).toEqual(['workspace-2', 'workspace-3', 'workspace-1']);

		for (const event of [
			{
				beforeWorkspaceId: 'workspace-1',

				type: 'workspace-reordered' as const,
				workspaceId: 'workspace-1',
			},
			{
				beforeWorkspaceId: 'workspace-2',

				type: 'workspace-reordered' as const,
				workspaceId: 'workspace-1',
			},
		]) {
			expect(transitionProjectWorkspace(state, event)).toEqual({
				commands: [],
				outcome: 'unchanged',
				rejection: null,
				state,
			});
		}
	});

	it('rejects stale or cross-Project Workspace reorder identities without mutation', () => {
		let state = createWorkspaces(2);
		state = createProject(state, directory('directory-b'));
		for (const [event, rejection] of [
			[
				{
					beforeWorkspaceId: 'workspace-1',

					type: 'workspace-reordered' as const,
					workspaceId: 'workspace-missing',
				},
				'workspace-not-found',
			],
			[
				{
					beforeWorkspaceId: 'workspace-3',

					type: 'workspace-reordered' as const,
					workspaceId: 'workspace-1',
				},
				'workspace-not-found',
			],
		] as const) {
			expect(transitionProjectWorkspace(state, event)).toEqual({
				commands: [],
				outcome: 'rejected',
				rejection,
				state,
			});
		}
	});

	it('preserves each generated Workspace identity permutation and active terminal ownership', () => {
		assert(
			property(
				integer({ min: 1, max: 8 }),
				integer({ min: 0, max: 40 }),
				integer({ min: 0, max: 40 }),
				(workspaceCount, sourceSeed, targetSeed) => {
					const state = createWorkspaces(workspaceCount);
					const project = state.resourceScopes[0];
					if (project === undefined) throw new Error('Generated Project is missing.');
					const sourceIndex = sourceSeed % workspaceCount;
					const targetSlot = targetSeed % (workspaceCount + 1);
					const sourceWorkspace = state.workspaces[sourceIndex];
					if (sourceWorkspace === undefined) {
						throw new Error('Generated source Workspace is missing.');
					}
					const beforeWorkspaceId =
						targetSlot === workspaceCount ? null : (state.workspaces[targetSlot]?.id ?? null);
					const result = transitionProjectWorkspace(state, {
						beforeWorkspaceId,

						type: 'workspace-reordered',
						workspaceId: sourceWorkspace.id,
					});
					const beforeIds = state.workspaces.map((workspace) => workspace.id);
					const afterWorkspaces =
						result.state.workspaces.filter(
							(workspace) => workspace.projectId === result.state.projects[0]?.id,
						) ?? [];
					const afterIds = afterWorkspaces.map((workspace) => workspace.id);

					expect([...afterIds].sort()).toEqual([...beforeIds].sort());
					expect(new Set(afterIds).size).toBe(workspaceCount);
					expect(afterWorkspaces.map((workspace) => workspace.terminalContextId).sort()).toEqual(
						state.workspaces.map((workspace) => workspace.terminalContextId).sort(),
					);
					expect(result.state.activeLocation).toBe(state.activeLocation);
					expect(result.state.expandedProjectIds).toBe(state.expandedProjectIds);
				},
			),
			{ numRuns: 200, seed: 20_260_731 },
		);
	});

	it('uses the circular next surviving Directory for every removed primary reference', () => {
		let state = createProject(createProjectWorkspaceState());
		for (const id of ['directory-b', 'directory-c', 'directory-d']) {
			state = transitionProjectWorkspace(state, {
				directory: directory(id),
				resourceScopeId: 'resource-scope-1',
				type: 'directory-added',
			}).state;
		}
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-d',
			launchConfigurationId: null,
			name: 'Workspace D',
			resourceScopeId: 'resource-scope-1',
			projectId: 'project-1',
			terminalTitle: 'Terminal 2',
			type: 'workspace-created',
		}).state;
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-c',
			resourceScopeId: 'resource-scope-1',
			type: 'resource-primary-directory-set',
		}).state;

		const result = transitionProjectWorkspace(state, {
			directoryIds: ['directory-a', 'directory-c', 'directory-d'],
			resourceScopeId: 'resource-scope-1',
			type: 'directories-removed',
		});

		expect(result.outcome).toBe('applied');
		expect(result.state.resourceScopes[0]).toMatchObject({
			directories: [{ id: 'directory-b' }],
			primaryDirectoryId: 'directory-b',
		});
		expect(
			result.state.workspaces.filter(
				(workspace) => workspace.projectId === result.state.projects[0]?.id,
			),
		).toMatchObject([
			{ id: 'workspace-1', primaryDirectoryId: 'directory-b' },
			{ id: 'workspace-2', primaryDirectoryId: 'directory-b' },
		]);
	});

	it('wraps a removed final primary Directory to the first survivor', () => {
		let state = createProject(createProjectWorkspaceState());
		state = transitionProjectWorkspace(state, {
			directory: directory('directory-b'),
			resourceScopeId: 'resource-scope-1',
			type: 'directory-added',
		}).state;
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-b',
			resourceScopeId: 'resource-scope-1',
			type: 'resource-primary-directory-set',
		}).state;

		const result = transitionProjectWorkspace(state, {
			directoryIds: ['directory-b'],
			resourceScopeId: 'resource-scope-1',
			type: 'directories-removed',
		});

		expect(result.state.resourceScopes[0]?.primaryDirectoryId).toBe('directory-a');
	});

	it('rejects removal when a Workspace would fall back to a remote Directory', () => {
		let state = transitionProjectWorkspace(createProjectWorkspaceState(), {
			directory: remoteDirectory('environment-directory:00000000000000000000000000000001'),
			projectName: 'Remote Project',
			type: 'environment-project-registered',
		}).state;
		state = transitionProjectWorkspace(state, {
			directory: directory('directory-local'),
			resourceScopeId: 'resource-scope-1',
			type: 'directory-added',
		}).state;
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-local',
			launchConfigurationId: null,
			name: 'Local Workspace',
			resourceScopeId: 'resource-scope-1',
			projectId: 'project-1',
			terminalTitle: 'Local Terminal',
			type: 'workspace-created',
		}).state;

		const result = transitionProjectWorkspace(state, {
			directoryIds: ['directory-local'],
			resourceScopeId: 'resource-scope-1',
			type: 'directories-removed',
		});

		expect(result).toMatchObject({
			commands: [],
			outcome: 'rejected',
			rejection: 'directory-removal-not-allowed',
			state,
		});
	});

	it('rejects removal of the final or automatically discovered Directory', () => {
		const singleton = createProject(createProjectWorkspaceState());
		const finalResult = transitionProjectWorkspace(singleton, {
			directoryIds: ['directory-a'],
			resourceScopeId: 'resource-scope-1',
			type: 'directories-removed',
		});
		expect(finalResult).toMatchObject({
			commands: [],
			outcome: 'rejected',
			rejection: 'last-directory',
			state: singleton,
		});

		const withWorktree = transitionProjectWorkspace(singleton, {
			directory: {
				...directory('worktree-a', 'worktree'),
				source: 'selected',
			},
			resourceScopeId: 'resource-scope-1',
			type: 'directory-added',
		}).state;
		const worktreeResult = transitionProjectWorkspace(withWorktree, {
			directoryIds: ['worktree-a'],
			resourceScopeId: 'resource-scope-1',
			type: 'directories-removed',
		});
		expect(worktreeResult).toMatchObject({
			outcome: 'rejected',
			rejection: 'directory-removal-not-allowed',
			state: withWorktree,
		});
	});

	it('refreshes Directory observations without changing identity, order, or primary references', () => {
		let state = createProject(createProjectWorkspaceState());
		state = transitionProjectWorkspace(state, {
			directory: directory('directory-b'),
			resourceScopeId: 'resource-scope-1',
			type: 'directory-added',
		}).state;
		const result = transitionProjectWorkspace(state, {
			directory: {
				...directory('directory-a'),
				available: false,
				name: 'renamed',
				path: '/projects/renamed',
			},
			resourceScopeId: 'resource-scope-1',
			type: 'directory-updated',
		});

		expect(result.outcome).toBe('applied');
		expect(result.state.resourceScopes[0]).toMatchObject({
			directories: [
				{
					available: false,
					id: 'directory-a',
					name: 'renamed',
					path: '/projects/renamed',
				},
				{ id: 'directory-b' },
			],
			primaryDirectoryId: 'directory-a',
		});
		expect(
			result.state.workspaces.filter(
				(workspace) => workspace.projectId === result.state.projects[0]?.id,
			),
		).toMatchObject([{ primaryDirectoryId: 'directory-a' }]);
	});

	it('rejects Directory refreshes that change semantic kind or registration source', () => {
		const state = createProject(createProjectWorkspaceState());
		const changedKind = transitionProjectWorkspace(state, {
			directory: {
				...directory('directory-a', 'worktree'),
				source: 'selected',
			},
			resourceScopeId: 'resource-scope-1',
			type: 'directory-updated',
		});
		const changedSource = transitionProjectWorkspace(state, {
			directory: { ...directory('directory-a'), source: 'discovered' },
			resourceScopeId: 'resource-scope-1',
			type: 'directory-updated',
		});

		for (const result of [changedKind, changedSource]) {
			expect(result).toEqual({
				commands: [],
				outcome: 'rejected',
				rejection: 'directory-update-not-allowed',
				state,
			});
		}
	});

	it('keeps launch configuration references valid across create, duplicate, and reconcile', () => {
		const configuration = { id: 'launch-dev', name: 'Dev' };
		let state = transitionProjectWorkspace(createProjectWorkspaceState(), {
			directory: directory('directory-a'),
			launchConfigurations: [configuration],
			primaryWorkspaceLaunchConfigurationId: configuration.id,
			primaryWorkspaceName: 'Primary',
			projectName: 'Project',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		}).state;
		state = transitionProjectWorkspace(state, {
			copyName: 'Primary Copy',

			type: 'workspace-duplicated',
			workspaceId: 'workspace-1',
		}).state;

		expect(
			state.workspaces.filter((workspace) => workspace.projectId === state.projects[0]?.id),
		).toMatchObject([
			{ launchConfigurationId: configuration.id },
			{ launchConfigurationId: configuration.id },
		]);

		const result = transitionProjectWorkspace(state, {
			launchConfigurations: [],
			resourceScopeId: 'resource-scope-1',
			type: 'resource-launch-configurations-reconciled',
		});
		expect(result.state.resourceScopes[0]).toMatchObject({ launchConfigurations: [] });
		expect(
			result.state.workspaces.filter(
				(workspace) => workspace.projectId === result.state.projects[0]?.id,
			),
		).toMatchObject([{ launchConfigurationId: null }, { launchConfigurationId: null }]);
	});

	it('creates and duplicates Workspaces with fresh terminal-context identities', () => {
		let state = createProject(createProjectWorkspaceState());
		state = transitionProjectWorkspace(state, {
			directory: directory('directory-b'),
			resourceScopeId: 'resource-scope-1',
			type: 'directory-added',
		}).state;
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-b',
			launchConfigurationId: null,
			name: 'Feature',
			resourceScopeId: 'resource-scope-1',
			projectId: 'project-1',
			terminalTitle: 'Terminal 2',
			type: 'workspace-created',
		}).state;
		const result = transitionProjectWorkspace(state, {
			copyName: 'Feature Copy',

			type: 'workspace-duplicated',
			workspaceId: 'workspace-2',
		});

		expect(result.commands).toEqual([
			{
				sourceTerminalContextId: 'terminal-context-3',
				targetTerminalContextId: 'terminal-context-4',
				type: 'terminal-context-cloned',
			},
		]);
		expect(
			result.state.workspaces.filter(
				(workspace) => workspace.projectId === result.state.projects[0]?.id,
			)[2],
		).toMatchObject({
			id: 'workspace-3',
			name: 'Feature Copy',
			primaryDirectoryId: 'directory-b',
			terminalContextId: 'terminal-context-4',
		});
		expect(result.state.activeLocation).toEqual({
			kind: 'workspace',

			workspaceId: 'workspace-3',
		});
	});

	it('allows the final Workspace to be deleted and falls back to standalone', () => {
		const state = createProject(createProjectWorkspaceState());
		const result = transitionProjectWorkspace(state, {
			type: 'workspace-deleted',
			workspaceId: 'workspace-1',
		});

		expect(result.state.activeLocation).toEqual({ kind: 'standalone' });
		expect(
			result.state.workspaces.filter(
				(workspace) => workspace.projectId === result.state.projects[0]?.id,
			),
		).toEqual([]);
		expect(result.commands).toEqual([
			{ terminalContextId: 'terminal-context-2', type: 'terminal-context-disposed' },
		]);
	});

	it('dissolves only grouping and retains every running Workspace', () => {
		let state = createProject(createProjectWorkspaceState());
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-a',
			launchConfigurationId: null,
			name: 'Feature',
			resourceScopeId: 'resource-scope-1',
			projectId: 'project-1',
			terminalTitle: 'Terminal 2',
			type: 'workspace-created',
		}).state;
		const result = transitionProjectWorkspace(state, {
			projectId: 'project-1',
			type: 'project-removed',
		});

		expect(result.state.projects).toEqual([]);
		expect(result.state.activeLocation).toBe(state.activeLocation);
		expect(result.state.resourceScopes).toBe(state.resourceScopes);
		expect(result.state.workspaces).toEqual(
			state.workspaces.map((workspace) => ({ ...workspace, projectId: null })),
		);
		expect(result.commands).toEqual([]);
	});

	it('preserves membership, primary references, context uniqueness, and active-location validity', () => {
		assert(
			property(
				uniqueArray(integer({ min: 1, max: 20 }), { minLength: 1, maxLength: 8 }),
				uniqueArray(integer({ min: 1, max: 20 }), { maxLength: 8 }),
				(directorySequences, removalCandidates) => {
					let state = createProject(
						createProjectWorkspaceState(),
						directory(`directory-${directorySequences[0]}`),
					);
					for (const sequence of directorySequences.slice(1)) {
						state = transitionProjectWorkspace(state, {
							directory: directory(`directory-${sequence}`),
							resourceScopeId: 'resource-scope-1',
							type: 'directory-added',
						}).state;
					}
					for (const sequence of directorySequences.slice(0, 3)) {
						state = transitionProjectWorkspace(state, {
							directoryId: `directory-${sequence}`,
							launchConfigurationId: null,
							name: `Workspace ${sequence}`,
							resourceScopeId: 'resource-scope-1',
							projectId: 'project-1',
							terminalTitle: `Terminal ${sequence}`,
							type: 'workspace-created',
						}).state;
					}
					const projectBefore = state.resourceScopes[0];
					if (projectBefore === undefined) throw new Error('Generated Project is missing.');
					const requestedRemovalIds = removalCandidates.map((sequence) => `directory-${sequence}`);
					const removableIds = requestedRemovalIds.filter((id) =>
						projectBefore.directories.some((candidate) => candidate.id === id),
					);
					if (removableIds.length < projectBefore.directories.length) {
						state = transitionProjectWorkspace(state, {
							directoryIds: removableIds,
							resourceScopeId: 'resource-scope-1',
							type: 'directories-removed',
						}).state;
					}
					const project = state.resourceScopes[0];
					if (project === undefined) throw new Error('Project vanished during generated trace.');
					const directoryIds = new Set(project.directories.map((candidate) => candidate.id));
					const contextIds = new Set<string>([state.standaloneTerminalContextId]);
					expect(project.directories.length).toBeGreaterThan(0);
					expect(directoryIds.has(project.primaryDirectoryId)).toBe(true);
					for (const workspace of state.workspaces) {
						expect(directoryIds.has(workspace.primaryDirectoryId)).toBe(true);
						expect(contextIds.has(workspace.terminalContextId)).toBe(false);
						contextIds.add(workspace.terminalContextId);
					}
					const activeLocation = state.activeLocation;
					const activeWorkspaceExists =
						activeLocation.kind === 'standalone' ||
						state.workspaces.some((workspace) => workspace.id === activeLocation.workspaceId);
					expect(activeWorkspaceExists).toBe(true);
				},
			),
			{ numRuns: 200, seed: 20_260_730 },
		);
	});
});
