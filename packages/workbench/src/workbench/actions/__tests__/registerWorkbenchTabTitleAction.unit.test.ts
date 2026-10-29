import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { createWorkbenchPreviewTerminalPresentationComposition } from '../../model/createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
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
		const harness = createHarness();
		const context = harness.projectWorkspaceRuntime.getActiveTerminalContext();

		await expect(harness.execute(undefined)).resolves.toEqual({ kind: 'completed' });

		expect(harness.openEditor).toHaveBeenCalledExactlyOnceWith({
			tabId: 'terminal-context-1-tab-1',
			terminalContextId: context.terminalContextId,
			title: 'workspace',
		});
	});

	it('opens and commits the explicitly targeted Tab without redirecting to the active Tab', async () => {
		const harness = createHarness();
		const context = harness.projectWorkspaceRuntime.getActiveTerminalContext();
		const firstTabId = context.tabModel.getSnapshot().activeTabId;
		const creation = harness.projectWorkspaceRuntime.createTerminal({
			groupId: null,
			source: 'button',
			title: 'Terminal 2',
		});
		if (firstTabId === null || creation.kind !== 'created') {
			throw new Error('Expected two terminal Tabs.');
		}
		context.tabModel.selectTab(firstTabId);
		const target = {
			kind: 'open' as const,
			tabId: creation.tabId,
			terminalContextId: context.terminalContextId,
		};

		await expect(harness.execute(target)).resolves.toEqual({ kind: 'completed' });
		expect(harness.openEditor).toHaveBeenLastCalledWith({
			tabId: target.tabId,
			terminalContextId: target.terminalContextId,
			title: 'workspace',
		});

		await expect(
			harness.execute({ ...target, kind: 'commit', title: '  Focus work  ' }),
		).resolves.toEqual({ kind: 'completed' });
		expect(context.tabModel.getSnapshot()).toMatchObject({ activeTabId: firstTabId });
		expect(
			context.tabModel.getSnapshot().tabs.find((tab) => tab.id === creation.tabId)?.title,
		).toBe('Focus work');
		expect(context.tabModel.getSnapshot().tabs.find((tab) => tab.id === firstTabId)?.title).toBe(
			'workspace',
		);
	});

	it('maps an empty commit to reset and reports an unchanged reset exactly', async () => {
		const harness = createHarness();
		const context = harness.projectWorkspaceRuntime.getActiveTerminalContext();
		const tabId = context.tabModel.getSnapshot().activeTabId;
		if (tabId === null) throw new Error('Expected one active Tab.');
		context.panelWorkspaceStore.setTabCustomTitle(tabId, 'Custom');
		const input = {
			kind: 'commit' as const,
			tabId,
			terminalContextId: context.terminalContextId,
			title: '   ',
		};

		await expect(harness.execute(input)).resolves.toEqual({ kind: 'completed' });
		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('workspace');
		await expect(harness.execute(input)).resolves.toEqual({ kind: 'unchanged' });
	});

	it('disables stale Tab and terminal-context targets', async () => {
		const harness = createHarness();
		const context = harness.projectWorkspaceRuntime.getActiveTerminalContext();
		const tabId = context.tabModel.getSnapshot().activeTabId;
		if (tabId === null) throw new Error('Expected one active Tab.');
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

		context.tabModel.closeTab(tabId);
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

function createHarness(): Harness {
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('workbench');
	const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/workspace',
		standaloneTerminalTitle: 'Terminal 1',
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('terminal-1'),
			() => terminalPresentationId('terminal-next'),
		),
	});
	const openEditor = vi.fn<(request: WorkbenchTabTitleEditorRequest) => void>();
	registerWorkbenchTabTitleAction({
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
