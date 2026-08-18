// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	createProjectWorkspaceState,
	transitionProjectWorkspace,
	type ProjectWorkspaceState,
} from '../../../../../project-workspace';
import type {
	WorkbenchProjectFolderAcquisition,
	WorkbenchProjectFolderAcquisitionResult,
} from '../../../../types/workbench-project-folder-acquisition';
import { useProjectFolderAcquisition } from '../useProjectFolderAcquisition';

afterEach(cleanup);

describe('useProjectFolderAcquisition', () => {
	it('commits a selected Folder once and suppresses overlapping requests', async () => {
		let settle: ((result: WorkbenchProjectFolderAcquisitionResult) => void) | undefined;
		const acquisition: WorkbenchProjectFolderAcquisition = {
			selectExistingFolder: vi.fn<() => Promise<WorkbenchProjectFolderAcquisitionResult>>(
				() =>
					new Promise<WorkbenchProjectFolderAcquisitionResult>((resolve) => {
						settle = resolve;
					}),
			),
		};
		const createProject = vi.fn<(path: string) => void>();
		const rendered = renderHook(() =>
			useProjectFolderAcquisition({
				acquisition,
				addProjectDirectory: vi.fn<(projectId: string, path: string) => void>(),
				createProject,
				projectWorkspaceState: createProjectWorkspaceState(),
			}),
		);

		act(() => {
			rendered.result.current.requestCreateProject();
			rendered.result.current.requestCreateProject();
		});
		expect(acquisition.selectExistingFolder).toHaveBeenCalledOnce();
		expect(rendered.result.current.selecting).toBe(true);
		await act(async () => {
			settle?.({ kind: 'selected', path: '/workspace/orchbee' });
			await Promise.resolve();
		});

		expect(createProject).toHaveBeenCalledWith('/workspace/orchbee');
		expect(rendered.result.current.selecting).toBe(false);
	});

	it('rechecks the latest Project before adding and ignores settlement after unmount', async () => {
		let settle: ((result: WorkbenchProjectFolderAcquisitionResult) => void) | undefined;
		const acquisition: WorkbenchProjectFolderAcquisition = {
			selectExistingFolder: () =>
				new Promise<WorkbenchProjectFolderAcquisitionResult>((resolve) => {
					settle = resolve;
				}),
		};
		const addProjectDirectory = vi.fn<(projectId: string, path: string) => void>();
		const rendered = renderHook(
			({ snapshot }: { readonly snapshot: ProjectWorkspaceState }) =>
				useProjectFolderAcquisition({
					acquisition,
					addProjectDirectory,
					createProject: vi.fn<(path: string) => void>(),
					projectWorkspaceState: snapshot,
				}),
			{ initialProps: { snapshot: projectState() } },
		);

		act(() => rendered.result.current.requestAddProjectFolder('project-1'));
		rendered.rerender({ snapshot: createProjectWorkspaceState() });
		await act(async () => {
			settle?.({ kind: 'selected', path: '/workspace/new' });
			await Promise.resolve();
		});
		expect(addProjectDirectory).not.toHaveBeenCalled();
		expect(rendered.result.current.failure).toBe('project-unavailable');

		act(() => rendered.result.current.dismissFailure());
		act(() => rendered.result.current.requestCreateProject());
		rendered.unmount();
		settle?.({ kind: 'selected', path: '/workspace/late' });
		await Promise.resolve();
		expect(addProjectDirectory).not.toHaveBeenCalled();
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
