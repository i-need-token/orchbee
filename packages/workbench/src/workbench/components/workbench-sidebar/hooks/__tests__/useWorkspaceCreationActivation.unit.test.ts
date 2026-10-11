// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, startTransition, StrictMode, Suspense } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	createProjectWorkspaceState,
	transitionProjectWorkspace,
	type ProjectWorkspaceState,
} from '../../../../../project-workspace';
import { useWorkspaceCreationActivation } from '../useWorkspaceCreationActivation';

afterEach(cleanup);
const subscribeNavigation = () => () => undefined;

describe('useWorkspaceCreationActivation', () => {
	it.each(['before', 'after'] as const)(
		'waits for committed membership when creation settles %s publication commits',
		(settlement) => {
			const activateWorkspace = vi.fn<(workspaceId: string) => void>();
			const initial = projectState();
			const published = addWorkspace(initial);
			const pending = new Promise<void>(() => undefined);
			const rendered = renderHook(
				({ snapshot, suspend }) => {
					const view = useWorkspaceCreationActivation({
						activateWorkspace,
						projectWorkspaceState: snapshot,
						subscribeNavigation,
					});
					if (suspend) throw pending;
					return view;
				},
				{
					initialProps: { snapshot: initial, suspend: false },
					wrapper: ({ children }) => createElement(Suspense, { fallback: null }, children),
				},
			);
			const complete = rendered.result.current.begin();
			act(() => {
				startTransition(() => rendered.rerender({ snapshot: published, suspend: true }));
			});
			if (settlement === 'before') act(() => complete('workspace-2'));
			expect(activateWorkspace).not.toHaveBeenCalled();
			rendered.rerender({ snapshot: published, suspend: false });
			if (settlement === 'after') act(() => complete('workspace-2'));
			expect(activateWorkspace).toHaveBeenCalledExactlyOnceWith('workspace-2');
		},
	);

	it('waits through publication and regrouping, then activates the exact Workspace once', () => {
		const activateWorkspace = vi.fn<(workspaceId: string) => void>();
		let state = projectState();
		const rendered = renderHook(
			({ projectWorkspaceState }) =>
				useWorkspaceCreationActivation({
					subscribeNavigation,
					activateWorkspace,
					projectWorkspaceState,
				}),
			{
				initialProps: { projectWorkspaceState: state },
				wrapper: ({ children }) => createElement(StrictMode, null, children),
			},
		);
		const complete = rendered.result.current.begin();
		act(() => complete('workspace-2'));
		expect(activateWorkspace).not.toHaveBeenCalled();
		state = addWorkspace(state);
		state = transitionProjectWorkspace(state, {
			type: 'project-dissolved',
			projectId: 'project-1',
		}).state;
		rendered.rerender({ projectWorkspaceState: state });
		expect(activateWorkspace).toHaveBeenCalledExactlyOnceWith('workspace-2');
		rendered.rerender({ projectWorkspaceState: state });
		expect(activateWorkspace).toHaveBeenCalledOnce();
	});

	it('activates an already published Workspace after the operation result arrives', () => {
		const activateWorkspace = vi.fn<(workspaceId: string) => void>();
		const state = addWorkspace(projectState());
		const rendered = renderHook(() =>
			useWorkspaceCreationActivation({
				subscribeNavigation,
				activateWorkspace,
				projectWorkspaceState: state,
			}),
		);
		const complete = rendered.result.current.begin();
		act(() => complete('workspace-2'));
		expect(activateWorkspace).toHaveBeenCalledExactlyOnceWith('workspace-2');
	});

	it('does not steal focus after navigation away and back, or after explicit navigation to the same location', () => {
		const activateWorkspace = vi.fn<(workspaceId: string) => void>();
		let state = projectState();
		const rendered = renderHook(
			({ projectWorkspaceState }) =>
				useWorkspaceCreationActivation({
					subscribeNavigation,
					activateWorkspace,
					projectWorkspaceState,
				}),
			{ initialProps: { projectWorkspaceState: state } },
		);
		const complete = rendered.result.current.begin();
		state = transitionProjectWorkspace(state, { type: 'standalone-activated' }).state;
		rendered.rerender({ projectWorkspaceState: state });
		state = transitionProjectWorkspace(state, {
			type: 'workspace-activated',
			workspaceId: 'workspace-1',
		}).state;
		state = addWorkspace(state);
		rendered.rerender({ projectWorkspaceState: state });
		act(() => complete('workspace-2'));
		const second = rendered.result.current.begin();
		act(() => rendered.result.current.cancel());
		act(() => second('workspace-2'));
		expect(activateWorkspace).not.toHaveBeenCalled();
	});

	it('ignores completion after unmount', () => {
		const activateWorkspace = vi.fn<(workspaceId: string) => void>();
		const rendered = renderHook(() =>
			useWorkspaceCreationActivation({
				subscribeNavigation,
				activateWorkspace,
				projectWorkspaceState: projectState(),
			}),
		);
		const complete = rendered.result.current.begin();
		rendered.unmount();
		act(() => complete('workspace-1'));
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
	const next = transitionProjectWorkspace(state, {
		directoryId: 'directory-1',
		launchConfigurationId: null,
		name: 'feature',
		resourceScopeId: 'resource-scope-1',
		projectId: 'project-1',
		terminalTitle: 'Terminal',
		type: 'workspace-created',
	}).state;
	return { ...next, activeLocation: state.activeLocation };
}
