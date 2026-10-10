// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	createProjectWorkspaceState,
	transitionProjectWorkspace,
	type ProjectWorkspaceState,
} from '../../../../../project-workspace';
import type {
	WorkbenchProjectWorktreeCapability,
	WorkbenchProjectWorktreeDiscoveryResult,
} from '../../../../types/workbench-project-worktree';
import { useProjectWorktreeCreation } from '../useProjectWorktreeCreation';

afterEach(cleanup);

describe('useProjectWorktreeCreation', () => {
	it('discovers editable defaults and submits the selected repository without a path', async () => {
		const requestWorkspaceActivation = vi.fn<(workspaceId: string) => void>();
		const capability: WorkbenchProjectWorktreeCapability = {
			adopt: vi.fn<WorkbenchProjectWorktreeCapability['adopt']>(),
			create: vi.fn<WorkbenchProjectWorktreeCapability['create']>(async () => ({
				directoryId: 'managed-worktree-1',
				kind: 'created',
				workspaceId: null,
			})),
			discover: vi.fn<WorkbenchProjectWorktreeCapability['discover']>(async () => ({
				discovery: {
					managedRoot: '/managed/worktrees',
					repositories: [
						{
							defaultBaseRef: 'main',
							label: 'orchbee',
							localBaseRefs: ['main'],
							repositoryId: 'repository-1',
							sourceDirectoryId: 'directory-1',
							suggestedBranchName: 'orchbee/workspace-1',
							worktreeInventory: { kind: 'available', worktrees: [] },
						},
					],
				},
				kind: 'discovered',
			})),
		};
		const rendered = renderHook(() =>
			useProjectWorktreeCreation({
				capability,
				projectWorkspaceState: projectState(),
				beginWorkspaceActivation: () => requestWorkspaceActivation,
			}),
		);

		await act(async () => {
			rendered.result.current.open('workspace-1');
			await Promise.resolve();
		});
		expect(capability.discover).toHaveBeenCalledWith({
			preferredDirectoryId: 'directory-1',
			sourceWorkspaceId: 'workspace-1',
		});
		expect(rendered.result.current.dialog).toMatchObject({
			baseRef: 'main',
			branchName: 'orchbee/workspace-1',
			createWorkspace: true,
			phase: 'ready',
			repositoryId: 'repository-1',
		});

		act(() => {
			rendered.result.current.setBaseRef(' release ');
			rendered.result.current.setBranchName(' feature/worktree ');
			rendered.result.current.setCreateWorkspace(false);
		});
		await act(async () => {
			rendered.result.current.submit();
			await Promise.resolve();
		});
		expect(capability.create).toHaveBeenCalledWith({
			baseRef: 'release',
			branchName: 'feature/worktree',
			createWorkspace: false,
			sourceWorkspaceId: 'workspace-1',
			repositoryId: 'repository-1',
			sourceDirectoryId: 'directory-1',
		});
		expect(rendered.result.current.dialog).toBeNull();
		expect(requestWorkspaceActivation).not.toHaveBeenCalled();
	});

	it('requests activation for the exact Workspace returned by a current creation', async () => {
		const requestWorkspaceActivation = vi.fn<(workspaceId: string) => void>();
		const capability: WorkbenchProjectWorktreeCapability = {
			adopt: vi.fn<WorkbenchProjectWorktreeCapability['adopt']>(),
			create: vi.fn<WorkbenchProjectWorktreeCapability['create']>(async () => ({
				directoryId: 'managed-worktree-1',
				kind: 'created',
				workspaceId: 'workspace-2',
			})),
			discover: vi.fn<WorkbenchProjectWorktreeCapability['discover']>(async () => ({
				discovery: {
					managedRoot: '/managed/worktrees',
					repositories: [
						{
							defaultBaseRef: 'main',
							label: 'orchbee',
							localBaseRefs: ['main'],
							repositoryId: 'repository-1',
							sourceDirectoryId: 'directory-1',
							suggestedBranchName: 'orchbee/workspace-1',
							worktreeInventory: { kind: 'available', worktrees: [] },
						},
					],
				},
				kind: 'discovered',
			})),
		};
		const rendered = renderHook(() =>
			useProjectWorktreeCreation({
				capability,
				projectWorkspaceState: projectState(),
				beginWorkspaceActivation: () => requestWorkspaceActivation,
			}),
		);

		await act(async () => {
			rendered.result.current.open('workspace-1');
			await Promise.resolve();
		});
		await act(async () => {
			rendered.result.current.submit();
			await Promise.resolve();
		});

		expect(requestWorkspaceActivation).toHaveBeenCalledOnce();
		expect(requestWorkspaceActivation).toHaveBeenCalledWith('workspace-2');
	});

	it('ignores discovery that settles after the dialog closes', async () => {
		const requestWorkspaceActivation = vi.fn<(workspaceId: string) => void>();
		let settleDiscovery: ((result: WorkbenchProjectWorktreeDiscoveryResult) => void) | undefined;
		const capability: WorkbenchProjectWorktreeCapability = {
			adopt: vi.fn<WorkbenchProjectWorktreeCapability['adopt']>(),
			create: vi.fn<WorkbenchProjectWorktreeCapability['create']>(),
			discover: vi.fn<WorkbenchProjectWorktreeCapability['discover']>(
				() =>
					new Promise<WorkbenchProjectWorktreeDiscoveryResult>((resolve) => {
						settleDiscovery = resolve;
					}),
			),
		};
		const rendered = renderHook(() =>
			useProjectWorktreeCreation({
				capability,
				projectWorkspaceState: projectState(),
				beginWorkspaceActivation: () => requestWorkspaceActivation,
			}),
		);

		act(() => {
			rendered.result.current.open('workspace-1');
			rendered.result.current.close();
		});
		await act(async () => {
			settleDiscovery?.({
				discovery: { managedRoot: '/managed/worktrees', repositories: [] },
				kind: 'discovered',
			});
			await Promise.resolve();
		});
		expect(rendered.result.current.dialog).toBeNull();
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
