import { z } from 'zod';
import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionResult,
} from '@orchbee/actions';
import { TAB_STRIP_ACTION_IDS, type TabSelectionInput } from '@orchbee/tab-strip';
import { describe, expect, it, vi } from 'vitest';
import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixtureId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../__tests__/server-workbench-snapshot';
import type { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { registerWorkbenchTabSelectionActions } from '../registerWorkbenchTabSelectionActions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../workbench-action-ids';

const FIRST_TAB_ID = fixtureTabId(1);
const GROUP_ID = fixtureId(0xc01);
const GROUPED_A_ID = fixtureTabId(2);
const GROUPED_B_ID = fixtureTabId(3);
const PINNED_TAB_ID = fixtureTabId(4);

/** A pinned Tab, the first Tab, and two Tabs in one group, with the first Tab active. */
function groupedSnapshot(): WorkbenchSnapshot {
	return workbenchSnapshot(
		1,
		{ directories: [snapshotDirectory(1)], workspaces: [snapshotWorkspace(1)] },
		[
			snapshotContext(
				1,
				[
					snapshotTab(4, undefined, { label: 'Pinned', pinned: true }),
					snapshotTab(1),
					snapshotTab(2, undefined, { groupId: GROUP_ID, label: 'Grouped A' }),
					snapshotTab(3, undefined, { groupId: GROUP_ID, label: 'Grouped B' }),
				],
				[{ color: 'blue', groupId: GROUP_ID, label: 'Group 1' }],
			),
		],
	);
}

/** The grouped Tabs, with the first Tab active and the group collapsed. */
function createGroupedHarness(): Harness {
	const harness = createHarness(groupedSnapshot());
	const model = activeTerminalContext(harness.projectWorkspaceRuntime).tabModel;
	model.selectTab(FIRST_TAB_ID);
	model.toggleGroupCollapsed(GROUP_ID);
	return harness;
}

describe('registerWorkbenchTabSelectionActions', () => {
	it('selects the one-based TabModel position through the prepared Tab Strip Action', async () => {
		const harness = createGroupedHarness();
		const model = activeTerminalContext(harness.projectWorkspaceRuntime).tabModel;
		const lowerExecute = vi.fn<(input: TabSelectionInput) => void>();
		registerTabSelection(harness, (context, input) => {
			lowerExecute(input);
			model.selectTab(input.tabId);
			context.markCommitted();
			return { kind: 'completed' };
		});

		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			PINNED_TAB_ID,
			FIRST_TAB_ID,
			GROUPED_A_ID,
			GROUPED_B_ID,
		]);
		await expect(harness.execute({ position: 3 })).resolves.toEqual({ kind: 'completed' });

		expect(lowerExecute).toHaveBeenCalledExactlyOnceWith({ mode: 'replace', tabId: GROUPED_A_ID });
		expect(model.getSnapshot()).toMatchObject({ activeTabId: GROUPED_A_ID });
		expect(model.getSnapshot().groups[0]).toMatchObject({ collapsed: false, id: GROUP_ID });
	});

	it('is unavailable when the active terminal context has no Tab at that position', async () => {
		const harness = createHarness(oneWorkspaceSnapshot(1, [snapshotTab(1)]));
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

	it('selects previous and next in canonical order and wraps both boundaries', async () => {
		const harness = createGroupedHarness();
		const model = activeTerminalContext(harness.projectWorkspaceRuntime).tabModel;
		const lowerExecute = vi.fn<(input: TabSelectionInput) => void>();
		registerTabSelection(harness, (context, input) => {
			lowerExecute(input);
			model.selectTab(input.tabId);
			context.markCommitted();
			return { kind: 'completed' };
		});

		await expect(harness.executeAdjacent(WORKBENCH_ACTION_IDS.selectPreviousTab)).resolves.toEqual({
			kind: 'completed',
		});
		await expect(harness.executeAdjacent(WORKBENCH_ACTION_IDS.selectPreviousTab)).resolves.toEqual({
			kind: 'completed',
		});
		await expect(harness.executeAdjacent(WORKBENCH_ACTION_IDS.selectNextTab)).resolves.toEqual({
			kind: 'completed',
		});

		expect(lowerExecute).toHaveBeenNthCalledWith(1, { mode: 'replace', tabId: PINNED_TAB_ID });
		expect(lowerExecute).toHaveBeenNthCalledWith(2, { mode: 'replace', tabId: GROUPED_B_ID });
		expect(lowerExecute).toHaveBeenNthCalledWith(3, { mode: 'replace', tabId: PINNED_TAB_ID });
		expect(model.getSnapshot()).toMatchObject({ activeTabId: PINNED_TAB_ID });
		expect(model.getSnapshot().groups[0]).toMatchObject({ collapsed: false, id: GROUP_ID });
	});

	it('re-resolves the current active Tab at execution and blocks after lower disposal', async () => {
		const harness = createHarness(
			oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2), snapshotTab(3)]),
		);
		const model = activeTerminalContext(harness.projectWorkspaceRuntime).tabModel;
		model.selectTab(FIRST_TAB_ID);
		const lowerExecute = vi.fn<(input: TabSelectionInput) => void>();
		const lowerRegistration = registerTabSelection(harness, (context, input) => {
			lowerExecute(input);
			model.selectTab(input.tabId);
			context.markCommitted();
			return { kind: 'completed' };
		});

		expect(
			harness.runtime.resolveAction(WORKBENCH_ACTION_IDS.selectNextTab, undefined, {
				scope: harness.scope,
				source: 'keyboard',
			}),
		).toMatchObject({ availability: { kind: 'enabled' } });
		model.selectTab(fixtureTabId(2));
		await expect(harness.executeAdjacent(WORKBENCH_ACTION_IDS.selectNextTab)).resolves.toEqual({
			kind: 'completed',
		});
		expect(lowerExecute).toHaveBeenCalledExactlyOnceWith({
			mode: 'replace',
			tabId: fixtureTabId(3),
		});

		lowerRegistration.dispose();
		await expect(
			harness.executeAdjacent(WORKBENCH_ACTION_IDS.selectPreviousTab),
		).resolves.toMatchObject({
			code: 'action-unavailable',
			kind: 'blocked',
		});
	});

	it('leaves adjacent selection unavailable for a singleton Tab model', async () => {
		const harness = createHarness(oneWorkspaceSnapshot(1, [snapshotTab(1)]));
		registerTabSelection(harness, () => ({ kind: 'completed' }));

		for (const actionId of [
			WORKBENCH_ACTION_IDS.selectPreviousTab,
			WORKBENCH_ACTION_IDS.selectNextTab,
		]) {
			expect(
				harness.runtime.resolveAction(actionId, undefined, {
					scope: harness.scope,
					source: 'keyboard',
				}),
			).toMatchObject({ availability: { kind: 'disabled' } });
			await expect(harness.executeAdjacent(actionId)).resolves.toMatchObject({
				code: 'action-unavailable',
				kind: 'blocked',
			});
		}
	});
});

