import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionResult,
} from '@orchbee/actions';
import { describe, expect, it, vi } from 'vitest';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixturePanelId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotSplit,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import {
	PANEL_ACTION_IDS,
	panelCloseInput,
	panelSwapInput,
	panelTargetInput,
	panelVisualSelectionInput,
	type PanelCloseInput,
	type PanelSwapInput,
	type PanelTargetInput,
	type PanelVisualSelectionInput,
} from '../../components/workbench-panel-workspace/actions/panel-action-contracts';
import type { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { registerWorkbenchActiveTerminalActions } from '../registerWorkbenchActiveTerminalActions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../workbench-action-ids';

describe('registerWorkbenchActiveTerminalActions', () => {
	it.each([
		{ expected: fixturePanelId(1), panels: 1 as const },
		{ expected: fixturePanelId(2), panels: 2 as const },
	])(
		'executes the prepared Panel close Action with $panels Panel(s) in the Tab',
		async ({ expected, panels }) => {
			const harness = createHarness(panels);
			const lowerExecute = vi.fn<(input: PanelCloseInput) => void>();
			registerPanelClose(harness, (context, input) => {
				lowerExecute(input);
				context.markCommitted();
				return { kind: 'completed' };
			});

			await expect(harness.execute(WORKBENCH_ACTION_IDS.closeActiveTerminal)).resolves.toEqual({
				kind: 'completed',
			});
			expect(lowerExecute).toHaveBeenCalledExactlyOnceWith({ panelId: expected, tabId: TAB_ID });
		},
	);

	it('preserves lower Action confirmation cancellation without calling its handler', async () => {
		const harness = createHarness(1, () => false);
		const lowerExecute = vi.fn<() => void>();
		registerPanelClose(
			harness,
			() => {
				lowerExecute();
				return { kind: 'completed' };
			},
			true,
		);

		await expect(harness.execute(WORKBENCH_ACTION_IDS.closeActiveTerminal)).resolves.toEqual({
			kind: 'cancelled',
		});
		expect(lowerExecute).not.toHaveBeenCalled();
	});

	it('propagates cancellation to a running prepared lower Action', async () => {
		const harness = createHarness();
		let lowerWasAborted = false;
		let notifyStarted: (() => void) | undefined;
		const started = new Promise<void>((resolve) => {
			notifyStarted = resolve;
		});
		registerPanelClose(harness, async (context) => {
			notifyStarted?.();
			await new Promise<void>((resolve) => {
				context.signal.addEventListener(
					'abort',
					() => {
						lowerWasAborted = context.signal.aborted;
						resolve();
					},
					{ once: true },
				);
			});
			return { kind: 'cancelled' };
		});

		const execution = harness.runtime.execute(WORKBENCH_ACTION_IDS.closeActiveTerminal, undefined, {
			scope: harness.scope,
			source: 'keyboard',
		});
		await started;
		execution.cancel();

		await expect(execution.completion).resolves.toEqual({ kind: 'cancelled' });
		expect(lowerWasAborted).toBe(true);
	});

	it('keeps maximization unavailable for a singleton active panel', () => {
		const harness = createHarness();
		expect(
			harness.runtime.resolveAction(WORKBENCH_ACTION_IDS.toggleActivePanelMaximization, undefined, {
				scope: harness.scope,
				source: 'keyboard',
			}),
		).toMatchObject({ availability: { kind: 'disabled' } });
	});

	it('executes the prepared resize-mode Action for the active panel', async () => {
		const harness = createHarness();
		const lowerExecute = vi.fn<(input: PanelTargetInput) => void>();
		registerPanelTargetAction(harness, PANEL_ACTION_IDS.startResize, (context, input) => {
			lowerExecute(input);
			context.markCommitted();
			return { kind: 'completed' };
		});

		await expect(harness.execute(WORKBENCH_ACTION_IDS.resizeActivePanel)).resolves.toEqual({
			kind: 'completed',
		});
		expect(lowerExecute).toHaveBeenCalledExactlyOnceWith({ panelId: fixturePanelId(1) });
	});

	it('routes equalization to the exact active Panel and preserves an unchanged result', async () => {
		const harness = createHarness(2);
		const lowerExecute = vi.fn<(input: PanelTargetInput) => void>();
		registerPanelTargetAction(harness, PANEL_ACTION_IDS.equalize, (_context, input) => {
			lowerExecute(input);
			return { kind: 'unchanged' };
		});

		await expect(harness.execute(WORKBENCH_ACTION_IDS.equalizeActivePanels)).resolves.toEqual({
			kind: 'unchanged',
		});
		expect(lowerExecute).toHaveBeenCalledExactlyOnceWith({ panelId: fixturePanelId(2) });
	});

	it('keeps equalization unavailable for singleton and maximized active Tabs', () => {
		const harness = createHarness();
		const invocation = {
			scope: harness.scope,
			source: 'keyboard' as const,
		};
		expect(
			harness.runtime.resolveAction(
				WORKBENCH_ACTION_IDS.equalizeActivePanels,
				undefined,
				invocation,
			),
		).toMatchObject({ availability: { kind: 'disabled' } });

		const split = createHarness(2);
		activeTerminalContext(split.projectWorkspaceRuntime).panelWorkspaceStore.transition({
			panelId: fixturePanelId(2),
			tabId: TAB_ID,
			type: 'panel-maximization-toggled',
		});
		expect(
			split.runtime.resolveAction(WORKBENCH_ACTION_IDS.equalizeActivePanels, undefined, {
				...invocation,
				scope: split.scope,
			}),
		).toMatchObject({ availability: { kind: 'disabled' } });
	});

	it('executes the prepared focus Action for a directional neighbor', async () => {
		const harness = createHarness(2);
		const lowerExecute = vi.fn<(input: PanelTargetInput) => void>();
		registerPanelFocus(harness, (context, input) => {
			lowerExecute(input);
			context.markCommitted();
			return { kind: 'completed' };
		});

		await expect(harness.execute(WORKBENCH_ACTION_IDS.focusActivePanelLeft)).resolves.toEqual({
			kind: 'completed',
		});
		expect(lowerExecute).toHaveBeenCalledExactlyOnceWith({ panelId: fixturePanelId(1) });
	});

	it('keeps an outer-edge directional focus Action unavailable', () => {
		const harness = createHarness();
		expect(
			harness.runtime.resolveAction(WORKBENCH_ACTION_IDS.focusActivePanelLeft, undefined, {
				scope: harness.scope,
				source: 'keyboard',
			}),
		).toMatchObject({ availability: { kind: 'disabled' } });
	});

	it('cycles focus through the prepared exact Panel Action and revalidates availability', async () => {
		const harness = createHarness();
		const invocation = {
			scope: harness.scope,
			source: 'keyboard' as const,
		};
		expect(
			harness.runtime.resolveAction(
				WORKBENCH_ACTION_IDS.focusActivePanelNext,
				undefined,
				invocation,
			),
		).toMatchObject({ availability: { kind: 'disabled' } });

		const split = createHarness(2);
		const lowerExecute = vi.fn<(input: PanelTargetInput) => void>();
		registerPanelFocus(split, (context, input) => {
			lowerExecute(input);
			context.markCommitted();
			return { kind: 'completed' };
		});

		await expect(split.execute(WORKBENCH_ACTION_IDS.focusActivePanelNext)).resolves.toEqual({
			kind: 'completed',
		});
		expect(lowerExecute).toHaveBeenCalledExactlyOnceWith({ panelId: fixturePanelId(1) });

		activeTerminalContext(split.projectWorkspaceRuntime).panelWorkspaceStore.transition({
			panelId: fixturePanelId(1),
			tabId: TAB_ID,
			type: 'panel-maximization-toggled',
		});
		expect(
			split.runtime.resolveAction(WORKBENCH_ACTION_IDS.focusActivePanelPrevious, undefined, {
				...invocation,
				scope: split.scope,
			}),
		).toMatchObject({ availability: { kind: 'disabled' } });
	});

	it('focuses an exact canonical Panel position and preserves a current-target unchanged result', async () => {
		const harness = createHarness(3);
		const lowerExecute = vi.fn<(input: PanelTargetInput) => void>();
		registerPanelFocus(harness, (context, input) => {
			lowerExecute(input);
			if (input.panelId === fixturePanelId(3)) return { kind: 'unchanged' };
			context.markCommitted();
			return { kind: 'completed' };
		});

		await expect(
			harness.execute(WORKBENCH_ACTION_IDS.focusActivePanelByPosition, { position: 1 }),
		).resolves.toEqual({ kind: 'completed' });
		await expect(
			harness.execute(WORKBENCH_ACTION_IDS.focusActivePanelByPosition, { position: 3 }),
		).resolves.toEqual({ kind: 'unchanged' });
		expect(lowerExecute).toHaveBeenNthCalledWith(1, { panelId: fixturePanelId(1) });
		expect(lowerExecute).toHaveBeenNthCalledWith(2, { panelId: fixturePanelId(3) });
	});

	it('keeps missing, maximized, stale-source, and unregistered positional focus unavailable', () => {
		const harness = createHarness(2);
		const store = activeTerminalContext(harness.projectWorkspaceRuntime).panelWorkspaceStore;
		const lowerRegistration = registerPanelFocus(harness, () => ({ kind: 'unchanged' }));
		const invocation = { scope: harness.scope, source: 'keyboard' as const };

		expect(
			harness.runtime.resolveAction(
				WORKBENCH_ACTION_IDS.focusActivePanelByPosition,
				{ position: 2 },
				invocation,
			),
		).toMatchObject({ availability: { kind: 'enabled' } });
		expect(
			harness.runtime.resolveAction(
				WORKBENCH_ACTION_IDS.focusActivePanelByPosition,
				{ position: 3 },
				invocation,
			),
		).toMatchObject({ availability: { kind: 'disabled' } });
		expect(
			harness.runtime.resolveAction(
				WORKBENCH_ACTION_IDS.focusActivePanelByPosition,
				{ position: 1 },
				{ ...invocation, target: { id: 'missing-panel', type: 'panel' } },
			),
		).toMatchObject({ availability: { kind: 'disabled' } });

		store.transition({
			panelId: fixturePanelId(2),
			tabId: TAB_ID,
			type: 'panel-maximization-toggled',
		});
		expect(
			harness.runtime.resolveAction(
				WORKBENCH_ACTION_IDS.focusActivePanelByPosition,
				{ position: 1 },
				invocation,
			),
		).toMatchObject({ availability: { kind: 'disabled' } });
		store.transition({
			panelId: fixturePanelId(2),
			tabId: TAB_ID,
			type: 'panel-maximization-toggled',
		});

		lowerRegistration.dispose();
		expect(
			harness.runtime.resolveAction(
				WORKBENCH_ACTION_IDS.focusActivePanelByPosition,
				{ position: 1 },
				invocation,
			),
		).toMatchObject({ availability: { kind: 'disabled' } });
	});

	it('directly focuses or swaps the sole other visible Panel through lower Actions', async () => {
		const harness = createHarness(2);
		const focus = vi.fn<(input: PanelTargetInput) => void>();
		const swap = vi.fn<(input: PanelSwapInput) => void>();
		registerPanelFocus(harness, (context, input) => {
			focus(input);
			context.markCommitted();
			return { kind: 'completed' };
		});
		registerPanelSwap(harness, (context, input) => {
			swap(input);
			context.markCommitted();
			return { kind: 'completed' };
		});

		await expect(harness.execute(WORKBENCH_ACTION_IDS.focusActivePanelVisible)).resolves.toEqual({
			kind: 'completed',
		});
		await expect(harness.execute(WORKBENCH_ACTION_IDS.swapActivePanelVisible)).resolves.toEqual({
			kind: 'completed',
		});
		expect(focus).toHaveBeenCalledExactlyOnceWith({ panelId: fixturePanelId(1) });
		expect(swap).toHaveBeenCalledExactlyOnceWith({
			panelId: fixturePanelId(2),
			targetPanelId: fixturePanelId(1),
		});
	});

	it('starts lower visual selection with the exact active Panel when three are visible', async () => {
		const harness = createHarness(3);
		const start = vi.fn<(input: PanelVisualSelectionInput) => void>();
		registerPanelVisualSelection(harness, (context, input) => {
			start(input);
			context.markCommitted();
			return { kind: 'completed' };
		});

		await expect(harness.execute(WORKBENCH_ACTION_IDS.focusActivePanelVisible)).resolves.toEqual({
			kind: 'completed',
		});
		await expect(harness.execute(WORKBENCH_ACTION_IDS.swapActivePanelVisible)).resolves.toEqual({
			kind: 'completed',
		});
		expect(start).toHaveBeenNthCalledWith(1, { mode: 'focus', panelId: fixturePanelId(3) });
		expect(start).toHaveBeenNthCalledWith(2, { mode: 'swap', panelId: fixturePanelId(3) });
	});

	it('executes the prepared Panel swap Action for a directional neighbor', async () => {
		const harness = createHarness(2);
		const lowerExecute = vi.fn<(input: PanelSwapInput) => void>();
		const panelScope = harness.runtime.createScope('panel-workspace', harness.scope);
		harness.runtime.register(
			panelScope,
			defineAction({
				category: { id: 'panels', title: { defaultMessage: 'Panels' } },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: (input) => input.panelId,
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (context, input) => {
						lowerExecute(input);
						context.markCommitted();
						return { kind: 'completed' };
					},
				},
				id: PANEL_ACTION_IDS.swap,
				input: panelSwapInput,
				title: { defaultMessage: 'Move panel' },
			}),
		);

		await expect(harness.execute(WORKBENCH_ACTION_IDS.moveActivePanelLeft)).resolves.toEqual({
			kind: 'completed',
		});
		expect(lowerExecute).toHaveBeenCalledExactlyOnceWith({
			panelId: fixturePanelId(2),
			targetPanelId: fixturePanelId(1),
		});
	});

	it('keeps an outer-edge directional movement Action unavailable', () => {
		const harness = createHarness();
		expect(
			harness.runtime.resolveAction(WORKBENCH_ACTION_IDS.moveActivePanelLeft, undefined, {
				scope: harness.scope,
				source: 'keyboard',
			}),
		).toMatchObject({ availability: { kind: 'disabled' } });
	});
});

