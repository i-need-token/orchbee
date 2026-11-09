import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	type ActionInputValidation,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionResult,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';
import { TAB_STRIP_ACTION_IDS, TabStripOrientation, type TabModel } from '@orchbee/tab-strip';
import { describe, expect, it, vi } from 'vitest';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotTab,
	type FixtureRecord,
} from '../../../__tests__/server-workbench-snapshot';
import type { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { registerWorkbenchActiveTabActions } from '../registerWorkbenchActiveTabActions';
import { registerWorkbenchTabSelectionActions } from '../registerWorkbenchTabSelectionActions';
import type { ActiveTabMovementDecision as TabMoveInput } from '../resolveActiveTabMovement';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../workbench-action-ids';

const FIRST = fixtureTabId(1);
const SECOND = fixtureTabId(2);
const THIRD = fixtureTabId(3);
const FOURTH = fixtureTabId(4);
const PINNED_A = fixtureTabId(5);
const PINNED_B = fixtureTabId(6);

function pinnedTab(sequence: number): FixtureRecord {
	return snapshotTab(sequence, undefined, { pinned: true });
}

describe('registerWorkbenchActiveTabActions', () => {
	it('gives the lower Actions the active Tab, its selection and what follows it', async () => {
		const harness = createHarness(
			[snapshotTab(1), snapshotTab(2), snapshotTab(3), snapshotTab(4)],
			FIRST,
		);
		const model = activeTerminalContext(harness.projectWorkspaceRuntime).tabModel;
		// Adding a Tab to the selection also makes it the active Tab.
		model.toggleSelected(SECOND);
		const lowerInputs = vi.fn<(actionId: string, input: unknown) => void>();
		const tabScope = harness.runtime.createScope('tab-strip', harness.scope);
		for (const [scope, actionId] of [
			[harness.scope, WORKBENCH_ACTION_IDS.createTerminal],
			[tabScope, TAB_STRIP_ACTION_IDS.closeTabs],
			[tabScope, TAB_STRIP_ACTION_IDS.duplicateTabs],
		] as const) {
			harness.runtime.register(
				scope,
				defineAction({
					category: { id: 'tabs', title: { defaultMessage: 'Tabs' } },
					execution: {
						concurrency: 'parallel',
						concurrencyKey: () => actionId,
						mode: 'foreground',
					},
					getAvailability: () => ({ kind: 'enabled' }),
					handler: {
						execute: (context, input) => {
							lowerInputs(actionId, input);
							context.markCommitted();
							return { kind: 'completed' };
						},
					},
					id: actionId,
					input: anyObjectInput,
					title: { defaultMessage: actionId },
				}),
			);
		}

		for (const actionId of [
			WORKBENCH_ACTION_IDS.createTabAfterActive,
			WORKBENCH_ACTION_IDS.duplicateActiveTabs,
			WORKBENCH_ACTION_IDS.closeTabsAfterActive,
		]) {
			await expect(harness.execute(actionId)).resolves.toEqual({ kind: 'completed' });
		}
		expect(lowerInputs.mock.calls).toEqual([
			[WORKBENCH_ACTION_IDS.createTerminal, { groupId: null, index: 2 }],
			[TAB_STRIP_ACTION_IDS.duplicateTabs, { tabIds: [FIRST, SECOND] }],
			[TAB_STRIP_ACTION_IDS.closeTabs, { closeSource: 'non-ui', tabIds: [THIRD, FOURTH] }],
		]);

		model.selectTab(FOURTH);
		expect(
			harness.runtime.resolveAction(WORKBENCH_ACTION_IDS.closeTabsAfterActive, undefined, {
				scope: harness.scope,
				source: 'command-surface',
			}),
		).toMatchObject({ availability: { kind: 'disabled' } });
	});

	it('moves the active Tab circularly through the prepared Tab Strip Action', async () => {
		const harness = createHarness([snapshotTab(1), snapshotTab(2), snapshotTab(3)], FIRST);
		const model = activeTerminalContext(harness.projectWorkspaceRuntime).tabModel;
		const lowerExecute = vi.fn<(input: TabMoveInput) => void>();
		registerTabMovement(harness, (context, input) => {
			lowerExecute(input);
			applyMove(input, model);
			context.markCommitted();
			return { kind: 'completed' };
		});

		await expect(harness.execute(WORKBENCH_ACTION_IDS.moveActiveTabBackward)).resolves.toEqual({
			kind: 'completed',
		});
		expect(lowerExecute).toHaveBeenLastCalledWith({
			direction: 'last',
			tabId: FIRST,
		});
		expect(model.getSnapshot()).toMatchObject({ activeTabId: FIRST });
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([SECOND, THIRD, FIRST]);

		await expect(harness.execute(WORKBENCH_ACTION_IDS.moveActiveTabForward)).resolves.toEqual({
			kind: 'completed',
		});
		expect(lowerExecute).toHaveBeenLastCalledWith({
			direction: 'first',
			tabId: FIRST,
		});
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([FIRST, SECOND, THIRD]);
	});

	it('keeps circular movement inside the active Tab pinned partition', async () => {
		const harness = createHarness([pinnedTab(5), pinnedTab(6), snapshotTab(1)], PINNED_A);
		const model = activeTerminalContext(harness.projectWorkspaceRuntime).tabModel;
		registerTabMovement(harness, (context, input) => {
			applyMove(input, model);
			context.markCommitted();
			return { kind: 'completed' };
		});

		await expect(harness.execute(WORKBENCH_ACTION_IDS.moveActiveTabBackward)).resolves.toEqual({
			kind: 'completed',
		});

		expect(model.getSnapshot()).toMatchObject({ activeTabId: PINNED_A });
		expect(model.getSnapshot().tabs.map((tab) => [tab.id, tab.pinned])).toEqual([
			[PINNED_B, true],
			[PINNED_A, true],
			[FIRST, false],
		]);
	});

	it('is unavailable when the active pin partition has fewer than two Tabs', async () => {
		const harness = createHarness([pinnedTab(5), snapshotTab(1)], FIRST);
		registerTabMovement(harness, () => ({ kind: 'completed' }));

		for (const actionId of [
			WORKBENCH_ACTION_IDS.moveActiveTabBackward,
			WORKBENCH_ACTION_IDS.moveActiveTabForward,
		]) {
			expect(
				harness.runtime.resolveAction(actionId, undefined, {
					scope: harness.scope,
					source: 'keyboard',
				}),
			).toMatchObject({ availability: { kind: 'disabled' } });
			await expect(harness.execute(actionId)).resolves.toMatchObject({
				code: 'action-unavailable',
				kind: 'blocked',
			});
		}
	});

	it('re-resolves the active Tab at execution and blocks after lower disposal', async () => {
		const harness = createHarness([snapshotTab(1), snapshotTab(2), snapshotTab(3)], FIRST);
		const model = activeTerminalContext(harness.projectWorkspaceRuntime).tabModel;
		const lowerExecute = vi.fn<(input: TabMoveInput) => void>();
		const lowerRegistration = registerTabMovement(harness, (context, input) => {
			lowerExecute(input);
			applyMove(input, model);
			context.markCommitted();
			return { kind: 'completed' };
		});

		expect(
			harness.runtime.resolveAction(WORKBENCH_ACTION_IDS.moveActiveTabForward, undefined, {
				scope: harness.scope,
				source: 'keyboard',
			}),
		).toMatchObject({ availability: { kind: 'enabled' } });
		model.selectTab(THIRD);
		await expect(harness.execute(WORKBENCH_ACTION_IDS.moveActiveTabForward)).resolves.toEqual({
			kind: 'completed',
		});
		expect(lowerExecute).toHaveBeenCalledExactlyOnceWith({ direction: 'first', tabId: THIRD });

		lowerRegistration.dispose();
		await expect(
			harness.execute(WORKBENCH_ACTION_IDS.moveActiveTabBackward),
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
	});

	it('gives the host reason only when no surface provides the lower Action', () => {
		const harness = createHarness([snapshotTab(1), snapshotTab(2)], SECOND);
		const ownReason = { defaultMessage: 'Lower owner reason', id: 'test.lower.reason' };
		const reasonOf = (actionId: string) => {
			const resolution = harness.runtime.resolveAction(actionId, undefined, {
				scope: harness.scope,
				source: 'command-surface',
			});
			return resolution?.availability.kind === 'disabled' ? resolution.availability.reason : null;
		};

		// A state of the active Tab: the last Tab has nothing after it to close.
		expect(reasonOf(WORKBENCH_ACTION_IDS.closeTabsAfterActive)).not.toBeNull();
		expect(reasonOf(WORKBENCH_ACTION_IDS.closeTabsAfterActive)).not.toEqual(
			workbenchMessages.actionUnavailable,
		);
		// No surface registers creation, so the host lacks the Action itself.
		expect(reasonOf(WORKBENCH_ACTION_IDS.createTabAfterActive)).toEqual(
			workbenchMessages.actionUnavailable,
		);
		// A lower Action that refuses says why, and the composite repeats its owner's reason.
		harness.runtime.register(
			harness.scope,
			defineAction({
				category: { id: 'tabs', title: { defaultMessage: 'Tabs' } },
				execution: { concurrency: 'parallel', concurrencyKey: () => 'create', mode: 'foreground' },
				getAvailability: () => ({ kind: 'disabled', reason: ownReason }),
				handler: { execute: () => ({ kind: 'completed' }) },
				id: WORKBENCH_ACTION_IDS.createTerminal,
				input: anyObjectInput,
				title: { defaultMessage: 'Create' },
			}),
		);
		expect(reasonOf(WORKBENCH_ACTION_IDS.createTabAfterActive)).toEqual(ownReason);
	});

	it('coexists with adjacent Tab composites during all-Action discovery', () => {
		const harness = createHarness([snapshotTab(1)], FIRST);
		registerWorkbenchTabSelectionActions({
			projectWorkspaceRuntime: harness.projectWorkspaceRuntime,
			runtime: harness.runtime,
			scope: harness.scope,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(harness.runtime),
		});
		const compositeActionIds: readonly string[] = [
			WORKBENCH_ACTION_IDS.moveActiveTabBackward,
			WORKBENCH_ACTION_IDS.moveActiveTabForward,
			WORKBENCH_ACTION_IDS.selectNextTab,
			WORKBENCH_ACTION_IDS.selectPreviousTab,
		];

		expect(
			harness.runtime
				.discover({ scope: harness.scope, source: 'command-surface' }, { includeDescendants: true })
				.filter((result) => compositeActionIds.includes(result.descriptor.id))
				.map((result) => result.descriptor.id),
		).toEqual([
			WORKBENCH_ACTION_IDS.moveActiveTabBackward,
			WORKBENCH_ACTION_IDS.moveActiveTabForward,
			WORKBENCH_ACTION_IDS.selectPreviousTab,
			WORKBENCH_ACTION_IDS.selectNextTab,
		]);
	});
});

interface Harness {
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ReturnType<ActionRuntime['createScope']>;
	execute(
		actionId:
			| typeof WORKBENCH_ACTION_IDS.closeTabsAfterActive
			| typeof WORKBENCH_ACTION_IDS.createTabAfterActive
			| typeof WORKBENCH_ACTION_IDS.duplicateActiveTabs
			| typeof WORKBENCH_ACTION_IDS.moveActiveTabBackward
			| typeof WORKBENCH_ACTION_IDS.moveActiveTabForward,
	): Promise<unknown>;
}

/** One Workspace holding `tabs`, with `activeTabId` active. */
function createHarness(tabs: readonly FixtureRecord[], activeTabId: string): Harness {
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('workbench');
	const projectWorkspaceRuntime = createSnapshotRuntime(oneWorkspaceSnapshot(1, tabs));
	activeTerminalContext(projectWorkspaceRuntime).tabModel.selectTab(activeTabId);
	registerWorkbenchActiveTabActions({
		projectWorkspaceRuntime,
		runtime,
		scope,
		tabStripOrientation: TabStripOrientation.Horizontal,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
	});
	return {
		execute: async (actionId) =>
			await runtime.execute(actionId, undefined, { scope, source: 'keyboard' }).completion,
		projectWorkspaceRuntime,
		runtime,
		scope,
	};
}

function registerTabMovement(
	harness: Harness,
	execute: (
		context: ActionExecutionContext,
		input: TabMoveInput,
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
			id: TAB_STRIP_ACTION_IDS.moveTab,
			input: testTabMoveInput,
			title: { defaultMessage: 'Move Tab' },
		}),
	);
}