interface Harness {
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ReturnType<ActionRuntime['createScope']>;
	executeAdjacent(
		actionId:
			| typeof WORKBENCH_ACTION_IDS.selectNextTab
			| typeof WORKBENCH_ACTION_IDS.selectPreviousTab,
	): Promise<unknown>;
	execute(input: { readonly position: number }): Promise<unknown>;
}

function createHarness(snapshot: WorkbenchSnapshot): Harness {
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('workbench');
	const projectWorkspaceRuntime = createSnapshotRuntime(snapshot);
	registerWorkbenchTabSelectionActions({
		projectWorkspaceRuntime,
		runtime,
		scope,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
	});
	return {
		executeAdjacent: async (actionId) =>
			await runtime.execute(actionId, undefined, { scope, source: 'keyboard' }).completion,
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
): ActionDisposable {
	const tabScope = harness.runtime.createScope('tab-strip', harness.scope);
	return harness.runtime.register(
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
			input: {
				name: 'WorkbenchActionFixture',
				parse(input) {
					const parsed = z
						.object({
							tabId: z.string(),
							mode: z.enum(['add-range', 'extend', 'replace', 'toggle']),
						})
						.safeParse(input);
					return parsed.success
						? { success: true, data: parsed.data }
						: { success: false, code: 'invalid-fixture-input' };
				},
			},
			title: { defaultMessage: 'Select Tab' },
		}),
	);
}
