// @vitest-environment jsdom

import {
	ActionRuntime,
	type ActionAvailability,
	type ActionExecution,
	type ActionScope,
} from '@orchbee/actions';
import { cleanup, render } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { TAB_STRIP_ACTION_IDS } from '../../actions/tab-strip-action-contracts';
import { DEFAULT_TAB_STRIP_MESSAGES } from '../../constants/default-tab-strip-messages';
import { TabModel } from '../../model/tab-model';
import { TabStripComboButtonController } from '../../model/TabStripComboButtonController';
import type {
	TabStripChangeCallback,
	TabStripCloseCallback,
	TabStripCloseConfirmation,
	TabStripNewTabRequest,
	TabStripSelectCallback,
	TabStripTabCreationIntent,
} from '../../types/tab-strip-options';
import type { TabStripActions } from '../../types/tab-strip-actions';
import { useTabStripActions } from '../useTabStripActions';

interface HarnessProps {
	readonly captureActions?: ((actions: TabStripActions) => void) | undefined;
	readonly captureScope: (scope: ActionScope) => void;
	readonly comboButtonController?: TabStripComboButtonController | undefined;
	readonly model: TabModel;
	readonly getCloseTabsConfirmation?: TabStripCloseConfirmation | undefined;
	readonly getTabCreationAvailability?:
		| ((intent: TabStripTabCreationIntent) => ActionAvailability)
		| undefined;
	readonly onCloseTabs?: TabStripCloseCallback | undefined;
	readonly onNewTab?: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly onSelectTab?: TabStripSelectCallback | undefined;
	readonly onTabsChange?: TabStripChangeCallback | undefined;
	readonly parentScope?: ActionScope | undefined;
	readonly runtime?: ActionRuntime | undefined;
}

function Harness({
	captureActions,
	captureScope,
	comboButtonController = new TabStripComboButtonController(),
	model,
	getCloseTabsConfirmation,
	getTabCreationAvailability,
	onCloseTabs,
	onNewTab,
	onSelectTab,
	onTabsChange,
	parentScope,
	runtime,
}: HarnessProps): ReactNode {
	const actions = useTabStripActions({
		actionRuntime: runtime,
		comboButtonController,
		messages: DEFAULT_TAB_STRIP_MESSAGES,
		model,
		getCloseTabsConfirmation,
		getTabCreationAvailability,
		onMoveGroupToNewWindow: undefined,
		onMoveTabsToNewWindow: undefined,
		onNewTab,
		onCloseTabs,
		onSelectTab,
		onTabsChange,
		parentActionScope: parentScope,
		requestCloseTabs: (tabIds) => {
			model.closeTabs(tabIds);
		},
		onVerticalCollapsedChange: undefined,
		onVerticalExpandOnHoverChange: undefined,
		onVerticalTabSearchClick: undefined,
		verticalCollapsed: false,
		verticalExpandOnHover: true,
	});
	captureActions?.(actions);
	captureScope(actions.scope);
	return null;
}

afterEach(() => cleanup());

