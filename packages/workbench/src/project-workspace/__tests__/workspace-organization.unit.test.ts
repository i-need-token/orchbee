import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	createProjectWorkspaceState,
	transitionProjectWorkspace,
	type ProjectWorkspaceState,
	type WorkbenchFolderDirectory,
	type WorkbenchWorktreeDirectory,
} from '../project-workspace-state';

function directory(path: string): WorkbenchFolderDirectory {
	return {
		id: 'shared-id',
		name: path,
		path,
		available: true,
		git: null,
		kind: 'folder',
		placement: { kind: 'local' },
		source: 'selected',
	};
}

function create(state: ProjectWorkspaceState, path: string): ProjectWorkspaceState {
	return transitionProjectWorkspace(state, {
		type: 'project-created',
		directory: directory(path),
		launchConfigurations: [{ id: 'launch', name: path }],
		primaryWorkspaceLaunchConfigurationId: 'launch',
		primaryWorkspaceName: path,
		projectName: path,
		terminalTitle: 'Shell',
	}).state;
}

const worktree: WorkbenchWorktreeDirectory = {
	...directory('/worktrees/feature'),
	id: 'worktree',
	kind: 'worktree',
	source: 'created',
	git: { branch: 'feature', head: 'abc', repositoryId: 'repository', worktree: true },
};

