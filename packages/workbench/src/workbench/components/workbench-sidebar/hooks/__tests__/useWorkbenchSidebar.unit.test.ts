// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import {
	createProjectWorkspaceState,
	transitionProjectWorkspace,
	type ProjectWorkspaceState,
} from '../../../../../project-workspace';
import type {
	WorkbenchEnvironmentProjectConnectionCapability,
	WorkbenchEnvironmentProjectConnectionStatus,
} from '../../../../types/workbench-environment-project-connection';
import { useWorkbenchSidebar } from '../useWorkbenchSidebar';

afterEach(cleanup);

describe('useWorkbenchSidebar', () => {
	it('prepares and clears a fresh Project creation draft', () => {
		const rendered = renderHook(() => useWorkbenchSidebar(createProjectWorkspaceState()));

		act(() => rendered.result.current.showProjectCreation());
		expect(rendered.result.current.projectCreation?.path).toBe('');
		act(() => rendered.result.current.projectCreation?.setPath('/workspace/orchbee'));
		expect(rendered.result.current.projectCreation?.path).toBe('/workspace/orchbee');
		act(() => rendered.result.current.projectCreation?.close());
		expect(rendered.result.current.projectCreation).toBeNull();
	});

	it('prepares and clears Project and Workspace management drafts', () => {
		const state = projectState();
		const rendered = renderHook(
			({ snapshot }: { readonly snapshot: ProjectWorkspaceState }) => useWorkbenchSidebar(snapshot),
			{ initialProps: { snapshot: state } },
		);

		act(() => rendered.result.current.showProjectManager('project-1'));
		expect(rendered.result.current.projectManagement).toMatchObject({
			name: 'orchbee',
			project: { id: 'project-1' },
		});
		act(() => rendered.result.current.projectManagement?.setName('Orchbee'));
		expect(rendered.result.current.projectManagement?.name).toBe('Orchbee');
		act(() => rendered.result.current.projectManagement?.close());
		expect(rendered.result.current.projectManagement).toBeNull();

		act(() => rendered.result.current.showWorkspaceManager('project-1', 'workspace-1'));
		expect(rendered.result.current.workspaceManagement).toMatchObject({
			name: 'Primary',
			primaryDirectoryId: 'directory-1',
			workspace: { id: 'workspace-1' },
		});
		act(() => rendered.result.current.workspaceManagement?.setName('Feature'));
		expect(rendered.result.current.workspaceManagement?.name).toBe('Feature');
		act(() => rendered.result.current.workspaceManagement?.close());
		expect(rendered.result.current.workspaceManagement).toBeNull();
	});

	it('subscribes to the optional Environment Project connection source', () => {
		let listener: (() => void) | undefined;
		let statuses: readonly WorkbenchEnvironmentProjectConnectionStatus[] = [
			{ projectId: 'project-1', state: 'disconnected' },
		];
		const connection: WorkbenchEnvironmentProjectConnectionCapability = {
			connect: () => undefined,
			disconnect: () => undefined,
			getSnapshot: () => statuses,
			subscribe: (nextListener) => {
				listener = nextListener;
				return () => {
					listener = undefined;
				};
			},
		};
		const rendered = renderHook(() => useWorkbenchSidebar(projectState(), connection));

		expect(rendered.result.current.connectionStatusByProjectId.get('project-1')).toEqual({
			projectId: 'project-1',
			state: 'disconnected',
		});
		act(() => {
			statuses = [{ projectId: 'project-1', state: 'connected' }];
			listener?.();
		});
		expect(rendered.result.current.connectionStatusByProjectId.get('project-1')).toEqual({
			projectId: 'project-1',
			state: 'connected',
		});
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
		terminalTitle: 'Terminal 1',
		type: 'project-created',
	}).state;
}