describe('useTabStripActions', () => {
	it('retires fallback executions after switching to an injected runtime', async () => {
		const signals: AbortSignal[] = [];
		const external = new ActionRuntime();
		const model = new TabModel({ tabs: [{ id: 'tab', title: 'Tab', pinned: false, group: null }] });
		let actions: TabStripActions | undefined;
		let releaseClose: (() => void) | undefined;
		const closeGate = new Promise<void>((resolve) => {
			releaseClose = resolve;
		});
		const props: HarnessProps = {
			captureActions: (value) => {
				actions = value;
			},
			captureScope: () => undefined,
			model,
			onCloseTabs: async (context) => {
				signals.push(context.signal);
				await closeGate;
				return { kind: 'completed' };
			},
		};
		const rendered = render(createElement(Harness, props));
		if (actions === undefined || releaseClose === undefined)
			throw new Error('Expected fallback close fixtures');
		const execution = actions.execute(
			TAB_STRIP_ACTION_IDS.closeTabs,
			{ closeSource: 'mouse', tabIds: ['tab'] },
			'pointer',
		);
		try {
			const signal = signals[0];
			if (signal === undefined)
				throw new Error('Expected the admitted handler cancellation signal');
			expect(execution.getSnapshot().state).toBe('running');
			rendered.rerender(createElement(Harness, { ...props, runtime: external }));
			await Promise.resolve();
			expect(signal.aborted).toBe(false);
			rendered.unmount();
			await Promise.resolve();
			expect(signal.aborted).toBe(true);
			expect(execution.getSnapshot().state).toBe('cancelling');
			releaseClose();
			await expect(execution.completion).resolves.toEqual({ kind: 'cancelled' });
			expect(() => external.createScope('still-live')).not.toThrow();
		} finally {
			rendered.unmount();
			releaseClose();
			await execution.completion;
			external.dispose();
		}
	});

	it('asks once to close Tabs whose close is still running', async () => {
		const model = new TabModel({ tabs: [{ id: 'tab', title: 'Tab', pinned: false, group: null }] });
		let actions: TabStripActions | undefined;
		let releaseClose: (() => void) | undefined;
		const closeGate = new Promise<void>((resolve) => {
			releaseClose = resolve;
		});
		const onCloseTabs = vi.fn<TabStripCloseCallback>(async () => {
			await closeGate;
			return { kind: 'completed' };
		});
		render(
			createElement(Harness, {
				captureActions: (value) => {
					actions = value;
				},
				captureScope: () => undefined,
				model,
				onCloseTabs,
			}),
		);
		if (actions === undefined) throw new Error('Expected the Tab strip actions');
		const input = { closeSource: 'mouse', tabIds: ['tab'] } as const;

		const first = actions.execute(TAB_STRIP_ACTION_IDS.closeTabs, input, 'pointer');
		const second = actions.execute(TAB_STRIP_ACTION_IDS.closeTabs, input, 'pointer');

		await expect(second.completion).resolves.toEqual({
			code: 'duplicate-action-execution',
			kind: 'blocked',
		});
		releaseClose?.();
		await expect(first.completion).resolves.toEqual({ kind: 'completed' });
		expect(onCloseTabs).toHaveBeenCalledOnce();
	});

	it('keeps its fallback runtime usable across injected runtime replacement', async () => {
		const external = new ActionRuntime();
		const model = new TabModel();
		let actions: TabStripActions | undefined;
		const props: HarnessProps = {
			captureActions: (value) => {
				actions = value;
			},
			captureScope: () => undefined,
			model,
		};
		const rendered = render(createElement(Harness, props));
		try {
			rendered.rerender(createElement(Harness, { ...props, runtime: external }));
			await Promise.resolve();
			rendered.rerender(createElement(Harness, props));
			await Promise.resolve();
			if (actions === undefined) throw new Error('Expected fallback action fixtures');
			await expect(
				actions.createTab({ groupId: null, title: 'Fallback' }, 'button').completion,
			).resolves.toEqual({ kind: 'completed' });
			expect(model.getSnapshot().tabs.map((tab) => tab.title)).toEqual(['Fallback']);
		} finally {
			rendered.unmount();
			external.dispose();
		}
	});

	it('blocks an invalid arrangement without corrupting the model', async () => {
		const runtime = new ActionRuntime();
		const model = new TabModel({
			tabs: [
				{ id: 'p', title: 'Pinned', pinned: true, group: null },
				{ id: 'a', title: 'Unpinned', pinned: false, group: null },
			],
		});
		let scope: ActionScope | undefined;
		const rendered = render(
			createElement(Harness, {
				captureScope: (value) => {
					scope = value;
				},
				model,
				parentScope: runtime.createScope('workbench'),
				runtime,
			}),
		);
		try {
			if (scope === undefined) throw new Error('Expected mounted action scope');
			const before = model.getSnapshot();
			const state = model.captureDragRestoreState();
			await expect(
				runtime.execute(
					TAB_STRIP_ACTION_IDS.applyArrangement,
					{
						before: state,
						state: { ...state, tabs: [...state.tabs].reverse() },
						tabIds: ['p'],
					},
					{ scope, source: 'programmatic' },
				).completion,
			).resolves.toEqual({
				kind: 'blocked',
				code: 'invalid-tab-arrangement',
			});
			expect(model.getSnapshot()).toBe(before);
			expect(model.captureDragRestoreState()).toEqual(state);
			expect(model.getIndexOfTab('p')).toBe(0);
			expect(() => model.updateTab('a', { title: 'Still usable' })).not.toThrow();
		} finally {
			rendered.unmount();
			runtime.dispose();
		}
	});

	describe('when an authority decides the Tabs', () => {
		const orderOf = (model: TabModel) => model.getSnapshot().tabs.map((tab) => tab.id);

		function mounted(onTabsChange: TabStripChangeCallback, model: TabModel) {
			let actions: TabStripActions | undefined;
			render(
				createElement(Harness, {
					captureActions: (value) => {
						actions = value;
					},
					captureScope: () => undefined,
					model,
					onTabsChange,
				}),
			);
			if (actions === undefined) throw new Error('Expected the Tab strip actions');
			return actions;
		}

		/** Where a pointer release leaves things: the model as it was, and where the Tabs ended up. */
		function releaseFirstPastSecond(model: TabModel) {
			const before = model.captureDragRestoreState();
			model.moveTabsByDrag(['a'], 2, 0, 3, null, { coalesceNonconsecutive: true });
			const state = model.captureDragRestoreState();
			model.restoreCanceledDrag(before);
			return { before, state, tabIds: ['a'] };
		}

		const threeTabs = () =>
			new TabModel({
				tabs: [
					{ group: null, id: 'a', pinned: false, title: 'A' },
					{ group: null, id: 'b', pinned: false, title: 'B' },
					{ group: null, id: 'c', pinned: false, title: 'C' },
				],
			});

		it('asks nothing of a drag that ended where it started', async () => {
			const onTabsChange = vi.fn<TabStripChangeCallback>(() => ({ kind: 'completed' }));
			const model = threeTabs();
			const actions = mounted(onTabsChange, model);
			const before = model.captureDragRestoreState();

			await expect(
				actions.execute(
					TAB_STRIP_ACTION_IDS.applyArrangement,
					{ before, state: before, tabIds: ['a'] },
					'pointer',
				).completion,
			).resolves.toEqual({ kind: 'unchanged' });

			expect(onTabsChange).not.toHaveBeenCalled();
		});

		it('keeps a dragged arrangement on screen while the authority decides it', async () => {
			let answer: (result: { readonly kind: 'completed' }) => void = () => undefined;
			const onTabsChange = vi.fn<TabStripChangeCallback>(
				() =>
					new Promise((resolve) => {
						answer = resolve;
					}),
			);
			const model = threeTabs();
			const actions = mounted(onTabsChange, model);

			const execution = actions.execute(
				TAB_STRIP_ACTION_IDS.applyArrangement,
				releaseFirstPastSecond(model),
				'pointer',
			);

			expect(orderOf(model)).toEqual(['b', 'a', 'c']);
			expect(onTabsChange).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
				groupId: null,
				insertIndex: 2,
				kind: 'tabs-dragged',
				sourceOrder: ['a', 'b', 'c'],
				tabIds: ['a'],
			});
			answer({ kind: 'completed' });
			await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
			expect(orderOf(model)).toEqual(['b', 'a', 'c']);
		});

		it('returns refused Tabs and keeps what was selected while waiting', async () => {
			let refuse: (result: { readonly code: string; readonly kind: 'blocked' }) => void = () =>
				undefined;
			const model = threeTabs();
			const actions = mounted(
				() =>
					new Promise((resolve) => {
						refuse = resolve;
					}),
				model,
			);

			const execution = actions.execute(
				TAB_STRIP_ACTION_IDS.applyArrangement,
				releaseFirstPastSecond(model),
				'pointer',
			);
			model.selectTab('c');
			refuse({ code: 'revision-mismatch', kind: 'blocked' });

			await expect(execution.completion).resolves.toEqual({
				code: 'revision-mismatch',
				kind: 'blocked',
			});
			expect(orderOf(model)).toEqual(['a', 'b', 'c']);
			expect(model.getSnapshot().activeTabId).toBe('c');
		});

		it('says when a move would land on a collapsed group, and leaves its model alone', async () => {
			const onTabsChange = vi.fn<TabStripChangeCallback>(() => ({ kind: 'completed' }));
			const model = new TabModel({
				groups: [{ collapsed: true, color: 'blue', id: 'g', title: 'G' }],
				tabs: [
					{ group: null, id: 'a', pinned: false, title: 'A' },
					{ group: 'g', id: 'b', pinned: false, title: 'B' },
				],
			});
			const actions = mounted(onTabsChange, model);

			await actions.execute(
				TAB_STRIP_ACTION_IDS.moveTab,
				{ direction: 'next', tabId: 'a' },
				'pointer',
			).completion;

			expect(onTabsChange).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
				adjacentGroupCollapsed: true,
				direction: 'next',
				kind: 'tab-moved',
				tabId: 'a',
			});
			expect(orderOf(model)).toEqual(['a', 'b']);
		});
	});

	it.each([
		{ interruption: 'observer fault', nestedResult: undefined },
		{ interruption: 'runtime disposal', nestedResult: undefined },
		{ interruption: 'registration retirement', nestedResult: undefined },
		{
			interruption: 'nested duplicate',
			nestedResult: { kind: 'blocked', code: 'duplicate-action-execution' },
		},
	])(
		'keeps the committed arrangement after synchronous $interruption',
		async ({ interruption, nestedResult }) => {
			const runtime = new ActionRuntime();
			const model = new TabModel({
				tabs: [
					{ id: 'a', title: 'First', pinned: false, group: null },
					{ id: 'b', title: 'Second', pinned: false, group: null },
				],
			});
			let scope: ActionScope | undefined;
			const rendered = render(
				createElement(Harness, {
					captureScope: (value) => {
						scope = value;
					},
					model,
					parentScope: runtime.createScope('workbench'),
					runtime,
				}),
			);
			try {
				if (scope === undefined) throw new Error('Expected mounted action scope');
				const invocation = { scope, source: 'programmatic' as const };
				const before = model.getSnapshot();
				const state = model.captureDragRestoreState();
				let nested: ActionExecution | undefined;
				model.subscribe(() => {
					switch (interruption) {
						case 'observer fault':
							throw new Error('observer failed');
						case 'runtime disposal':
							runtime.dispose();
							break;
						case 'registration retirement':
							rendered.unmount();
							break;
						case 'nested duplicate':
							nested = runtime.execute(
								TAB_STRIP_ACTION_IDS.applyArrangement,
								{ before: state, state, tabIds: ['a'] },
								invocation,
							);
							break;
					}
				});
				const healthyObserver = vi.fn<() => void>();
				model.subscribe(healthyObserver);
				await expect(
					runtime.execute(
						TAB_STRIP_ACTION_IDS.applyArrangement,
						{
							before: state,
							state: { ...state, tabs: [...state.tabs].reverse() },
							tabIds: ['a'],
						},
						invocation,
					).completion,
				).resolves.toEqual({ kind: 'completed' });
				expect(model.getSnapshot().revision).toBe(before.revision + 1);
				expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['b', 'a']);
				expect(healthyObserver).toHaveBeenCalledOnce();
				expect(await nested?.completion).toEqual(nestedResult);
			} finally {
				rendered.unmount();
				runtime.dispose();
			}
		},
	);

	it('duplicates the admitted canonical source set without activating or selecting the new tabs', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'b',
			selectedTabIds: ['a', 'b'],
			groups: [{ id: 'group', title: 'Group', color: 'blue', collapsed: false }],
			tabs: [
				{ id: 'a', title: 'Pinned', pinned: true, group: null },
				{ id: 'b', title: 'Grouped', pinned: false, group: 'group' },
				{ id: 'c', title: 'Tail', pinned: false, group: 'group' },
			],
		});
		const before = model.getSnapshot();
		const availability = vi.fn<(intent: TabStripTabCreationIntent) => ActionAvailability>(() => ({
			kind: 'enabled',
		}));
		let actions: TabStripActions | undefined;
		render(
			createElement(Harness, {
				captureActions: (candidate) => {
					actions = candidate;
				},
				captureScope: () => undefined,
				getTabCreationAvailability: availability,
				model,
				parentScope,
				runtime,
			}),
		);
		if (actions === undefined) throw new Error('Expected mounted Tab actions.');
		await expect(
			actions.execute(TAB_STRIP_ACTION_IDS.duplicateTabs, { tabIds: ['a', 'b'] }, 'context-menu')
				.completion,
		).resolves.toMatchObject({ kind: 'completed' });
		const after = model.getSnapshot();
		expect(availability).toHaveBeenCalledWith({ kind: 'duplicate', tabIds: ['a', 'b'] });
		expect(
			after.tabs.map((tab) => ({ title: tab.title, pinned: tab.pinned, group: tab.group })),
		).toEqual([
			{ title: 'Pinned', pinned: true, group: null },
			{ title: 'Pinned', pinned: true, group: null },
			{ title: 'Grouped', pinned: false, group: 'group' },
			{ title: 'Grouped', pinned: false, group: 'group' },
			{ title: 'Tail', pinned: false, group: 'group' },
		]);
		expect(after.tabs.filter((tab) => before.tabs.some((source) => source.id === tab.id))).toEqual(
			before.tabs,
		);
		expect(new Set(after.tabs.map((tab) => tab.id)).size).toBe(5);
		expect(after.activeTabId).toBe(before.activeTabId);
		expect(after.selectedTabIds).toEqual(before.selectedTabIds);
	});

	it('guards every tab-producing action with the host creation policy', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-1',
			groups: [],
			tabs: [{ group: null, id: 'tab-1', pinned: false, title: 'Tab 1' }],
		});
		const getTabCreationAvailability = vi.fn<
			(intent: TabStripTabCreationIntent) => ActionAvailability
		>(
			(_intent: TabStripTabCreationIntent): ActionAvailability => ({
				kind: 'disabled',
				reason: { id: 'remote-terminal-creation-unavailable' },
			}),
		);
		let actions: TabStripActions | undefined;
		let scope: ActionScope | undefined;
		render(
			createElement(Harness, {
				captureActions: (candidate) => {
					actions = candidate;
				},
				captureScope: (candidate) => {
					scope = candidate;
				},
				getTabCreationAvailability,
				model,
				parentScope,
				runtime,
			}),
		);
		if (actions === undefined || scope === undefined) {
			throw new Error('Expected the tab-strip action fixtures.');
		}

		expect(actions.getCreationAvailability({ groupId: null, kind: 'new-tab' })).toMatchObject({
			kind: 'disabled',
		});
		await expect(
			actions.createTab({ groupId: null, title: 'New Tab' }, 'button').completion,
		).resolves.toMatchObject({ kind: 'blocked' });
		await expect(
			actions.execute(TAB_STRIP_ACTION_IDS.createGroupWithTab, undefined, 'context-menu')
				.completion,
		).resolves.toMatchObject({ kind: 'blocked' });
		await expect(
			actions.execute(TAB_STRIP_ACTION_IDS.duplicateTabs, { tabIds: ['tab-1'] }, 'context-menu')
				.completion,
		).resolves.toMatchObject({ kind: 'blocked' });
		expect(model.getSnapshot().tabs).toHaveLength(1);
	});

	it('delegates Tab selection before mutating the model', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-1',
			tabs: [
				{ group: null, id: 'tab-1', pinned: false, title: 'Tab 1' },
				{ group: null, id: 'tab-2', pinned: false, title: 'Tab 2' },
			],
		});
		let releaseSelection: (() => void) | undefined;
		const selectionGate = new Promise<void>((resolve) => {
			releaseSelection = resolve;
		});
		const onSelectTab = vi.fn<TabStripSelectCallback>(async (context, input) => {
			await selectionGate;
			context.markCommitted();
			model.selectTab(input.tabId);
			return { kind: 'completed' };
		});
		let actions: TabStripActions | undefined;
		render(
			createElement(Harness, {
				captureActions: (candidate) => {
					actions = candidate;
				},
				captureScope: () => undefined,
				model,
				onSelectTab,
				parentScope,
				runtime,
			}),
		);
		if (actions === undefined || releaseSelection === undefined) {
			throw new Error('Expected delegated selection fixtures.');
		}

		const execution = actions.execute(
			TAB_STRIP_ACTION_IDS.selectTab,
			{ mode: 'extend', tabId: 'tab-2' },
			'pointer',
		);
		expect(model.getSnapshot().activeTabId).toBe('tab-1');
		expect(onSelectTab).toHaveBeenCalledWith(expect.anything(), {
			mode: 'extend',
			tabId: 'tab-2',
		});

		releaseSelection();
		await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
		expect(model.getSnapshot().activeTabId).toBe('tab-2');
	});

	it('delegates close completion without deleting the Tab before the owner commits', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-1',
			tabs: [{ group: null, id: 'tab-1', pinned: false, title: 'Tab 1' }],
		});
		let releaseClose: (() => void) | undefined;
		const closeGate = new Promise<void>((resolve) => {
			releaseClose = resolve;
		});
		const onCloseTabs = vi.fn<TabStripCloseCallback>(async (context, request) => {
			await closeGate;
			context.markCommitted();
			model.closeTabs(request.tabIds);
			return { kind: 'completed' };
		});
		let actions: TabStripActions | undefined;
		render(
			createElement(Harness, {
				captureActions: (candidate) => {
					actions = candidate;
				},
				captureScope: () => undefined,
				model,
				onCloseTabs,
				parentScope,
				runtime,
			}),
		);
		if (actions === undefined || releaseClose === undefined) {
			throw new Error('Expected delegated close fixtures.');
		}

		const execution = actions.execute(
			TAB_STRIP_ACTION_IDS.closeTabs,
			{ closeSource: 'mouse', tabIds: ['tab-1'] },
			'pointer',
		);
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['tab-1']);
		expect(onCloseTabs).toHaveBeenCalledWith(expect.anything(), {
			closeSource: 'mouse',
			tabIds: ['tab-1'],
		});

		releaseClose();
		await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
		expect(model.getSnapshot().tabs).toHaveLength(0);
	});

	it('resolves owner close protection before delegating Tab closure', async () => {
		const confirm = vi.fn<() => boolean>(() => false);
		const runtime = new ActionRuntime({ confirm });
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-1',
			tabs: [{ group: null, id: 'tab-1', pinned: false, title: 'Tab 1' }],
		});
		const getCloseTabsConfirmation = vi.fn<TabStripCloseConfirmation>(() => ({
			confirmLabel: { id: 'test.close.confirm' },
			description: { id: 'test.close.description' },
			risk: 'destructive',
			title: { id: 'test.close.title' },
		}));
		const onCloseTabs = vi.fn<TabStripCloseCallback>(() => ({ kind: 'completed' }));
		let actions: TabStripActions | undefined;
		render(
			createElement(Harness, {
				captureActions: (candidate) => {
					actions = candidate;
				},
				captureScope: () => undefined,
				getCloseTabsConfirmation,
				model,
				onCloseTabs,
				parentScope,
				runtime,
			}),
		);
		if (actions === undefined) throw new Error('Expected delegated close fixtures.');

		const request = { closeSource: 'mouse' as const, tabIds: ['tab-1'] };
		const execution = actions.execute(TAB_STRIP_ACTION_IDS.closeTabs, request, 'pointer');

		await expect(execution.completion).resolves.toEqual({ kind: 'cancelled' });
		expect(getCloseTabsConfirmation).toHaveBeenCalledWith(request);
		expect(confirm).toHaveBeenCalledOnce();
		expect(onCloseTabs).not.toHaveBeenCalled();
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['tab-1']);
	});

	it('registers menu actions against the injected runtime and disposes them on unmount', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-1',
			groups: [
				{
					collapsed: false,
					color: 'blue',
					id: 'group-1',
					title: 'Group 1',
				},
			],
			tabs: [
				{ group: 'group-1', id: 'tab-1', pinned: false, title: 'Tab 1' },
				{ group: 'group-1', id: 'tab-2', pinned: false, title: 'Tab 2' },
			],
		});
		let scope: ActionScope | undefined;
		const rendered = render(
			createElement(Harness, {
				captureScope: (candidate) => {
					scope = candidate;
				},
				model,
				parentScope,
				runtime,
			}),
		);
		expect(scope).toBeDefined();
		if (scope === undefined) {
			throw new Error('Expected the tab-strip action scope to be captured');
		}

		const arrangement = model.captureDragRestoreState();
		const listedActionIds = new Set(
			runtime
				.list(
					{ scope, source: 'command-surface' },
					{
						[TAB_STRIP_ACTION_IDS.closeTabs]: {
							closeSource: 'non-ui',
							tabIds: ['tab-1'],
						},
						[TAB_STRIP_ACTION_IDS.createGroup]: { tabIds: ['tab-1'], title: '' },
						[TAB_STRIP_ACTION_IDS.createTab]: {
							groupId: null,
							title: 'New Tab',
						},
						[TAB_STRIP_ACTION_IDS.duplicateTabs]: { tabIds: ['tab-1'] },
						[TAB_STRIP_ACTION_IDS.moveGroup]: {
							direction: 1,
							groupId: 'group-1',
						},
						[TAB_STRIP_ACTION_IDS.moveGroupToWindow]: { groupId: 'group-1' },
						[TAB_STRIP_ACTION_IDS.moveTab]: {
							direction: 'next',
							tabId: 'tab-1',
						},
						[TAB_STRIP_ACTION_IDS.moveTabsToGroup]: {
							groupId: null,
							tabIds: ['tab-1'],
						},
						[TAB_STRIP_ACTION_IDS.applyArrangement]: {
							before: arrangement,
							state: arrangement,
							tabIds: ['tab-1'],
						},
						[TAB_STRIP_ACTION_IDS.moveTabsToWindow]: { tabIds: ['tab-1'] },
						[TAB_STRIP_ACTION_IDS.selectTab]: {
							mode: 'replace',
							tabId: 'tab-1',
						},
						[TAB_STRIP_ACTION_IDS.setGroupCollapsed]: {
							collapsed: false,
							groupId: 'group-1',
						},
						[TAB_STRIP_ACTION_IDS.setTabSearchPinned]: { value: true },
						[TAB_STRIP_ACTION_IDS.setTabsPinned]: {
							pinned: true,
							tabIds: ['tab-1'],
						},
						[TAB_STRIP_ACTION_IDS.setVerticalCollapsed]: { value: true },
						[TAB_STRIP_ACTION_IDS.setVerticalExpandOnHover]: { value: false },
						[TAB_STRIP_ACTION_IDS.ungroup]: { groupId: 'group-1' },
						[TAB_STRIP_ACTION_IDS.updateGroup]: {
							groupId: 'group-1',
							title: 'Updated',
						},
					},
				)
				.map((action) => action.descriptor.id),
		);
		expect(listedActionIds).toEqual(
			new Set(
				Object.values(TAB_STRIP_ACTION_IDS).filter(
					(actionId) => actionId !== TAB_STRIP_ACTION_IDS.applyArrangement,
				),
			),
		);

		await runtime.execute(
			TAB_STRIP_ACTION_IDS.duplicateTabs,
			{ tabIds: ['tab-1'] },
			{ scope, source: 'context-menu' },
		).completion;
		expect(model.getSnapshot().tabs).toHaveLength(3);

		await runtime.execute(
			TAB_STRIP_ACTION_IDS.selectTab,
			{ mode: 'replace', tabId: 'tab-2' },
			{ scope, source: 'keyboard' },
		).completion;
		expect(model.getSnapshot().activeTabId).toBe('tab-2');

		await runtime.execute(
			TAB_STRIP_ACTION_IDS.setGroupCollapsed,
			{ collapsed: true, groupId: 'group-1' },
			{ scope, source: 'pointer' },
		).completion;
		expect(model.getGroup('group-1')?.collapsed).toBe(true);

		const reorderedArrangement = model.captureDragRestoreState();
		await runtime.execute(
			TAB_STRIP_ACTION_IDS.applyArrangement,
			{
				before: reorderedArrangement,
				state: {
					...reorderedArrangement,
					tabs: [...reorderedArrangement.tabs].reverse(),
				},
				tabIds: ['tab-1'],
			},
			{ scope, source: 'pointer' },
		).completion;
		expect(model.getSnapshot().tabs[0]?.id).not.toBe('tab-1');

		rendered.unmount();
		expect(
			runtime
				.execute(
					TAB_STRIP_ACTION_IDS.duplicateTabs,
					{ tabIds: ['tab-1'] },
					{ scope, source: 'context-menu' },
				)
				.getSnapshot().state,
		).toBe('unsupported');
	});

	it('preserves the invocation source when a new tab routes through its host', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const model = new TabModel({
			activeTabId: 'tab-1',
			groups: [],
			tabs: [{ group: null, id: 'tab-1', pinned: false, title: 'Tab 1' }],
		});
		const onNewTab = vi.fn<(request: TabStripNewTabRequest) => ActionExecution>((request) =>
			runtime.execute('test.host-tab.create', undefined, {
				scope: parentScope,
				source: request.source,
			}),
		);
		let actions: TabStripActions | undefined;
		let scope: ActionScope | undefined;
		render(
			createElement(Harness, {
				captureActions: (captured) => {
					actions = captured;
				},
				captureScope: (candidate) => {
					scope = candidate;
				},
				model,
				onNewTab,
				parentScope,
				runtime,
			}),
		);
		if (scope === undefined) {
			throw new Error('Expected the tab-strip action scope to be captured');
		}
		if (actions === undefined) {
			throw new Error('Expected the tab-strip actions to be captured');
		}

		const execution = actions.createTab(
			{
				groupId: null,
				title: 'New Tab',
			},
			'command-surface',
		);

		await expect(execution.completion).resolves.toEqual({ kind: 'unsupported' });
		expect(onNewTab).toHaveBeenCalledWith({
			groupId: null,
			source: 'command-surface',
		});
		expect(
			runtime.resolveAction(
				TAB_STRIP_ACTION_IDS.createTab,
				{ groupId: null, title: 'New Tab' },
				{ scope, source: 'command-surface' },
			),
		).toBeNull();
	});

	it('notifies Action consumers when model and checked-state dependencies change', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('workbench');
		const comboButtonController = new TabStripComboButtonController({ richAnimations: false });
		const model = new TabModel({
			activeTabId: 'tab-1',
			groups: [],
			tabs: [
				{ group: null, id: 'tab-1', pinned: false, title: 'Tab 1' },
				{ group: null, id: 'tab-2', pinned: false, title: 'Tab 2' },
			],
		});
		let scope: ActionScope | undefined;
		render(
			createElement(Harness, {
				captureScope: (candidate) => {
					scope = candidate;
				},
				comboButtonController,
				model,
				parentScope,
				runtime,
			}),
		);
		if (scope === undefined) {
			throw new Error('Expected the tab-strip action scope to be captured');
		}
		const listener = vi.fn<() => void>();
		runtime.subscribe(listener);

		model.closeTab('tab-1');
		await Promise.resolve();

		expect(listener).toHaveBeenCalledOnce();
		expect(
			runtime.resolveAction(
				TAB_STRIP_ACTION_IDS.selectTab,
				{ mode: 'replace', tabId: 'tab-1' },
				{ scope, source: 'command-surface' },
			),
		).toMatchObject({
			availability: { kind: 'disabled' },
		});

		comboButtonController.setPinned('tab-search', true);
		await Promise.resolve();

		expect(listener).toHaveBeenCalledTimes(2);
		expect(
			runtime.resolveAction(
				TAB_STRIP_ACTION_IDS.setTabSearchPinned,
				{ value: false },
				{ scope, source: 'command-surface' },
			),
		).toMatchObject({
			availability: { checked: true, kind: 'enabled' },
		});
	});
});

