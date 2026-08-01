import { assert, integer, property, uniqueArray } from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	createProjectWorkspaceState,
	transitionProjectWorkspace,
	type ProjectWorkspaceState,
	type WorkbenchDirectory,
} from '../project-workspace-state';

function directory(id: string, kind: WorkbenchDirectory['kind'] = 'folder'): WorkbenchDirectory {
	const base = {
		available: true,
		id,
		kind,
		name: id,
		path: `/projects/${id}`,
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

function createProject(state: ProjectWorkspaceState, primaryDirectory = directory('directory-a')) {
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
			projectId: 'project-1',
			terminalTitle: `Terminal ${index}`,
			type: 'workspace-created',
		}).state;
	}
	return state;
}

describe('project-workspace-state', () => {
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
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		});
		expect(result.state.projects[0]).toMatchObject({
			id: 'project-1',
			launchConfigurations: [],
			primaryDirectoryId: 'directory-a',
			workspaces: [
				{
					id: 'workspace-1',
					launchConfigurationId: null,
					name: 'Primary',
					primaryDirectoryId: 'directory-a',
					terminalContextId: 'terminal-context-2',
				},
			],
		});
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
		const project = state.projects[0];
		if (project === undefined) throw new Error('Expected the owning Project.');
		const result = transitionProjectWorkspace(state, {
			beforeWorkspaceId: 'workspace-1',
			projectId: 'project-1',
			type: 'workspace-reordered',
			workspaceId: 'workspace-3',
		});

		expect(result.outcome).toBe('applied');
		expect(result.state.projects[0]?.workspaces.map((workspace) => workspace.id)).toEqual([
			'workspace-3',
			'workspace-1',
			'workspace-2',
		]);
		expect(result.state.projects[0]?.directories).toBe(project.directories);
		expect(result.state.projects[0]?.launchConfigurations).toBe(project.launchConfigurations);
		expect(result.state.activeLocation).toBe(state.activeLocation);
		expect(result.state.expandedProjectIds).toBe(state.expandedProjectIds);
		expect(result.commands).toEqual([]);
	});

	it('moves a Workspace to the end and leaves self or satisfied placement unchanged', () => {
		const state = createWorkspaces(3);
		const moved = transitionProjectWorkspace(state, {
			beforeWorkspaceId: null,
			projectId: 'project-1',
			type: 'workspace-reordered',
			workspaceId: 'workspace-1',
		});
		expect(moved.state.projects[0]?.workspaces.map((workspace) => workspace.id)).toEqual([
			'workspace-2',
			'workspace-3',
			'workspace-1',
		]);

		for (const event of [
			{
				beforeWorkspaceId: 'workspace-1',
				projectId: 'project-1',
				type: 'workspace-reordered' as const,
				workspaceId: 'workspace-1',
			},
			{
				beforeWorkspaceId: 'workspace-2',
				projectId: 'project-1',
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
					projectId: 'project-missing',
					type: 'workspace-reordered' as const,
					workspaceId: 'workspace-2',
				},
				'project-not-found',
			],
			[
				{
					beforeWorkspaceId: 'workspace-1',
					projectId: 'project-1',
					type: 'workspace-reordered' as const,
					workspaceId: 'workspace-missing',
				},
				'workspace-not-found',
			],
			[
				{
					beforeWorkspaceId: 'workspace-3',
					projectId: 'project-1',
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
					const project = state.projects[0];
					if (project === undefined) throw new Error('Generated Project is missing.');
					const sourceIndex = sourceSeed % workspaceCount;
					const targetSlot = targetSeed % (workspaceCount + 1);
					const sourceWorkspace = project.workspaces[sourceIndex];
					if (sourceWorkspace === undefined) {
						throw new Error('Generated source Workspace is missing.');
					}
					const beforeWorkspaceId =
						targetSlot === workspaceCount ? null : (project.workspaces[targetSlot]?.id ?? null);
					const result = transitionProjectWorkspace(state, {
						beforeWorkspaceId,
						projectId: project.id,
						type: 'workspace-reordered',
						workspaceId: sourceWorkspace.id,
					});
					const beforeIds = project.workspaces.map((workspace) => workspace.id);
					const afterWorkspaces = result.state.projects[0]?.workspaces ?? [];
					const afterIds = afterWorkspaces.map((workspace) => workspace.id);

					expect([...afterIds].sort()).toEqual([...beforeIds].sort());
					expect(new Set(afterIds).size).toBe(workspaceCount);
					expect(afterWorkspaces.map((workspace) => workspace.terminalContextId).sort()).toEqual(
						project.workspaces.map((workspace) => workspace.terminalContextId).sort(),
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
				projectId: 'project-1',
				type: 'directory-added',
			}).state;
		}
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-d',
			launchConfigurationId: null,
			name: 'Workspace D',
			projectId: 'project-1',
			terminalTitle: 'Terminal 2',
			type: 'workspace-created',
		}).state;
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-c',
			projectId: 'project-1',
			type: 'project-primary-directory-set',
		}).state;

		const result = transitionProjectWorkspace(state, {
			directoryIds: ['directory-a', 'directory-c', 'directory-d'],
			projectId: 'project-1',
			type: 'directories-removed',
		});

		expect(result.outcome).toBe('applied');
		expect(result.state.projects[0]).toMatchObject({
			directories: [{ id: 'directory-b' }],
			primaryDirectoryId: 'directory-b',
			workspaces: [
				{ id: 'workspace-1', primaryDirectoryId: 'directory-b' },
				{ id: 'workspace-2', primaryDirectoryId: 'directory-b' },
			],
		});
	});

	it('wraps a removed final primary Directory to the first survivor', () => {
		let state = createProject(createProjectWorkspaceState());
		state = transitionProjectWorkspace(state, {
			directory: directory('directory-b'),
			projectId: 'project-1',
			type: 'directory-added',
		}).state;
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-b',
			projectId: 'project-1',
			type: 'project-primary-directory-set',
		}).state;

		const result = transitionProjectWorkspace(state, {
			directoryIds: ['directory-b'],
			projectId: 'project-1',
			type: 'directories-removed',
		});

		expect(result.state.projects[0]?.primaryDirectoryId).toBe('directory-a');
	});

	it('rejects removal of the final or automatically discovered Directory', () => {
		const singleton = createProject(createProjectWorkspaceState());
		const finalResult = transitionProjectWorkspace(singleton, {
			directoryIds: ['directory-a'],
			projectId: 'project-1',
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
			projectId: 'project-1',
			type: 'directory-added',
		}).state;
		const worktreeResult = transitionProjectWorkspace(withWorktree, {
			directoryIds: ['worktree-a'],
			projectId: 'project-1',
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
			projectId: 'project-1',
			type: 'directory-added',
		}).state;
		const result = transitionProjectWorkspace(state, {
			directory: {
				...directory('directory-a'),
				available: false,
				name: 'renamed',
				path: '/projects/renamed',
			},
			projectId: 'project-1',
			type: 'directory-updated',
		});

		expect(result.outcome).toBe('applied');
		expect(result.state.projects[0]).toMatchObject({
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
			workspaces: [{ primaryDirectoryId: 'directory-a' }],
		});
	});

	it('rejects Directory refreshes that change semantic kind or registration source', () => {
		const state = createProject(createProjectWorkspaceState());
		const changedKind = transitionProjectWorkspace(state, {
			directory: {
				...directory('directory-a', 'worktree'),
				source: 'selected',
			},
			projectId: 'project-1',
			type: 'directory-updated',
		});
		const changedSource = transitionProjectWorkspace(state, {
			directory: { ...directory('directory-a'), source: 'discovered' },
			projectId: 'project-1',
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
			projectId: 'project-1',
			type: 'workspace-duplicated',
			workspaceId: 'workspace-1',
		}).state;

		expect(state.projects[0]?.workspaces).toMatchObject([
			{ launchConfigurationId: configuration.id },
			{ launchConfigurationId: configuration.id },
		]);

		const result = transitionProjectWorkspace(state, {
			launchConfigurations: [],
			projectId: 'project-1',
			type: 'project-launch-configurations-reconciled',
		});
		expect(result.state.projects[0]).toMatchObject({
			launchConfigurations: [],
			workspaces: [{ launchConfigurationId: null }, { launchConfigurationId: null }],
		});
	});

	it('creates and duplicates Workspaces with fresh terminal-context identities', () => {
		let state = createProject(createProjectWorkspaceState());
		state = transitionProjectWorkspace(state, {
			directory: directory('directory-b'),
			projectId: 'project-1',
			type: 'directory-added',
		}).state;
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-b',
			launchConfigurationId: null,
			name: 'Feature',
			projectId: 'project-1',
			terminalTitle: 'Terminal 2',
			type: 'workspace-created',
		}).state;
		const result = transitionProjectWorkspace(state, {
			copyName: 'Feature Copy',
			projectId: 'project-1',
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
		expect(result.state.projects[0]?.workspaces[2]).toMatchObject({
			id: 'workspace-3',
			name: 'Feature Copy',
			primaryDirectoryId: 'directory-b',
			terminalContextId: 'terminal-context-4',
		});
		expect(result.state.activeLocation).toEqual({
			kind: 'workspace',
			projectId: 'project-1',
			workspaceId: 'workspace-3',
		});
	});

	it('allows the final Workspace to be deleted and falls back to standalone', () => {
		const state = createProject(createProjectWorkspaceState());
		const result = transitionProjectWorkspace(state, {
			projectId: 'project-1',
			type: 'workspace-deleted',
			workspaceId: 'workspace-1',
		});

		expect(result.state.activeLocation).toEqual({ kind: 'standalone' });
		expect(result.state.projects[0]?.workspaces).toEqual([]);
		expect(result.commands).toEqual([
			{ terminalContextId: 'terminal-context-2', type: 'terminal-context-disposed' },
		]);
	});

	it('removes only Orchbee Project state and returns every context for disposal', () => {
		let state = createProject(createProjectWorkspaceState());
		state = transitionProjectWorkspace(state, {
			directoryId: 'directory-a',
			launchConfigurationId: null,
			name: 'Feature',
			projectId: 'project-1',
			terminalTitle: 'Terminal 2',
			type: 'workspace-created',
		}).state;
		const result = transitionProjectWorkspace(state, {
			projectId: 'project-1',
			type: 'project-removed',
		});

		expect(result.state.projects).toEqual([]);
		expect(result.state.activeLocation).toEqual({ kind: 'standalone' });
		expect(result.commands).toEqual([
			{ terminalContextId: 'terminal-context-2', type: 'terminal-context-disposed' },
			{ terminalContextId: 'terminal-context-3', type: 'terminal-context-disposed' },
		]);
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
							projectId: 'project-1',
							type: 'directory-added',
						}).state;
					}
					for (const sequence of directorySequences.slice(0, 3)) {
						state = transitionProjectWorkspace(state, {
							directoryId: `directory-${sequence}`,
							launchConfigurationId: null,
							name: `Workspace ${sequence}`,
							projectId: 'project-1',
							terminalTitle: `Terminal ${sequence}`,
							type: 'workspace-created',
						}).state;
					}
					const projectBefore = state.projects[0];
					if (projectBefore === undefined) throw new Error('Generated Project is missing.');
					const requestedRemovalIds = removalCandidates.map((sequence) => `directory-${sequence}`);
					const removableIds = requestedRemovalIds.filter((id) =>
						projectBefore.directories.some((candidate) => candidate.id === id),
					);
					if (removableIds.length < projectBefore.directories.length) {
						state = transitionProjectWorkspace(state, {
							directoryIds: removableIds,
							projectId: 'project-1',
							type: 'directories-removed',
						}).state;
					}
					const project = state.projects[0];
					if (project === undefined) throw new Error('Project vanished during generated trace.');
					const directoryIds = new Set(project.directories.map((candidate) => candidate.id));
					const contextIds = new Set<string>([state.standaloneTerminalContextId]);
					expect(project.directories.length).toBeGreaterThan(0);
					expect(directoryIds.has(project.primaryDirectoryId)).toBe(true);
					for (const workspace of project.workspaces) {
						expect(directoryIds.has(workspace.primaryDirectoryId)).toBe(true);
						expect(contextIds.has(workspace.terminalContextId)).toBe(false);
						contextIds.add(workspace.terminalContextId);
					}
					const activeLocation = state.activeLocation;
					const activeWorkspaceExists =
						activeLocation.kind === 'standalone' ||
						project.workspaces.some((workspace) => workspace.id === activeLocation.workspaceId);
					expect(activeWorkspaceExists).toBe(true);
				},
			),
			{ numRuns: 200, seed: 20_260_730 },
		);
	});
});
