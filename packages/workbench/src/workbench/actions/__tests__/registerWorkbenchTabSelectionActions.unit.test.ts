import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	type ActionExecutionContext,
	type ActionResult,
} from '@orchbee/actions';
import {
	TAB_STRIP_ACTION_IDS,
	tabSelectionInput,
	type TabSelectionInput,
} from '@orchbee/tab-strip';
import { describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../../model/createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { registerWorkbenchTabSelectionActions } from '../registerWorkbenchTabSelectionActions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../workbench-action-ids';

describe('registerWorkbenchTabSelectionActions', () => {
	it('selects the one-based TabModel position through the prepared Tab Strip Action', async () => {
		const harness = createHarness();
		const model = harness.projectWorkspaceRuntime.getActiveTerminalContext().tabModel;
		model.addTab({ pinned: false, title: 'Grouped A' }, { activate: false, id: 'group-a' });
		model.addTab({ pinned: false, title: 'Grouped B' }, { activate: false, id: 'group-b' });
		model.createGroup({ collapsed: true, color: 'blue', id: 'group-1', title: 'Group 1' }, [
			'group-a',
			'group-b',
		]);
		model.addTab({ pinned: true, title: 'Pinned' }, { activate: false, id: 'pinned' });
		const lowerExecute = vi.fn<(input: TabSelectionInput) => void>();
		registerTabSelection(harness, (context, input) => {
			lowerExecute(input);
			model.selectTab(input.tabId);
			context.markCommitted();
			return { kind: 'completed' };
		});

		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'terminal-context-1-tab-1',
			'group-a',
			'group-b',
		]);
		await expect(harness.execute({ position: 3 })).resolves.toEqual({ kind: 'completed' });

		expect(lowerExecute).toHaveBeenCalledExactlyOnceWith({ mode: 'replace', tabId: 'group-a' });
		expect(model.getSnapshot()).toMatchObject({ activeTabId: 'group-a' });
		expect(model.getSnapshot().groups[0]).toMatchObject({ collapsed: false, id: 'group-1' });
	});

	it('is unavailable when the active terminal context has no Tab at that position', async () => {
		const harness = createHarness();
		registerTabSelection(harness, () => ({ kind: 'completed' }));

		expect(
			harness.runtime.resolveAction(
				WORKBENCH_ACTION_IDS.selectTabByPosition,
				{ position: 2 },
				{ scope: harness.scope, source: 'keyboard' },
			),
		).toMatchObject({ availability: { kind: 'disabled' } });
		await expect(harness.execute({ position: 2 })).resolves.toMatchObject({
			code: 'action-unavailable',
			kind: 'blocked',
		});
	});
});

interface Harness {
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ReturnType<ActionRuntime['createScope']>;
	execute(input: { readonly position: number }): Promise<unknown>;
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
	registerWorkbenchTabSelectionActions({
		projectWorkspaceRuntime,
		runtime,
		scope,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
	});
	return {
		execute: async (input) =>
			await runtime.execute(WORKBENCH_ACTION_IDS.selectTabByPosition, input, {
				scope,
				source: 'keyboard',
			}).completion,
		projectWorkspaceRuntime,
		runtime,
		scope,
	};
}

function registerTabSelection(
	harness: Harness,
	execute: (
		context: ActionExecutionContext,
		input: TabSelectionInput,
	) => ActionResult | Promise<ActionResult>,
): void {
	const tabScope = harness.runtime.createScope('tab-strip', harness.scope);
	harness.runtime.register(
		tabScope,
		defineAction({
			category: { id: 'tabs', title: { defaultMessage: 'Tabs' } },
			execution: {
				concurrency: 'parallel',
				concurrencyKey: (input) => input.tabId,
				mode: 'foreground',
			},
			getAvailability: () => ({ kind: 'enabled' }),
			handler: { execute },
			id: TAB_STRIP_ACTION_IDS.selectTab,
			input: tabSelectionInput,
			title: { defaultMessage: 'Select Tab' },
		}),
	);
}