describe('target group liveness', () => {
	it('blocks an invalid insertion without committing or corrupting the model', async () => {
		const diagnostic = vi.fn<(actionId: string, error: unknown) => void>();
		const runtime = new ActionRuntime({ onUnexpectedError: diagnostic });
		const model = new TabModel({
			tabs: ['a', 'b'].map((id) => ({ id, title: id, group: 'work', pinned: false })),
			groups: [{ id: 'work', title: 'Work', color: 'blue', collapsed: false }],
		});
		let actions: TabStripActions | undefined;
		const rendered = render(
			createElement(Harness, {
				model,
				runtime,
				captureScope: () => undefined,
				captureActions: (value) => {
					actions = value;
				},
			}),
		);
		try {
			if (actions === undefined) throw new Error('Expected mounted actions');
			const before = model.getSnapshot();
			const execution = actions.createTab(
				{ groupId: null, title: 'Rejected', index: 1 },
				'programmatic',
			);
			await expect(execution.completion).resolves.toEqual({
				kind: 'blocked',
				code: 'invalid-tab-arrangement',
			});
			expect(model.getSnapshot()).toBe(before);
			expect(model.getIndexOfTab('tab-1')).toBeNull();
			expect(diagnostic).not.toHaveBeenCalled();
			await expect(
				actions.createTab({ groupId: null, title: 'Allowed' }, 'programmatic').completion,
			).resolves.toEqual({ kind: 'completed' });
			expect(model.getSnapshot().activeTabId).toBe('tab-1');
		} finally {
			rendered.unmount();
			runtime.dispose();
		}
	});

	it('blocks unknown create and move targets without diagnostics or identity consumption', async () => {
		const diagnostic = vi.fn<(actionId: string, error: unknown) => void>();
		const runtime = new ActionRuntime({ onUnexpectedError: diagnostic });
		const model = new TabModel({
			tabs: [{ id: 'existing', title: 'Existing', pinned: false, group: null }],
		});
		let actions: TabStripActions | undefined;
		const rendered = render(
			createElement(Harness, {
				model,
				runtime,
				captureScope: () => undefined,
				captureActions: (value) => {
					actions = value;
				},
			}),
		);
		try {
			if (actions === undefined) throw new Error('Expected mounted actions');
			const before = model.getSnapshot();
			for (const [id, input] of [
				[TAB_STRIP_ACTION_IDS.createTab, { groupId: 'missing', title: 'Rejected' }],
				[TAB_STRIP_ACTION_IDS.moveTabsToGroup, { groupId: 'missing', tabIds: ['existing'] }],
			] as const) {
				const invocation = { scope: actions.scope, source: 'programmatic' as const };
				expect(runtime.resolveAction(id, input, invocation)?.availability.kind).toBe('disabled');
				await expect(runtime.execute(id, input, invocation).completion).resolves.toMatchObject({
					kind: 'blocked',
					code: 'action-unavailable',
				});
			}
			expect(actions.getCreationAvailability({ kind: 'new-tab', groupId: 'missing' }).kind).toBe(
				'disabled',
			);
			expect(model.getSnapshot()).toBe(before);
			expect(diagnostic).not.toHaveBeenCalled();
			await expect(
				actions.createTab({ groupId: null, title: 'Allowed' }, 'button').completion,
			).resolves.toEqual({ kind: 'completed' });
			expect(model.getSnapshot().activeTabId).toBe('tab-1');
		} finally {
			rendered.unmount();
			runtime.dispose();
		}
	});

	it('rechecks prepared destinations after deletion and retains live and null targets', async () => {
		const runtime = new ActionRuntime();
		const model = new TabModel({
			tabs: [{ id: 'existing', title: 'Existing', pinned: false, group: 'work' }],
			groups: [{ id: 'work', title: 'Work', color: 'blue', collapsed: false }],
		});
		let actions: TabStripActions | undefined;
		const rendered = render(
			createElement(Harness, {
				model,
				runtime,
				captureScope: () => undefined,
				captureActions: (value) => {
					actions = value;
				},
			}),
		);
		try {
			if (actions === undefined) throw new Error('Expected mounted actions');
			await expect(
				actions.createTab({ groupId: 'work', title: 'Allowed' }, 'button').completion,
			).resolves.toEqual({ kind: 'completed' });
			await expect(
				actions.execute(
					TAB_STRIP_ACTION_IDS.moveTabsToGroup,
					{ groupId: null, tabIds: ['tab-1'] },
					'programmatic',
				).completion,
			).resolves.toEqual({ kind: 'completed' });
			await expect(
				actions.execute(
					TAB_STRIP_ACTION_IDS.moveTabsToGroup,
					{ groupId: 'work', tabIds: ['tab-1', 'already-gone'] },
					'programmatic',
				).completion,
			).resolves.toEqual({ kind: 'completed' });
			const prepared = runtime
				.discover(
					{ scope: actions.scope, source: 'programmatic' },
					{
						inputByActionId: {
							[TAB_STRIP_ACTION_IDS.createTab]: { groupId: 'work', title: 'Stale' },
							[TAB_STRIP_ACTION_IDS.moveTabsToGroup]: { groupId: 'work', tabIds: ['existing'] },
						},
					},
				)
				.filter(
					(entry) =>
						entry.descriptor.id === TAB_STRIP_ACTION_IDS.createTab ||
						entry.descriptor.id === TAB_STRIP_ACTION_IDS.moveTabsToGroup,
				);
			expect(prepared).toHaveLength(2);
			model.ungroup('work');
			const before = model.getSnapshot();
			for (const entry of prepared) {
				if (entry.kind !== 'ready') throw new Error('Expected prepared group actions');
				await expect(runtime.executePrepared(entry.invocation).completion).resolves.toMatchObject({
					kind: 'blocked',
					code: 'action-unavailable',
				});
			}
			expect(model.getSnapshot()).toBe(before);
		} finally {
			rendered.unmount();
			runtime.dispose();
		}
	});
});
