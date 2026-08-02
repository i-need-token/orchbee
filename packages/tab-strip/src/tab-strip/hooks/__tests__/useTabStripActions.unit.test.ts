import { ActionRuntime, type ActionExecution, type ActionScope } from '@orchbee/actions';
import { cleanup, render } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { TAB_STRIP_ACTION_IDS } from '../../actions/tab-strip-action-contracts';
import { DEFAULT_TAB_STRIP_MESSAGES } from '../../constants/default-tab-strip-messages';
import { TabModel } from '../../model/tab-model';
import { TabStripComboButtonController } from '../../model/tab-strip-combo-button';
import type { TabStripCloseCallback, TabStripNewTabRequest } from '../../types/tab-strip-options';
import type { TabStripActions } from '../../types/tab-strip-actions';
import { useTabStripActions } from '../useTabStripActions';

interface HarnessProps {
	readonly captureActions?: ((actions: TabStripActions) => void) | undefined;
	readonly captureScope: (scope: ActionScope) => void;
	readonly comboButtonController?: TabStripComboButtonController | undefined;
	readonly model: TabModel;
	readonly onCloseTabs?: TabStripCloseCallback | undefined;
	readonly onNewTab?: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly parentScope: ActionScope;
	readonly runtime: ActionRuntime;
}

function Harness({
	captureActions,
	captureScope,
	comboButtonController = new TabStripComboButtonController(),
	model,
	onCloseTabs,
	onNewTab,
	parentScope,
	runtime,
}: HarnessProps): ReactNode {
	const actions = useTabStripActions({
		actionRuntime: runtime,
		comboButtonController,
		messages: DEFAULT_TAB_STRIP_MESSAGES,
		model,
		onMoveGroupToNewWindow: undefined,
		onMoveTabsToNewWindow: undefined,
		onNewTab,
		onCloseTabs,
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
						[TAB_STRIP_ACTION_IDS.createGroup]: { tabIds: ['tab-1'] },
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
						[TAB_STRIP_ACTION_IDS.applyArrangement]: { state: arrangement },
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
				state: {
					...reorderedArrangement,
					tabs: [...reorderedArrangement.tabs].reverse(),
				},
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
