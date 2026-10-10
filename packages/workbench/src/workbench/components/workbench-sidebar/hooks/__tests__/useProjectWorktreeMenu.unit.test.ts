// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, StrictMode } from 'react';
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
import { useProjectWorktreeMenu } from '../useProjectWorktreeMenu';

afterEach(cleanup);

describe('useProjectWorktreeMenu', () => {
	it('refreshes in Strict Mode and adopts an opaque Worktree identity', async () => {
		const capability = createCapability();
		const requestWorkspaceActivation = vi.fn<(workspaceId: string) => void>();
		const rendered = renderHook(
			() =>
				useProjectWorktreeMenu({
					capability,
					projectWorkspaceState: projectState(),
					beginWorkspaceActivation: () => requestWorkspaceActivation,
				}),
			{ wrapper: ({ children }) => createElement(StrictMode, null, children) },
		);

		act(() => rendered.result.current.refresh('workspace-1'));
		await vi.waitFor(() => expect(rendered.result.current.busy).toBe(false));

		expect(capability.discover).toHaveBeenCalledWith({
			preferredDirectoryId: 'directory-1',
			sourceWorkspaceId: 'workspace-1',
		});
		expect(rendered.result.current.discoveryByWorkspace.get('workspace-1')).toMatchObject({
			repositories: [
				{
					label: 'orchbee',
					worktreeInventory: {
						kind: 'available',
						worktrees: [{ label: 'feature/one', worktreeId: 'worktree-1' }],
					},
				},
			],
		});

		act(() =>
			rendered.result.current.adopt({
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
				worktreeId: 'worktree-1',
			}),
		);
		await vi.waitFor(() => expect(rendered.result.current.busy).toBe(false));
		expect(capability.adopt).toHaveBeenCalledWith({
			sourceWorkspaceId: 'workspace-1',
			repositoryId: 'repository-1',
			sourceDirectoryId: 'directory-1',
			worktreeId: 'worktree-1',
		});
		expect(rendered.result.current.failure).toBeNull();
		expect(requestWorkspaceActivation).toHaveBeenCalledOnce();
		expect(requestWorkspaceActivation).toHaveBeenCalledWith('workspace-2');
	});

	it('serializes rapid Project refreshes and exposes an adoption failure', async () => {
		let settleFirst: ((result: WorkbenchProjectWorktreeDiscoveryResult) => void) | undefined;
		const capability = createCapability();
		const requestWorkspaceActivation = vi.fn<(workspaceId: string) => void>();
		capability.discover
			.mockImplementationOnce(
				() =>
					new Promise<WorkbenchProjectWorktreeDiscoveryResult>((resolve) => {
						settleFirst = resolve;
					}),
			)
			.mockResolvedValueOnce({ discovery: discovery('ghostty'), kind: 'discovered' });
		capability.adopt.mockResolvedValueOnce({ code: 'worktree-not-found', kind: 'failed' });
		const rendered = renderHook(() =>
			useProjectWorktreeMenu({
				capability,
				projectWorkspaceState: projectState(2),
				beginWorkspaceActivation: () => requestWorkspaceActivation,
			}),
		);

		act(() => {
			rendered.result.current.refresh('workspace-1');
			rendered.result.current.refresh('workspace-2');
		});
		expect(capability.discover).toHaveBeenCalledTimes(1);
		await act(async () => {
			settleFirst?.({ discovery: discovery('orchbee'), kind: 'discovered' });
			await Promise.resolve();
		});
		await vi.waitFor(() => expect(capability.discover).toHaveBeenCalledTimes(2));
		await vi.waitFor(() => expect(rendered.result.current.busy).toBe(false));

		act(() =>
			rendered.result.current.adopt({
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
				worktreeId: 'worktree-stale',
			}),
		);
		await vi.waitFor(() => expect(rendered.result.current.failure).toBe('worktree-not-found'));
		expect(requestWorkspaceActivation).not.toHaveBeenCalled();
		act(rendered.result.current.dismissFailure);
		expect(rendered.result.current.failure).toBeNull();
	});
});

function createCapability() {
	return {
		adopt: vi.fn<WorkbenchProjectWorktreeCapability['adopt']>(async () => ({
			directoryId: 'discovered-worktree-1',
			kind: 'adopted',
			workspaceId: 'workspace-2',
		})),
		create: vi.fn<WorkbenchProjectWorktreeCapability['create']>(),
		discover: vi.fn<WorkbenchProjectWorktreeCapability['discover']>(async () => ({
			discovery: discovery('orchbee'),
			kind: 'discovered',
		})),
	};
}

function discovery(label: string) {
	return {
		managedRoot: '/managed/worktrees',
		repositories: [
			{
				defaultBaseRef: 'main',
				label,
				localBaseRefs: ['main'],
				repositoryId: `repository-${label}`,
				sourceDirectoryId: `directory-${label}`,
				suggestedBranchName: `${label}/workspace-1`,
				worktreeInventory: {
					kind: 'available' as const,
					worktrees: [{ label: 'feature/one', worktreeId: 'worktree-1' }],
				},
			},
		],
	};
}

function projectState(count = 1): ProjectWorkspaceState {
	let state = createProjectWorkspaceState();
	for (let index = 1; index <= count; index += 1) {
		state = transitionProjectWorkspace(state, {
			directory: {
				available: true,
				git: null,
				id: `directory-${index}`,
				kind: 'folder',
				name: index === 1 ? 'orchbee' : 'ghostty',
				path: `/workspace/project-${index}`,
				placement: { kind: 'local' },
				source: 'selected',
			},
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: `project-${index}`,
			terminalTitle: 'Terminal',
			type: 'project-created',
		}).state;
	}
	return state;
}