describe('Workspace organization', () => {
	it('creates an ungrouped Workspace and its first shell in one transition', () => {
		const result = transitionProjectWorkspace(createProjectWorkspaceState(), {
			type: 'workspace-created-default',
			directory: directory('/home'),
			name: 'Workspace',
			terminalTitle: 'Shell',
		});
		expect(result.state.projects).toEqual([]);
		expect(result.state.workspaces).toMatchObject([
			{ id: 'workspace-1', projectId: null, launchConfigurationId: null },
		]);
		expect(result.commands).toEqual([
			{
				type: 'terminal-context-created',
				terminalContextId: 'terminal-context-2',
				cwd: '/home',
				terminalTitle: 'Shell',
			},
		]);
	});

	it('moves and dissolves grouping without merging colliding resource IDs or losing unused resources', () => {
		let state = create(create(createProjectWorkspaceState(), '/a'), '/b');
		state = transitionProjectWorkspace(state, {
			type: 'directory-added',
			resourceScopeId: 'resource-scope-1',
			directory: { ...directory('/a/unused'), id: 'unused' },
		}).state;
		const resources = state.resourceScopes;
		const before = state.workspaces;
		const selection = state.activeLocation;
		const moved = transitionProjectWorkspace(state, {
			type: 'workspace-project-set',
			workspaceId: 'workspace-1',
			projectId: 'project-2',
		});
		const dissolved = transitionProjectWorkspace(moved.state, {
			type: 'project-dissolved',
			projectId: 'project-2',
		});
		expect(moved.commands).toEqual([]);
		expect(dissolved.commands).toEqual([]);
		expect(dissolved.state.resourceScopes).toBe(resources);
		expect(dissolved.state.activeLocation).toBe(selection);
		expect(dissolved.state.workspaces).toEqual(
			before.map((workspace) => ({ ...workspace, projectId: null })),
		);
		expect(dissolved.state.resourceScopes.map((scope) => scope.launchConfigurations)).toEqual([
			[{ id: 'launch', name: '/a' }],
			[{ id: 'launch', name: '/b' }],
		]);
	});

	it('creates a group and moves only the chosen Workspace atomically', () => {
		const initial = create(createProjectWorkspaceState(), '/a');
		const result = transitionProjectWorkspace(initial, {
			type: 'workspace-project-created',
			workspaceId: 'workspace-1',
			name: 'Review',
		});
		expect(result.state.projects).toMatchObject([
			{ id: 'project-1' },
			{ id: 'project-2', name: 'Review' },
		]);
		expect(result.state.workspaces[0]).toEqual({
			...initial.workspaces[0],
			projectId: 'project-2',
		});
		expect(result.state.resourceScopes).toBe(initial.resourceScopes);
		expect(result.state.activeLocation).toBe(initial.activeLocation);
		expect(result.commands).toEqual([]);
	});

	it('auto-groups source and worktree, reuses current membership, and never resurrects a closed source', () => {
		const source = transitionProjectWorkspace(createProjectWorkspaceState(), {
			type: 'workspace-created-default',
			directory: directory('/a'),
			name: 'Source',
			terminalTitle: 'Shell',
		}).state;
		const event = {
			type: 'project-worktree-created',
			createWorkspace: true,
			sourceWorkspaceId: 'workspace-1',
			sourceDirectoryId: 'shared-id',
			directory: worktree,
			projectName: 'Repository',
			workspaceName: 'Feature',
			terminalTitle: 'Shell',
		} as const;
		const created = transitionProjectWorkspace(source, event);
		expect(created.state.workspaces.map((workspace) => workspace.projectId)).toEqual([
			'project-1',
			'project-1',
		]);
		expect(created.state.activeLocation).toBe(source.activeLocation);
		expect(created.commands).toHaveLength(1);
		const regrouped = transitionProjectWorkspace(source, {
			type: 'workspace-project-created',
			workspaceId: 'workspace-1',
			name: 'Current',
		}).state;
		const reused = transitionProjectWorkspace(regrouped, event);
		expect(reused.state.projects).toEqual(regrouped.projects);
		expect(reused.state.workspaces[1]?.projectId).toBe('project-1');
		const closed = transitionProjectWorkspace(source, {
			type: 'workspace-deleted',
			workspaceId: 'workspace-1',
		}).state;
		expect(transitionProjectWorkspace(closed, event)).toMatchObject({
			outcome: 'rejected',
			rejection: 'workspace-not-found',
			state: closed,
			commands: [],
		});
	});

	it('keeps a previously associated worktree unique across groups and namespaces without selecting it on publication', () => {
		let state = create(createProjectWorkspaceState(), '/a');
		const event = {
			type: 'project-worktree-adopted',
			sourceWorkspaceId: 'workspace-1',
			sourceDirectoryId: 'shared-id',
			directory: { ...worktree, source: 'discovered' },
			projectName: 'Repository',
			workspaceName: 'Feature',
			terminalTitle: 'Shell',
		} as const;
		state = transitionProjectWorkspace(state, event).state;
		state = transitionProjectWorkspace(state, {
			type: 'workspace-project-set',
			workspaceId: 'workspace-2',
			projectId: null,
		}).state;
		const repeated = transitionProjectWorkspace(state, event);
		expect(repeated).toMatchObject({ outcome: 'unchanged', state, commands: [] });
		expect(repeated.state.workspaces).toHaveLength(2);
	});

	it('keeps ordinary close and Project close-all separate from dissolution', () => {
		let state = create(createProjectWorkspaceState(), '/a');
		state = transitionProjectWorkspace(state, {
			type: 'workspace-duplicated',
			workspaceId: 'workspace-1',
			copyName: 'Second',
		}).state;
		const one = transitionProjectWorkspace(state, {
			type: 'workspace-deleted',
			workspaceId: 'workspace-1',
		});
		expect(one.state.workspaces.map((workspace) => workspace.id)).toEqual(['workspace-2']);
		expect(one.commands).toEqual([
			{ type: 'terminal-context-disposed', terminalContextId: 'terminal-context-2' },
		]);
		const all = transitionProjectWorkspace(state, {
			type: 'project-workspaces-closed',
			projectId: 'project-1',
		});
		expect(all.state.workspaces).toEqual([]);
		expect(all.state.projects).toBe(state.projects);
		expect(all.commands).toHaveLength(2);
	});

	it('preserves every live identity and resource through generated grouping traces', () => {
		fc.assert(
			fc.property(
				fc.array(fc.tuple(fc.integer({ min: 1, max: 3 }), fc.integer({ min: 0, max: 3 })), {
					maxLength: 80,
				}),
				(moves) => {
					let state = create(create(create(createProjectWorkspaceState(), '/a'), '/b'), '/c');
					const resources = state.resourceScopes;
					const workspaces = state.workspaces;
					const selection = state.activeLocation;
					for (const [workspace, project] of moves) {
						const moved = transitionProjectWorkspace(state, {
							type: 'workspace-project-set',
							workspaceId: `workspace-${workspace}`,
							projectId: project === 0 ? null : `project-${project}`,
						});
						expect(moved.commands).toEqual([]);
						state = moved.state;
						expect(state.resourceScopes).toBe(resources);
						expect(state.activeLocation).toBe(selection);
						expect(state.workspaces.map(({ projectId: _group, ...value }) => value)).toEqual(
							workspaces.map(({ projectId: _group, ...value }) => value),
						);
					}
				},
			),
			{ numRuns: 120, seed: 20261010 },
		);
	});
});
