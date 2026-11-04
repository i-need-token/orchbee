import { describe, expect, it } from 'vitest';

import {
	createProjectWorkspaceState,
	transitionProjectWorkspace,
	type ProjectWorkspaceState,
} from '../project-workspace-state';

function oneProjectState(): ProjectWorkspaceState {
	return {
		...createProjectWorkspaceState(),
		activeLocation: { kind: 'workspace', workspaceId: 'workspace-1' },
		expandedProjectIds: ['project-1'],
		projects: [{ id: 'project-1', name: 'Project' }],
		resourceScopes: [
			{
				directories: [
					{
						available: true,
						git: null,
						id: 'directory-a',
						kind: 'folder',
						name: 'directory-a',
						path: '/projects/directory-a',
						placement: { kind: 'local' },
						source: 'selected',
					},
				],
				id: 'resource-scope-1',
				launchConfigurations: [],
				name: 'Project',
				primaryDirectoryId: 'directory-a',
			},
		],
		workspaces: [
			{
				id: 'workspace-1',
				launchConfigurationId: null,
				name: 'Primary',
				primaryDirectoryId: 'directory-a',
				projectId: 'project-1',
				resourceScopeId: 'resource-scope-1',
				terminalContextId: 'terminal-context-2',
			},
		],
	};
}

describe('project-workspace-state', () => {
	it('toggles Project disclosure without changing the active location', () => {
		const state = oneProjectState();
		const result = transitionProjectWorkspace(state, {
			projectId: 'project-1',
			type: 'project-expansion-toggled',
		});

		expect(result.state.expandedProjectIds).toEqual([]);
		expect(result.state.activeLocation).toBe(state.activeLocation);
	});
});
