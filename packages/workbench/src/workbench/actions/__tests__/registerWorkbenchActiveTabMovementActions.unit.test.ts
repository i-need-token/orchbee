import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	type ActionInputValidation,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionResult,
} from '@orchbee/actions';
import { TAB_STRIP_ACTION_IDS, type TabModel } from '@orchbee/tab-strip';
import { describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../../model/createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { registerWorkbenchActiveTabMovementActions } from '../registerWorkbenchActiveTabMovementActions';
import { registerWorkbenchTabSelectionActions } from '../registerWorkbenchTabSelectionActions';
import type { ActiveTabMovementDecision as TabMoveInput } from '../resolveActiveTabMovement';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../workbench-action-ids';

describe('registerWorkbenchActiveTabMovementActions', () => {
	it('moves the active Tab circularly through the prepared Tab Strip Action', async () => {
		const harness = createHarness();
		const model = harness.projectWorkspaceRuntime.getActiveTerminalContext().tabModel;
		model.addTab({ pinned: false, title: 'Second' }, { activate: false, id: 'second' });
		model.addTab({ pinned: false, title: 'Third' }, { activate: false, id: 'third' });
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
			tabId: 'terminal-context-1-tab-1',
		});
		expect(model.getSnapshot()).toMatchObject({ activeTabId: 'terminal-context-1-tab-1' });
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'second',
			'third',
			'terminal-context-1-tab-1',
		]);

		await expect(harness.execute(WORKBENCH_ACTION_IDS.moveActiveTabForward)).resolves.toEqual({
			kind: 'completed',
		});
		expect(lowerExecute).toHaveBeenLastCalledWith({
			direction: 'first',
			tabId: 'terminal-context-1-tab-1',
		});
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'terminal-context-1-tab-1',
			'second',
			'third',
		]);
	});

	it('keeps circular movement inside the active Tab pinned partition', async () => {
		const harness = createHarness();
		const model = harness.projectWorkspaceRuntime.getActiveTerminalContext().tabModel;
		model.addTab({ pinned: true, title: 'Pinned A' }, { activate: false, id: 'pinned-a' });
		model.addTab({ pinned: true, title: 'Pinned B' }, { activate: false, id: 'pinned-b' });
		model.selectTab('pinned-a');
		registerTabMovement(harness, (context, input) => {
			applyMove(input, model);
			context.markCommitted();
			return { kind: 'completed' };
		});

		await expect(harness.execute(WORKBENCH_ACTION_IDS.moveActiveTabBackward)).resolves.toEqual({
			kind: 'completed',
		});

		expect(model.getSnapshot()).toMatchObject({ activeTabId: 'pinned-a' });
		expect(model.getSnapshot().tabs.map((tab) => [tab.id, tab.pinned])).toEqual([
			['pinned-b', true],
			['pinned-a', true],
			['terminal-context-1-tab-1', false],
		]);
	});

	it('is unavailable when the active pin partition has fewer than two Tabs', async () => {
		const harness = createHarness();
		const model = harness.projectWorkspaceRuntime.getActiveTerminalContext().tabModel;
		model.addTab({ pinned: true, title: 'Pinned' }, { activate: false, id: 'pinned' });
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
		const harness = createHarness();
		const model = harness.projectWorkspaceRuntime.getActiveTerminalContext().tabModel;
		model.addTab({ pinned: false, title: 'Second' }, { activate: false, id: 'second' });
		model.addTab({ pinned: false, title: 'Third' }, { activate: false, id: 'third' });
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
		model.selectTab('third');
		await expect(harness.execute(WORKBENCH_ACTION_IDS.moveActiveTabForward)).resolves.toEqual({
			kind: 'completed',
		});
		expect(lowerExecute).toHaveBeenCalledExactlyOnceWith({ direction: 'first', tabId: 'third' });

		lowerRegistration.dispose();
		await expect(
			harness.execute(WORKBENCH_ACTION_IDS.moveActiveTabBackward),
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
	});

	it('coexists with adjacent Tab composites during all-Action discovery', () => {
		const harness = createHarness();
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
			| typeof WORKBENCH_ACTION_IDS.moveActiveTabBackward
			| typeof WORKBENCH_ACTION_IDS.moveActiveTabForward,
	): Promise<unknown>;
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
	registerWorkbenchActiveTabMovementActions({
		projectWorkspaceRuntime,
		runtime,
		scope,
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
