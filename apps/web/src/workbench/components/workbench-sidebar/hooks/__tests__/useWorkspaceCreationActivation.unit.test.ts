// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, startTransition, StrictMode, Suspense } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import {
	createSnapshotRuntime,
	fixtureDirectoryId,
	fixtureProjectId,
	fixtureWorkspaceId,
	publishSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotProject,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../../../__tests__/server-workbench-snapshot';
import { useWorkspaceCreationActivation } from '../useWorkspaceCreationActivation';

afterEach(cleanup);
const subscribeNavigation = () => () => undefined;
const PRIMARY_WORKSPACE_ID = fixtureWorkspaceId(1);
const CREATED_WORKSPACE_ID = fixtureWorkspaceId(2);
const OTHER_WORKSPACE_ID = fixtureWorkspaceId(3);

describe('useWorkspaceCreationActivation', () => {
	it.each(['before', 'after'] as const)(
		'waits for committed membership when creation settles %s publication commits',
		(settlement) => {
			const activateWorkspace = vi.fn<(workspaceId: string) => void>();
			const runtime = createSnapshotRuntime(navigationSnapshot(1, [1]));
			const initial = runtime.getSnapshot();
			publishSnapshot(runtime, navigationSnapshot(2, [1, 2]));
			const published = runtime.getSnapshot();
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
			if (settlement === 'before') act(() => complete(CREATED_WORKSPACE_ID));
			expect(activateWorkspace).not.toHaveBeenCalled();
			rendered.rerender({ snapshot: published, suspend: false });
			if (settlement === 'after') act(() => complete(CREATED_WORKSPACE_ID));
			expect(activateWorkspace).toHaveBeenCalledExactlyOnceWith(CREATED_WORKSPACE_ID);
		},
	);

	it('waits through publication and regrouping, then activates the exact Workspace once', () => {
		const activateWorkspace = vi.fn<(workspaceId: string) => void>();
		const runtime = createSnapshotRuntime(navigationSnapshot(1, [1]));
		const rendered = renderHook(
			({ projectWorkspaceState }) =>
				useWorkspaceCreationActivation({
					subscribeNavigation,
					activateWorkspace,
					projectWorkspaceState,
				}),
			{
				initialProps: { projectWorkspaceState: runtime.getSnapshot() },
				wrapper: ({ children }) => createElement(StrictMode, null, children),
			},
		);
		const complete = rendered.result.current.begin();
		act(() => complete(CREATED_WORKSPACE_ID));
		expect(activateWorkspace).not.toHaveBeenCalled();
		// The Server publishes the new Workspace after its Project was dissolved.
		publishSnapshot(runtime, navigationSnapshot(2, [1, 2], false));
		const state = runtime.getSnapshot();
		rendered.rerender({ projectWorkspaceState: state });
		expect(activateWorkspace).toHaveBeenCalledExactlyOnceWith(CREATED_WORKSPACE_ID);
		rendered.rerender({ projectWorkspaceState: state });
		expect(activateWorkspace).toHaveBeenCalledOnce();
	});

	it('activates an already published Workspace after the operation result arrives', () => {
		const activateWorkspace = vi.fn<(workspaceId: string) => void>();
		const state = createSnapshotRuntime(navigationSnapshot(1, [1, 2])).getSnapshot();
		const rendered = renderHook(() =>
			useWorkspaceCreationActivation({
				subscribeNavigation,
				activateWorkspace,
				projectWorkspaceState: state,
			}),
		);
		const complete = rendered.result.current.begin();
		act(() => complete(CREATED_WORKSPACE_ID));
		expect(activateWorkspace).toHaveBeenCalledExactlyOnceWith(CREATED_WORKSPACE_ID);
	});

	it('does not steal focus after navigation away and back, or after explicit navigation to the same location', () => {
		const activateWorkspace = vi.fn<(workspaceId: string) => void>();
		const runtime = createSnapshotRuntime(navigationSnapshot(1, [1, 3]));
		const rendered = renderHook(
			({ projectWorkspaceState }) =>
				useWorkspaceCreationActivation({
					subscribeNavigation,
					activateWorkspace,
					projectWorkspaceState,
				}),
			{ initialProps: { projectWorkspaceState: runtime.getSnapshot() } },
		);
		const complete = rendered.result.current.begin();
		runtime.transition({ type: 'workspace-activated', workspaceId: OTHER_WORKSPACE_ID });
		rendered.rerender({ projectWorkspaceState: runtime.getSnapshot() });
		runtime.transition({ type: 'workspace-activated', workspaceId: PRIMARY_WORKSPACE_ID });
		publishSnapshot(runtime, navigationSnapshot(2, [1, 2, 3]));
		rendered.rerender({ projectWorkspaceState: runtime.getSnapshot() });
		act(() => complete(CREATED_WORKSPACE_ID));
		const second = rendered.result.current.begin();
		act(() => rendered.result.current.cancel());
		act(() => second(CREATED_WORKSPACE_ID));
		expect(activateWorkspace).not.toHaveBeenCalled();
	});

	it('ignores completion after unmount', () => {
		const activateWorkspace = vi.fn<(workspaceId: string) => void>();
		const rendered = renderHook(() =>
			useWorkspaceCreationActivation({
				subscribeNavigation,
				activateWorkspace,
				projectWorkspaceState: createSnapshotRuntime(navigationSnapshot(1, [1])).getSnapshot(),
			}),
		);
		const complete = rendered.result.current.begin();
		rendered.unmount();
		act(() => complete(PRIMARY_WORKSPACE_ID));
		expect(activateWorkspace).not.toHaveBeenCalled();
	});
});

/** Workspaces `sequences` in Directory 1, in Project 1 unless it was dissolved. */
function navigationSnapshot(
	revision: number,
	sequences: readonly number[],
	grouped = true,
): WorkbenchSnapshot {
	const projectId = grouped ? fixtureProjectId(1) : null;
	return workbenchSnapshot(
		revision,
		{
			directories: [snapshotDirectory(1)],
			projects: grouped ? [snapshotProject(1)] : [],
			workspaces: sequences.map((sequence) =>
				snapshotWorkspace(sequence, { primaryDirectoryId: fixtureDirectoryId(1), projectId }),
			),
		},
		sequences.map((sequence) => snapshotContext(sequence, [snapshotTab(sequence)])),
	);
}
