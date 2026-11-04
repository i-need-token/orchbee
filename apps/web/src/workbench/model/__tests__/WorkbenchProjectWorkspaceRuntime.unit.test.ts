import { describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixtureContextId,
	fixtureDirectoryId,
	fixturePanelId,
	fixturePresentationId,
	fixtureTabId,
	fixtureWorkspaceId,
	oneWorkspaceSnapshot,
	publishSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotPanel,
	snapshotPresentation,
	snapshotSplit,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../__tests__/server-workbench-snapshot';
import type { WorkbenchProjectWorkspaceRuntime } from '../WorkbenchProjectWorkspaceRuntime';

/** Workspace 1 holds two Tabs and Workspace 2 one, both in Directory 1. */
function twoWorkspaceSnapshot(revision: number, workspaceCount: 1 | 2 = 2) {
	return workbenchSnapshot(
		revision,
		{
			directories: [snapshotDirectory(1)],
			workspaces: [
				snapshotWorkspace(1),
				snapshotWorkspace(2, { primaryDirectoryId: fixtureDirectoryId(1) }),
			].slice(0, workspaceCount),
		},
		[
			snapshotContext(1, [snapshotTab(1), snapshotTab(2)]),
			snapshotContext(2, [snapshotTab(3)]),
		].slice(0, workspaceCount),
	);
}

function presentation(sequence: number) {
	return terminalPresentationId(fixturePresentationId(sequence));
}

function observeCloseActivity(
	runtime: WorkbenchProjectWorkspaceRuntime,
	sequence: number,
	closeActivity: 'active' | 'idle' | 'unknown',
) {
	return runtime.applyTerminalMetadataObservation(presentation(sequence), {
		closeActivity,
		currentDirectory: { kind: 'ignored' },
		naturalTitle: null,
	});
}

describe('WorkbenchProjectWorkspaceRuntime', () => {
	it('protects a terminal from closing only while its activity is not idle', () => {
		const runtime = createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1)]));
		const tabId = fixtureTabId(1);

		expect(runtime.getActiveTerminalTabsCloseConfirmationCount([tabId])).toBe(1);
		for (const [closeActivity, expectedCount] of [
			['idle', 0],
			['active', 1],
			['unknown', 1],
		] as const) {
			expect(observeCloseActivity(runtime, 1, closeActivity)).toEqual({ kind: 'applied' });
			expect(runtime.getActiveTerminalTabsCloseConfirmationCount([tabId])).toBe(expectedCount);
		}
	});

	it('counts close protection across every panel and child terminal in a Tab', () => {
		const runtime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [
				snapshotTab(
					1,
					snapshotSplit(
						1,
						'horizontal',
						snapshotPanel(1, snapshotPresentation(1), [snapshotPresentation(3)]),
						snapshotPanel(2),
					),
				),
			]),
		);
		const tabId = fixtureTabId(1);
		observeCloseActivity(runtime, 1, 'idle');
		for (const sequence of [2, 3]) {
			expect(observeCloseActivity(runtime, sequence, 'active')).toEqual({ kind: 'applied' });
		}

		expect(runtime.getActiveTerminalTabsCloseConfirmationCount([tabId])).toBe(2);
		expect(runtime.getActiveTerminalTabsCloseConfirmationCount([tabId, tabId])).toBe(2);
		expect(runtime.getActiveTerminalTabsCloseConfirmationCount(['missing-tab'])).toBe(0);
	});

	it('names a Tab without a chosen name by its current directory until a natural title arrives', () => {
		const runtime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1, snapshotPanel(1), { automaticLabel: true })]),
		);
		const context = activeTerminalContext(runtime);
		const title = () => context.tabModel.getSnapshot().tabs[0]?.title;
		const publishedTitle = title();

		expect(
			runtime.applyTerminalMetadataObservation(presentation(1), {
				closeActivity: 'idle',
				currentDirectory: { currentCwd: '/workspace/orchbee', kind: 'reported' },
				naturalTitle: null,
			}),
		).toEqual({ kind: 'applied' });
		expect(title()).toBe('orchbee');

		runtime.applyTerminalMetadataObservation(presentation(1), {
			closeActivity: 'active',
			currentDirectory: { kind: 'ignored' },
			naturalTitle: 'vim',
		});
		expect(title()).toBe('vim');

		runtime.applyTerminalMetadataObservation(presentation(1), {
			closeActivity: 'idle',
			currentDirectory: { currentCwd: 'C:\\Users\\someone\\orchbee', kind: 'reported' },
			naturalTitle: null,
		});
		expect(title()).toBe('orchbee');

		runtime.applyTerminalMetadataObservation(presentation(1), {
			closeActivity: 'idle',
			currentDirectory: { currentCwd: null, kind: 'reported' },
			naturalTitle: null,
		});
		expect(title()).toBe(publishedTitle);
	});

	it('preserves independent terminal contexts when switching locations', () => {
		const runtime = createSnapshotRuntime(twoWorkspaceSnapshot(1));
		const firstContext = activeTerminalContext(runtime);
		expect(firstContext.tabModel.getSnapshot().tabs).toHaveLength(2);

		runtime.transition({ type: 'workspace-activated', workspaceId: fixtureWorkspaceId(2) });
		expect(activeTerminalContext(runtime).tabModel.getSnapshot().tabs).toHaveLength(1);

		runtime.transition({ type: 'workspace-activated', workspaceId: fixtureWorkspaceId(1) });
		expect(activeTerminalContext(runtime)).toBe(firstContext);
	});

	it('notifies once after an applied domain transition and not after rejection', () => {
		const runtime = createSnapshotRuntime(twoWorkspaceSnapshot(1));
		const listener = vi.fn<() => void>();
		runtime.subscribe(listener);

		runtime.transition({ type: 'workspace-activated', workspaceId: fixtureWorkspaceId(2) });
		runtime.transition({ type: 'workspace-activated', workspaceId: 'missing' });

		expect(listener).toHaveBeenCalledTimes(1);
	});

	it('observes terminal topology and releases the observer', () => {
		const runtime = createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1)]));
		const listener = vi.fn<() => void>();
		const initialRevision = runtime.getTerminalTopologyRevision();
		const unsubscribe = runtime.subscribeTerminalTopology(listener);
		const observeTitle = (naturalTitle: string) =>
			runtime.applyTerminalMetadataObservation(presentation(1), {
				closeActivity: 'unknown',
				currentDirectory: { kind: 'ignored' },
				naturalTitle,
			});

		observeTitle('vim');

		expect(listener).toHaveBeenCalledOnce();
		expect(runtime.getTerminalTopologyRevision()).not.toBe(initialRevision);
		listener.mockClear();
		unsubscribe();
		observeTitle('htop');

		expect(listener).not.toHaveBeenCalled();
	});

	it('notifies Client View observers for choices this Client makes, not for Server renames', () => {
		const runtime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2)]),
		);
		const context = activeTerminalContext(runtime);
		const clientViewListener = vi.fn<() => void>();
		const unsubscribeClientView = runtime.subscribeClientView(clientViewListener);

		context.tabModel.selectTab(fixtureTabId(2));

		expect(clientViewListener).toHaveBeenCalledOnce();
		clientViewListener.mockClear();
		publishSnapshot(
			runtime,
			oneWorkspaceSnapshot(2, [snapshotTab(1), snapshotTab(2, undefined, { label: 'Renamed' })]),
		);
		expect(clientViewListener).not.toHaveBeenCalled();
		unsubscribeClientView();
	});

	it('moves a choice the next publication drops to what that publication still holds', () => {
		const runtime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2)]),
		);
		activeTerminalContext(runtime).tabModel.selectTab(fixtureTabId(2));

		publishSnapshot(runtime, oneWorkspaceSnapshot(2, [snapshotTab(1)]));

		expect(runtime.getClientView().terminalContexts[0]?.tabCollection.activeTabId).toBe(
			fixtureTabId(1),
		);
		expect(
			activeTerminalContext(runtime).panelWorkspaceStore.getSnapshot().tabs[0]?.root,
		).toMatchObject({ primaryPresentation: { lifecycle: { kind: 'running' } } });
	});

	it('keeps Panel focus history across a publication without recording it in the Client View', () => {
		const threePanels = snapshotTab(
			1,
			snapshotSplit(
				1,
				'horizontal',
				snapshotPanel(1),
				snapshotSplit(2, 'horizontal', snapshotPanel(2), snapshotPanel(3)),
			),
		);
		const runtime = createSnapshotRuntime(oneWorkspaceSnapshot(1, [threePanels]));
		const context = activeTerminalContext(runtime);
		for (const sequence of [1, 3, 2]) {
			context.panelWorkspaceStore.transition({
				panelId: fixturePanelId(sequence),
				tabId: fixtureTabId(1),
				type: 'panel-focused',
			});
		}
		const focusOrder = [fixturePanelId(1), fixturePanelId(3), fixturePanelId(2)];
		const clientView = runtime.getClientView();

		expect(JSON.stringify(clientView)).not.toContain('panelFocusHistory');
		expect(
			activeTerminalContext(
				createSnapshotRuntime(oneWorkspaceSnapshot(1, [threePanels]), { clientView }),
			).panelWorkspaceStore.getSnapshot().tabs[0]?.panelFocusHistory,
		).toEqual([fixturePanelId(2)]);

		publishSnapshot(runtime, oneWorkspaceSnapshot(2, [threePanels, snapshotTab(4)]));
		expect(
			activeTerminalContext(runtime).panelWorkspaceStore.getSnapshot().tabs[0]?.panelFocusHistory,
		).toEqual(focusOrder);
	});

	it('crosses the Action commit point before applying a transition', () => {
		const runtime = createSnapshotRuntime(twoWorkspaceSnapshot(1));
		const initialState = runtime.getSnapshot();
		const beforeCommit = vi.fn<() => void>(() => {
			expect(runtime.getSnapshot()).toBe(initialState);
		});

		const result = runtime.transition(
			{ type: 'workspace-activated', workspaceId: fixtureWorkspaceId(2) },
			beforeCommit,
		);

		expect(result.outcome).toBe('applied');
		expect(beforeCommit).toHaveBeenCalledOnce();
		expect(runtime.getSnapshot()).not.toBe(initialState);

		const unchangedCommit = vi.fn<() => void>();
		const rejectedCommit = vi.fn<() => void>();
		runtime.transition(
			{ type: 'workspace-activated', workspaceId: fixtureWorkspaceId(2) },
			unchangedCommit,
		);
		runtime.transition({ type: 'workspace-activated', workspaceId: 'missing' }, rejectedCommit);
		expect(unchangedCommit).not.toHaveBeenCalled();
		expect(rejectedCommit).not.toHaveBeenCalled();
	});

	it('isolates observer failures after a transition', () => {
		const runtime = createSnapshotRuntime(twoWorkspaceSnapshot(1));
		const healthyListener = vi.fn<() => void>();
		runtime.subscribe(() => {
			throw new Error('observer failed');
		});
		runtime.subscribe(healthyListener);

		expect(() =>
			runtime.transition({ type: 'workspace-activated', workspaceId: fixtureWorkspaceId(2) }),
		).not.toThrow();
		expect(healthyListener).toHaveBeenCalledOnce();
		expect(runtime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: fixtureWorkspaceId(2),
		});
	});

	it('disposes the context of a Workspace the Server no longer publishes', () => {
		const runtime = createSnapshotRuntime(twoWorkspaceSnapshot(1));
		const firstContext = activeTerminalContext(runtime);

		publishSnapshot(runtime, twoWorkspaceSnapshot(2, 1));

		expect(runtime.getTerminalContext(fixtureContextId(2))).toBeNull();
		expect(activeTerminalContext(runtime)).toBe(firstContext);
	});
});
