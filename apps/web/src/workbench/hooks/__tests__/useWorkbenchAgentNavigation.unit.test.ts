// @vitest-environment jsdom

import type { ActionExecution } from '@orchbee/actions';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, startTransition, Suspense } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	createSnapshotRuntime,
	oneWorkspaceSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../__tests__/server-workbench-snapshot';
import type { WorkbenchActiveLocation } from '../../../project-workspace';
import type { WorkbenchAgentNavigationInput } from '../../model/project-workbench-agent-overview';
import type { WorkbenchAgentSnapshot } from '../../types/workbench-agent-observation';
import { useWorkbenchAgentNavigation } from '../useWorkbenchAgentNavigation';
import { pendingActionExecution } from './pendingActionExecution';

afterEach(cleanup);

const blockedExecution: ActionExecution = {
	cancel: () => undefined,
	completion: Promise.resolve({ code: 'unavailable', kind: 'blocked' }),
	executionId: 'agent-navigation-test',
	getSnapshot: () => ({
		progress: null,
		result: { code: 'unavailable', kind: 'blocked' },
		state: 'blocked',
	}),
	subscribe: () => ({ dispose: () => undefined }),
};

describe('useWorkbenchAgentNavigation', () => {
	it.each([false, true])(
		'cancels pending focus only for committed unrelated navigation (committed: %s)',
		async (committed) => {
			const fixture = navigationFixture();
			const execution = pendingActionExecution('pending-agent-navigation');
			const announceUnavailable = vi.fn<() => void>();
			const requestTerminalFocus = vi.fn<() => boolean>(() => true);
			const invokeAgentNavigation = vi.fn<
				(input: WorkbenchAgentNavigationInput) => ActionExecution
			>(() => execution.execution);
			const origin: WorkbenchActiveLocation = {
				kind: 'workspace',
				workspaceId: fixture.originWorkspaceId,
			};
			const unrelated: WorkbenchActiveLocation = {
				kind: 'workspace',
				workspaceId: fixture.unrelatedWorkspaceId,
			};
			const pending = new Promise<void>(() => undefined);
			const initialProps: { activeLocation: WorkbenchActiveLocation; suspend: boolean } = {
				activeLocation: origin,
				suspend: false,
			};
			const rendered = renderHook(
				({ activeLocation, suspend }) => {
					const navigate = useWorkbenchAgentNavigation({
						activeLocation,
						announceUnavailable,
						invokeAgentNavigation,
						projectWorkspaceRuntime: fixture.runtime,
						requestTerminalFocus,
						source: fixture.source,
					});
					if (suspend) throw pending;
					return navigate;
				},
				{
					initialProps,
					wrapper: ({ children }) => createElement(Suspense, { fallback: null }, children),
				},
			);
			if (!committed) {
				act(() =>
					startTransition(() => {
						rendered.rerender({ activeLocation: unrelated, suspend: true });
					}),
				);
			}
			act(() => rendered.result.current(fixture.input));
			if (committed) {
				act(() => {
					fixture.runtime.transition({
						type: 'workspace-activated',
						workspaceId: fixture.unrelatedWorkspaceId,
					});
				});
			}
			rendered.rerender({ activeLocation: committed ? unrelated : origin, suspend: false });
			// A newer state of the same Agent reevaluates the pending navigation.
			act(() => fixture.source.publish(fixture.observed(2n)));
			expect(invokeAgentNavigation).toHaveBeenCalledExactlyOnceWith(fixture.input);
			expect(requestTerminalFocus).not.toHaveBeenCalled();
			act(() => {
				fixture.runtime.transition({
					type: 'workspace-activated',
					workspaceId: fixture.input.workspaceId,
				});
			});
			rendered.rerender({
				activeLocation: { kind: 'workspace', workspaceId: fixture.input.workspaceId },
				suspend: false,
			});
			await execution.settle({ kind: 'completed' });
			expect(announceUnavailable).not.toHaveBeenCalled();
			expect(requestTerminalFocus).toHaveBeenCalledTimes(committed ? 0 : 1);
			rendered.unmount();
		},
	);

	it('announces a terminal that holds no Agent in the latest projection', () => {
		const runtime = createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1)]));
		const activeLocation = runtime.getSnapshot().activeLocation;
		const announceUnavailable = vi.fn<() => void>();
		const invokeAgentNavigation = vi.fn<(input: WorkbenchAgentNavigationInput) => ActionExecution>(
			() => blockedExecution,
		);
		const requestTerminalFocus = vi.fn<() => boolean>(() => true);
		const rendered = renderHook(() =>
			useWorkbenchAgentNavigation({
				activeLocation,
				announceUnavailable,
				invokeAgentNavigation,
				projectWorkspaceRuntime: runtime,
				requestTerminalFocus,
				source: undefined,
			}),
		);

		act(() =>
			rendered.result.current({
				presentationId: terminalPresentationId('terminal-a'),
				workspaceId: 'workspace-a',
			}),
		);

		expect(invokeAgentNavigation).toHaveBeenCalledOnce();
		expect(announceUnavailable).toHaveBeenCalledOnce();
		expect(requestTerminalFocus).not.toHaveBeenCalled();
	});
});

function navigationFixture() {
	const runtime = createSnapshotRuntime(
		workbenchSnapshot(
			1,
			{
				directories: [snapshotDirectory(1), snapshotDirectory(2), snapshotDirectory(3)],
				workspaces: [snapshotWorkspace(1), snapshotWorkspace(2), snapshotWorkspace(3)],
			},
			[
				snapshotContext(1, [snapshotTab(1)]),
				snapshotContext(2, [snapshotTab(2)]),
				snapshotContext(3, [snapshotTab(3)]),
			],
		),
	);
	// The person starts in a third Workspace, away from both.
	const [target, unrelated, origin] = runtime.getSnapshot().workspaces;
	if (target === undefined || unrelated === undefined || origin === undefined) {
		throw new Error('Expected three Workspaces.');
	}
	const panel = runtime
		.getTerminalContext(target.terminalContextId)
		?.panelWorkspaceStore.getSnapshot().tabs[0]?.root;
	if (panel?.kind !== 'panel') throw new Error('Expected one target Panel.');
	runtime.transition({ type: 'workspace-activated', workspaceId: origin.id });
	const { presentationId } = panel.primaryPresentation;
	const observed = (sequence: bigint): WorkbenchAgentSnapshot => ({
		complete: true,
		presentations: new Map([
			[presentationId, { agent: 'codex', binding: 'navigation-terminal', sequence, state: 'idle' }],
		]),
	});
	let snapshot = observed(1n);
	const listeners = new Set<() => void>();
	const source = {
		getSnapshot: () => snapshot,
		publish: (next: WorkbenchAgentSnapshot) => {
			snapshot = next;
			for (const listener of listeners) listener();
		},
		subscribe: (listener: () => void) => {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		},
	};
	return {
		input: { presentationId, workspaceId: target.id },
		observed,
		originWorkspaceId: origin.id,
		runtime,
		source,
		unrelatedWorkspaceId: unrelated.id,
	};
}
