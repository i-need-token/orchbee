import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	createSnapshotRuntime,
	fixtureTabId,
	oneWorkspaceSnapshot,
	publishSnapshot,
	snapshotPanel,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import { inertWorkbenchAuthority } from '../../../__tests__/inertWorkbenchAuthority';
import type { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority } from '../../types/workbench-authority';
import {
	registerWorkbenchTabTitleAction,
	type WorkbenchTabTitleEditorRequest,
} from '../registerWorkbenchTabTitleAction';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchTabTitleInput,
} from '../workbench-action-ids';

describe('registerWorkbenchTabTitleAction', () => {
	it('opens the active Tab with a captured stable identity and effective title', async () => {
		const harness = createHarness(oneWorkspaceSnapshot(1, [snapshotTab(1)]));
		const context = harness.projectWorkspaceRuntime.getActiveTerminalContext();

		await expect(harness.execute(undefined)).resolves.toEqual({ kind: 'completed' });

		expect(harness.openEditor).toHaveBeenCalledExactlyOnceWith({
			tabId: fixtureTabId(1),
			terminalContextId: context.terminalContextId,
			title: 'Tab 1',
			unnamed: false,
		});
	});

	it('opens and commits the explicitly targeted Tab without redirecting to the active Tab', async () => {
		const change = vi.fn<WorkbenchAuthority['change']>(() =>
			Promise.resolve({ kind: 'completed' }),
		);
		const harness = createHarness(oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2)]), {
			change,
		});
		const context = harness.projectWorkspaceRuntime.getActiveTerminalContext();
		context.tabModel.selectTab(fixtureTabId(1));
		const target = {
			kind: 'open' as const,
			tabId: fixtureTabId(2),
			terminalContextId: context.terminalContextId,
		};

		await expect(harness.execute(target)).resolves.toEqual({ kind: 'completed' });
		expect(harness.openEditor).toHaveBeenLastCalledWith({
			tabId: target.tabId,
			terminalContextId: target.terminalContextId,
			title: 'Tab 2',
			unnamed: false,
		});

		await expect(
			harness.execute({ ...target, kind: 'commit', title: '  Focus work  ' }),
		).resolves.toEqual({ kind: 'completed' });
		expect(change).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			label: 'Focus work',
			tabId: target.tabId,
			terminalContextId: target.terminalContextId,
			type: 'tab-renamed',
		});
		expect(context.tabModel.getSnapshot()).toMatchObject({ activeTabId: fixtureTabId(1) });
	});

	it('leaves a shared name to its authority, which only hears a name someone typed', async () => {
		const change = vi.fn<WorkbenchAuthority['change']>(() =>
			Promise.resolve({ kind: 'completed' }),
		);
		const harness = createHarness(
			oneWorkspaceSnapshot(1, [snapshotTab(1, snapshotPanel(1), { automaticLabel: true })]),
			{ change },
		);
		const context = harness.projectWorkspaceRuntime.getActiveTerminalContext();
		const tabId = context.tabModel.getSnapshot().activeTabId;
		if (tabId === null) throw new Error('Expected one active Tab.');
		const target = { tabId, terminalContextId: context.terminalContextId };
		const shown = context.tabModel.getSnapshot();

		await harness.execute({ ...target, kind: 'open' });
		expect(harness.openEditor).toHaveBeenLastCalledWith(expect.objectContaining({ unnamed: true }));

		await expect(harness.execute({ ...target, kind: 'commit', title: '   ' })).resolves.toEqual({
			kind: 'unchanged',
		});
		expect(change).not.toHaveBeenCalled();

		await expect(
			harness.execute({ ...target, kind: 'commit', title: '  Focus work  ' }),
		).resolves.toEqual({ kind: 'completed' });
		expect(change).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			...target,
			label: 'Focus work',
			type: 'tab-renamed',
		});
		// The name shows once the authority publishes it.
		expect(context.tabModel.getSnapshot()).toBe(shown);
	});

	it('disables stale Tab and terminal-context targets', async () => {
		const harness = createHarness(oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2)]));
		const context = harness.projectWorkspaceRuntime.getActiveTerminalContext();
		const tabId = fixtureTabId(1);
		const staleContextInput = {
			kind: 'open' as const,
			tabId,
			terminalContextId: 'missing-context',
		};
		expect(harness.resolve(staleContextInput)).toMatchObject({
			availability: { kind: 'disabled' },
		});
		await expect(harness.execute(staleContextInput)).resolves.toMatchObject({
			code: 'action-unavailable',
			kind: 'blocked',
		});

		publishSnapshot(harness.projectWorkspaceRuntime, oneWorkspaceSnapshot(2, [snapshotTab(2)]));
		const staleTabInput = {
			kind: 'open' as const,
			tabId,
			terminalContextId: context.terminalContextId,
		};
		expect(harness.resolve(staleTabInput)).toMatchObject({
			availability: { kind: 'disabled' },
		});
	});
});

interface Harness {
	readonly openEditor: ReturnType<typeof vi.fn<(request: WorkbenchTabTitleEditorRequest) => void>>;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	execute(input: WorkbenchTabTitleInput): Promise<unknown>;
	resolve(input: WorkbenchTabTitleInput): ReturnType<ActionRuntime['resolveAction']>;
}

function createHarness(
	snapshot: WorkbenchSnapshot,
	authority: Partial<WorkbenchAuthority> = {},
): Harness {
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('workbench');
	const projectWorkspaceRuntime = createSnapshotRuntime(snapshot);
	const openEditor = vi.fn<(request: WorkbenchTabTitleEditorRequest) => void>();
	registerWorkbenchTabTitleAction({
		authority: inertWorkbenchAuthority(authority),
		openEditor,
		projectWorkspaceRuntime,
		scope,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
	});
	return {
		execute: async (input) =>
			await runtime.execute(WORKBENCH_ACTION_IDS.setTabTitle, input, {
				scope,
				source: 'keyboard',
			}).completion,
		openEditor,
		projectWorkspaceRuntime,
		resolve: (input) =>
			runtime.resolveAction(WORKBENCH_ACTION_IDS.setTabTitle, input, {
				scope,
				source: 'keyboard',
			}),
	};
}
