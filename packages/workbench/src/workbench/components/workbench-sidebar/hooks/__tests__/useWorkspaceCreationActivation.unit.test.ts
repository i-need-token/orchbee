// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, StrictMode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	createProjectWorkspaceState,
	transitionProjectWorkspace,
	type ProjectWorkspaceState,
} from '../../../../../project-workspace';
import { useWorkspaceCreationActivation } from '../useWorkspaceCreationActivation';

afterEach(cleanup);

describe('useWorkspaceCreationActivation', () => {
	it('waits through late publication and activates the exact Workspace once', () => {
		const activateWorkspace = vi.fn<(projectId: string, workspaceId: string) => void>();
		let state = projectState();
		const rendered = renderHook(
			({ projectWorkspaceState }) =>
				useWorkspaceCreationActivation({ activateWorkspace, projectWorkspaceState }),
			{
				initialProps: { projectWorkspaceState: state },
				wrapper: ({ children }) => createElement(StrictMode, null, children),
			},
		);

		act(() => rendered.result.current('project-1', 'workspace-2'));
		expect(activateWorkspace).not.toHaveBeenCalled();

		state = addWorkspace(state);
		rendered.rerender({ projectWorkspaceState: state });
		expect(activateWorkspace).toHaveBeenCalledOnce();
		expect(activateWorkspace).toHaveBeenCalledWith('project-1', 'workspace-2');

		rendered.rerender({ projectWorkspaceState: state });
		expect(activateWorkspace).toHaveBeenCalledOnce();
	});

	it('activates immediately when publication arrived before the result identity', () => {
		const activateWorkspace = vi.fn<(projectId: string, workspaceId: string) => void>();
		const state = addWorkspace(projectState());
		const rendered = renderHook(() =>
			useWorkspaceCreationActivation({ activateWorkspace, projectWorkspaceState: state }),
		);

		act(() => rendered.result.current('project-1', 'workspace-2'));

		expect(activateWorkspace).toHaveBeenCalledOnce();
		expect(activateWorkspace).toHaveBeenCalledWith('project-1', 'workspace-2');
	});

	it('does not match the same Workspace identity in a different Project', () => {
		const activateWorkspace = vi.fn<(projectId: string, workspaceId: string) => void>();
		const state = projectState();
		const rendered = renderHook(() =>
			useWorkspaceCreationActivation({ activateWorkspace, projectWorkspaceState: state }),
		);

		act(() => rendered.result.current('project-2', 'workspace-1'));

		expect(activateWorkspace).not.toHaveBeenCalled();
	});
});

function projectState(): ProjectWorkspaceState {
	return transitionProjectWorkspace(createProjectWorkspaceState(), {
		directory: {
			available: true,
			git: null,
			id: 'directory-1',
			kind: 'folder',
			name: 'orchbee',
			path: '/workspace/orchbee',
			placement: { kind: 'local' },
			source: 'selected',
		},
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: 'Primary',
		projectName: 'orchbee',
		terminalTitle: 'Terminal',
		type: 'project-created',
	}).state;
}

function addWorkspace(state: ProjectWorkspaceState): ProjectWorkspaceState {
	return transitionProjectWorkspace(state, {
		directoryId: 'directory-1',
		launchConfigurationId: null,
		name: 'feature',
		projectId: 'project-1',
		terminalTitle: 'Terminal',
		type: 'workspace-created',
	}).state;
}