interface Harness {
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ReturnType<ActionRuntime['createScope']>;
	execute(actionId: string, input?: unknown): Promise<unknown>;
}

const TAB_ID = fixtureTabId(1);

/** One Tab of `panelCount` Panels side by side, as splitting rightward leaves it, the last focused. */
function createHarness(panelCount: 1 | 2 | 3 = 1, confirm?: () => boolean): Harness {
	const runtime = new ActionRuntime(confirm === undefined ? {} : { confirm });
	const scope = runtime.createScope('workbench');
	const root =
		panelCount === 1
			? snapshotPanel(1)
			: snapshotSplit(
					1,
					'horizontal',
					snapshotPanel(1),
					panelCount === 2
						? snapshotPanel(2)
						: snapshotSplit(2, 'horizontal', snapshotPanel(2), snapshotPanel(3)),
				);
	const projectWorkspaceRuntime = createSnapshotRuntime(
		oneWorkspaceSnapshot(1, [snapshotTab(1, root)]),
	);
	activeTerminalContext(projectWorkspaceRuntime).panelWorkspaceStore.transition({
		panelId: fixturePanelId(panelCount),
		tabId: TAB_ID,
		type: 'panel-focused',
	});
	registerWorkbenchActiveTerminalActions({
		projectWorkspaceRuntime,
		runtime,
		scope,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
	});
	return {
		execute: async (actionId, input) =>
			await runtime.execute(actionId, input, { scope, source: 'keyboard' }).completion,
		projectWorkspaceRuntime,
		runtime,
		scope,
	};
}