function applyMove(input: TabMoveInput, model: TabModel): void {
	switch (input.direction) {
		case 'first':
			model.moveTabFirst(input.tabId);
			break;
		case 'last':
			model.moveTabLast(input.tabId);
			break;
		case 'next':
			model.shiftTabRelative(input.tabId, 1);
			break;
		case 'previous':
			model.shiftTabRelative(input.tabId, -1);
			break;
	}
}

/** These lower stand-ins only record what they were asked to do. */
const anyObjectInput: ActionInputValidation<object> = {
	name: 'AnyObjectInput',
	parse: (input) =>
		typeof input === 'object' && input !== null
			? { data: input, success: true }
			: { code: 'expected-object-input', success: false },
};

const testTabMoveInput: ActionInputValidation<TabMoveInput> = {
	name: 'TabMoveInput',
	parse: (input) => {
		const tabId =
			typeof input === 'object' && input !== null && !Array.isArray(input)
				? Reflect.get(input, 'tabId')
				: undefined;
		if (
			typeof input !== 'object' ||
			input === null ||
			Array.isArray(input) ||
			typeof tabId !== 'string'
		) {
			return { code: 'expected-tab-move-input', success: false };
		}
		const direction = Reflect.get(input, 'direction');
		if (
			direction !== 'first' &&
			direction !== 'last' &&
			direction !== 'next' &&
			direction !== 'previous'
		) {
			return { code: 'expected-tab-move-input', success: false };
		}
		return {
			data: { direction, tabId },
			success: true,
		};
	},
};