function registerPanelClose(
	harness: Harness,
	execute: (
		context: ActionExecutionContext,
		input: PanelCloseInput,
	) => ActionResult | Promise<ActionResult>,
	withConfirmation = false,
): void {
	const panelScope = harness.runtime.createScope('panel-workspace', harness.scope);
	harness.runtime.register(
		panelScope,
		defineAction({
			category: { id: 'panels', title: { defaultMessage: 'Panels' } },
			...(withConfirmation
				? {
						confirmation: () => ({
							confirmLabel: { defaultMessage: 'Close' },
							description: { defaultMessage: 'Running' },
							risk: 'destructive' as const,
							title: { defaultMessage: 'Close?' },
						}),
					}
				: {}),
			execution: {
				concurrency: 'parallel',
				concurrencyKey: (input) => input.panelId,
				mode: 'foreground',
			},
			getAvailability: () => ({ kind: 'enabled' }),
			handler: { execute },
			id: PANEL_ACTION_IDS.close,
			input: panelCloseInput,
			title: { defaultMessage: 'Close panel' },
		}),
	);
}

function registerPanelFocus(
	harness: Harness,
	execute: (
		context: ActionExecutionContext,
		input: PanelTargetInput,
	) => ActionResult | Promise<ActionResult>,
): ActionDisposable {
	return registerPanelTargetAction(harness, PANEL_ACTION_IDS.focus, execute);
}

function registerPanelSwap(
	harness: Harness,
	execute: (
		context: ActionExecutionContext,
		input: PanelSwapInput,
	) => ActionResult | Promise<ActionResult>,
): void {
	const panelScope = harness.runtime.createScope('panel-workspace', harness.scope);
	harness.runtime.register(
		panelScope,
		defineAction({
			category: { id: 'panels', title: { defaultMessage: 'Panels' } },
			execution: {
				concurrency: 'parallel',
				concurrencyKey: (input) => input.panelId,
				mode: 'foreground',
			},
			getAvailability: () => ({ kind: 'enabled' }),
			handler: { execute },
			id: PANEL_ACTION_IDS.swap,
			input: panelSwapInput,
			title: { defaultMessage: 'Swap panel' },
		}),
	);
}

function registerPanelVisualSelection(
	harness: Harness,
	execute: (
		context: ActionExecutionContext,
		input: PanelVisualSelectionInput,
	) => ActionResult | Promise<ActionResult>,
): void {
	const panelScope = harness.runtime.createScope('panel-workspace', harness.scope);
	harness.runtime.register(
		panelScope,
		defineAction({
			category: { id: 'panels', title: { defaultMessage: 'Panels' } },
			execution: {
				concurrency: 'parallel',
				concurrencyKey: (input) => input.panelId,
				mode: 'foreground',
			},
			getAvailability: () => ({ kind: 'enabled' }),
			handler: { execute },
			id: PANEL_ACTION_IDS.startVisualSelection,
			input: panelVisualSelectionInput,
			title: { defaultMessage: 'Select panel visually' },
		}),
	);
}

function registerPanelTargetAction(
	harness: Harness,
	actionId:
		| typeof PANEL_ACTION_IDS.equalize
		| typeof PANEL_ACTION_IDS.focus
		| typeof PANEL_ACTION_IDS.startResize,
	execute: (
		context: ActionExecutionContext,
		input: PanelTargetInput,
	) => ActionResult | Promise<ActionResult>,
): ActionDisposable {
	const panelScope = harness.runtime.createScope('panel-workspace', harness.scope);
	return harness.runtime.register(
		panelScope,
		defineAction({
			category: { id: 'panels', title: { defaultMessage: 'Panels' } },
			execution: {
				concurrency: 'parallel',
				concurrencyKey: (input) => input.panelId,
				mode: 'foreground',
			},
			getAvailability: () => ({ kind: 'enabled' }),
			handler: { execute },
			id: actionId,
			input: panelTargetInput,
			title: { defaultMessage: 'Focus panel' },
		}),
	);
}
