import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionCatalogId,
	type ActionExecution,
	type ActionInputFor,
	type ActionInvocationSource,
	type ActionOutputFor,
	type ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import { useEffect, useMemo, useRef, useState } from 'react';

import {
	TAB_STRIP_ACTION_IDS,
	type TabCreateInput,
	type TabStripActionCatalog,
	tabArrangementInput,
	booleanValueInput,
	tabCloseInput,
	tabCreateInput,
	tabGroupCollapsedInput,
	tabGroupIdInput,
	tabGroupMoveInput,
	tabGroupShiftInput,
	tabGroupUpdateInput,
	tabIdsInput,
	tabMoveInput,
	tabPinnedInput,
	tabSelectionInput,
} from '../actions/tab-strip-action-contracts';
import type { TabCloseSource } from '../layout/tab-closing-mode';
import type { TabModel } from '../model/tab-model';
import type { TabStripComboButtonController } from '../model/tab-strip-combo-button';
import type { TabStripMessages } from '../types/tab-strip-messages';
import type {
	TabStripCloseCallback,
	TabStripCloseConfirmation,
	TabStripNewTabRequest,
	TabStripSelectCallback,
	TabStripTabCreationIntent,
} from '../types/tab-strip-options';
import type { TabStripActions } from '../types/tab-strip-actions';
import { createGroupForTabs, createNewTabGroup } from '../utils/tab-strip-internals';

interface UseTabStripActionsOptions {
	readonly actionRuntime: ActionRuntime | undefined;
	readonly messages: TabStripMessages;
	readonly model: TabModel;
	readonly comboButtonController: TabStripComboButtonController;
	readonly onMoveGroupToNewWindow: ((groupId: string) => void) | undefined;
	readonly onMoveTabsToNewWindow: ((tabIds: readonly string[]) => void) | undefined;
	readonly onNewTab: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly getTabCreationAvailability?:
		| ((
				intent: TabStripTabCreationIntent,
		  ) => ReturnType<TabStripActions['getCreationAvailability']>)
		| undefined;
	readonly onCloseTabs: TabStripCloseCallback | undefined;
	readonly getCloseTabsConfirmation: TabStripCloseConfirmation | undefined;
	readonly onSelectTab: TabStripSelectCallback | undefined;
	readonly parentActionScope: ActionScope | undefined;
	readonly requestCloseTabs: (tabIds: readonly string[], source?: TabCloseSource) => void;
	readonly onVerticalCollapsedChange: ((collapsed: boolean) => void) | undefined;
	readonly onVerticalExpandOnHoverChange: ((enabled: boolean) => void) | undefined;
	readonly onVerticalTabSearchClick: (() => void) | undefined;
	readonly verticalCollapsed: boolean;
	readonly verticalExpandOnHover: boolean;
}

export function useTabStripActions({
	actionRuntime,
	comboButtonController,
	messages,
	model,
	onMoveGroupToNewWindow,
	onMoveTabsToNewWindow,
	onNewTab,
	getTabCreationAvailability,
	onCloseTabs,
	getCloseTabsConfirmation,
	onSelectTab,
	parentActionScope,
	requestCloseTabs,
	onVerticalCollapsedChange,
	onVerticalExpandOnHoverChange,
	onVerticalTabSearchClick,
	verticalCollapsed,
	verticalExpandOnHover,
}: UseTabStripActionsOptions): TabStripActions {
	const [ownedRuntime] = useState(() => new ActionRuntime());
	const lifecycleGeneration = useRef(0);
	const runtime = actionRuntime ?? ownedRuntime;
	const typedRuntime = useMemo(
		() => createTypedActionRuntime<TabStripActionCatalog>(runtime),
		[runtime],
	);
	const scope = useMemo(
		() => runtime.createScope('tab-strip', parentActionScope ?? null),
		[parentActionScope, runtime],
	);

	useEffect(() => {
		lifecycleGeneration.current += 1;
		const generation = lifecycleGeneration.current;
		return () => {
			queueMicrotask(() => {
				if (actionRuntime === undefined && lifecycleGeneration.current === generation) {
					ownedRuntime.dispose();
				}
			});
		};
	}, [actionRuntime, ownedRuntime]);

	useEffect(() => {
		let active = true;
		let notificationPending = false;
		const notifyChanged = () => {
			if (notificationPending) {
				return;
			}
			notificationPending = true;
			queueMicrotask(() => {
				notificationPending = false;
				if (active) {
					runtime.notifyChanged();
				}
			});
		};
		const unsubscribeModel = model.subscribe(notifyChanged);
		const unsubscribeComboButton = comboButtonController.subscribe(notifyChanged);
		return () => {
			active = false;
			unsubscribeModel();
			unsubscribeComboButton();
		};
	}, [comboButtonController, model, runtime]);

	useEffect(() => {
		const category = {
			id: 'tabs',
			title: workbenchTabMessages.actionCategoryTabs,
		};
		const parallelByTabs = {
			concurrency: 'parallel' as const,
			concurrencyKey: (input: { readonly tabIds: readonly string[] }) =>
				input.tabIds.join('\u0000'),
			mode: 'foreground' as const,
		};
		const existingTabIds = (tabIds: readonly string[]) => {
			const ids = new Set(model.getSnapshot().tabs.map((tab) => tab.id));
			return [...new Set(tabIds)].filter((id) => ids.has(id));
		};
		const tabAvailability = (tabIds: readonly string[]) =>
			existingTabIds(tabIds).length === 0
				? {
						kind: 'disabled' as const,
						reason: workbenchMessages.actionUnavailable,
					}
				: { kind: 'enabled' as const };
		const creationAvailability = (intent: TabStripTabCreationIntent) =>
			getTabCreationAvailability?.(intent) ?? ({ kind: 'enabled' } as const);
		const commit = (context: { markCommitted(): void }, operation: () => void) => {
			const revision = model.getSnapshot().revision;
			context.markCommitted();
			operation();
			return model.getSnapshot().revision === revision
				? { kind: 'unchanged' as const }
				: { kind: 'completed' as const };
		};
		const createTabRegistration =
			onNewTab === undefined
				? typedRuntime.register(
						scope,
						defineAction({
							category,
							execution: {
								concurrency: 'parallel',
								concurrencyKey: (input) =>
									`${input.groupId ?? 'ungrouped'}:${input.index ?? 'end'}`,
								mode: 'foreground',
							},
							getAvailability: (_context, input) =>
								creationAvailability({
									groupId: input.groupId,
									...(input.index === undefined ? {} : { index: input.index }),
									kind: 'new-tab',
								}),
							handler: {
								execute: (context, input) =>
									commit(context, () => {
										model.addTab(
											{
												group: input.groupId,
												pinned: false,
												title: input.title,
											},
											input.index === undefined ? {} : { index: input.index },
										);
									}),
							},
							id: TAB_STRIP_ACTION_IDS.createTab,
							input: tabCreateInput,
							title: workbenchTabMessages.newTab,
						}),
					)
				: null;
		const registrations = [
			...(createTabRegistration === null ? [] : [createTabRegistration]),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					confirmation: (input) => getCloseTabsConfirmation?.(input) ?? null,
					execution: parallelByTabs,
					getAvailability: (_context, input) => tabAvailability(input.tabIds),
					handler: {
						execute: (context, input) => {
							const tabIds = existingTabIds(input.tabIds);
							return onCloseTabs === undefined
								? commit(context, () => requestCloseTabs(tabIds, input.closeSource))
								: onCloseTabs(context, {
										closeSource: input.closeSource,
										tabIds,
									});
						},
					},
					id: TAB_STRIP_ACTION_IDS.closeTabs,
					input: tabCloseInput,
					title: workbenchTabMessages.close,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: (input) => input.tabId,
						mode: 'foreground',
					},
					getAvailability: (_context, input) => tabAvailability([input.tabId]),
					handler: {
						execute: (context, input) =>
							onSelectTab === undefined
								? commit(context, () => {
										switch (input.mode) {
											case 'add-range':
												model.addSelectionFromAnchorTo(input.tabId);
												break;
											case 'extend':
												model.extendSelectionTo(input.tabId);
												break;
											case 'replace':
												model.selectTab(input.tabId);
												break;
											case 'toggle':
												model.toggleSelected(input.tabId);
												break;
										}
									})
								: onSelectTab(context, input),
					},
					id: TAB_STRIP_ACTION_IDS.selectTab,
					input: tabSelectionInput,
					title: workbenchTabMessages.selectTab,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: (input) => input.tabId,
						mode: 'foreground',
					},
					getAvailability: (_context, input) => tabAvailability([input.tabId]),
					handler: {
						execute: (context, input) =>
							commit(context, () => {
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
							}),
					},
					id: TAB_STRIP_ACTION_IDS.moveTab,
					input: tabMoveInput,
					title: workbenchTabMessages.moveTab,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: parallelByTabs,
					getAvailability: (_context, input) => tabAvailability(input.tabIds),
					handler: {
						execute: (context, input) =>
							commit(context, () => createGroupForTabs(model, input.tabIds)),
					},
					id: TAB_STRIP_ACTION_IDS.createGroup,
					input: tabIdsInput,
					title: workbenchTabMessages.newGroup,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: () => 'new-group',
						mode: 'foreground',
					},
					getAvailability: () => creationAvailability({ kind: 'new-group' }),
					handler: {
						execute: (context) =>
							commit(context, () => createNewTabGroup(model, messages.newTabTitle)),
					},
					id: TAB_STRIP_ACTION_IDS.createGroupWithTab,
					input: noActionInput,
					title: workbenchTabMessages.newGroup,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: parallelByTabs,
					getAvailability: (_context, input) => tabAvailability(input.tabIds),
					handler: {
						execute: (context, input) =>
							commit(context, () => model.moveTabsToGroup(input.tabIds, input.groupId)),
					},
					id: TAB_STRIP_ACTION_IDS.moveTabsToGroup,
					input: tabGroupMoveInput,
					title: workbenchTabMessages.moveTabsToGroup,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: parallelByTabs,
					getAvailability: (_context, input) =>
						onMoveTabsToNewWindow === undefined
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: tabAvailability(input.tabIds),
					handler: {
						execute: (context, input) => {
							if (onMoveTabsToNewWindow === undefined) {
								return { kind: 'blocked', code: 'host-capability-unavailable' };
							}
							context.markCommitted();
							onMoveTabsToNewWindow(existingTabIds(input.tabIds));
							return { kind: 'completed' };
						},
					},
					id: TAB_STRIP_ACTION_IDS.moveTabsToWindow,
					input: tabIdsInput,
					title: workbenchTabMessages.moveTabsToNewWindow,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: parallelByTabs,
					getAvailability: (_context, input) => {
						const availability = creationAvailability({
							kind: 'duplicate',
							tabIds: input.tabIds,
						});
						return availability.kind === 'enabled' ? tabAvailability(input.tabIds) : availability;
					},
					handler: {
						execute: (context, input) =>
							commit(context, () => {
								for (const id of existingTabIds(input.tabIds)) {
									model.duplicateTab(id);
								}
							}),
					},
					id: TAB_STRIP_ACTION_IDS.duplicateTabs,
					input: tabIdsInput,
					title: workbenchTabMessages.duplicate,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: parallelByTabs,
					getAvailability: (_context, input) => tabAvailability(input.tabIds),
					handler: {
						execute: (context, input) =>
							commit(context, () => model.setTabsPinned(input.tabIds, input.pinned)),
					},
					id: TAB_STRIP_ACTION_IDS.setTabsPinned,
					input: tabPinnedInput,
					title: workbenchTabMessages.pin,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: (input) => input.groupId,
						mode: 'foreground',
					},
					getAvailability: (_context, input) =>
						model.getGroup(input.groupId) === null
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: { kind: 'enabled' },
					handler: {
						execute: (context, input) => commit(context, () => model.ungroup(input.groupId)),
					},
					id: TAB_STRIP_ACTION_IDS.ungroup,
					input: tabGroupIdInput,
					title: workbenchTabMessages.ungroup,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: (input) => input.groupId,
						mode: 'foreground',
					},
					getAvailability: (_context, input) => {
						const group = model.getGroup(input.groupId);
						return group === null
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: {
									checked: group.collapsed,
									kind: 'enabled',
								};
					},
					handler: {
						execute: (context, input) => {
							const group = model.getGroup(input.groupId);
							if (group === null) {
								return { kind: 'blocked', code: 'group-not-found' };
							}
							if (group.collapsed === input.collapsed) {
								return { kind: 'unchanged' };
							}
							return commit(context, () => model.toggleGroupCollapsed(input.groupId));
						},
					},
					id: TAB_STRIP_ACTION_IDS.setGroupCollapsed,
					input: tabGroupCollapsedInput,
					title: workbenchTabMessages.setGroupCollapsed,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: (input) => input.groupId,
						mode: 'foreground',
					},
					getAvailability: (_context, input) =>
						model.getGroup(input.groupId) === null
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: { kind: 'enabled' },
					handler: {
						execute: (context, input) =>
							commit(context, () => model.shiftGroupRelative(input.groupId, input.direction)),
					},
					id: TAB_STRIP_ACTION_IDS.moveGroup,
					input: tabGroupShiftInput,
					title: workbenchTabMessages.moveGroup,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'reject-duplicate',
						concurrencyKey: () => 'arrangement',
						mode: 'foreground',
					},
					getAvailability: (context) =>
						context.source === 'pointer' || context.source === 'programmatic'
							? { kind: 'enabled' }
							: { kind: 'hidden' },
					handler: {
						execute: (context, input) =>
							commit(context, () => model.restoreCanceledDrag(input.state)),
					},
					id: TAB_STRIP_ACTION_IDS.applyArrangement,
					input: tabArrangementInput,
					title: workbenchTabMessages.moveTab,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: (input) => input.groupId,
						mode: 'foreground',
					},
					getAvailability: (_context, input) =>
						model.getGroup(input.groupId) === null
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: { kind: 'enabled' },
					handler: {
						execute: (context, input) =>
							commit(context, () =>
								model.updateGroup(input.groupId, {
									...(input.color === undefined ? {} : { color: input.color }),
									...(input.title === undefined ? {} : { title: input.title }),
								}),
							),
					},
					id: TAB_STRIP_ACTION_IDS.updateGroup,
					input: tabGroupUpdateInput,
					title: workbenchTabMessages.updateGroup,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: (input) => input.groupId,
						mode: 'foreground',
					},
					getAvailability: (_context, input) =>
						onMoveGroupToNewWindow === undefined || model.getGroup(input.groupId) === null
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: { kind: 'enabled' },
					handler: {
						execute: (context, input) => {
							if (onMoveGroupToNewWindow === undefined) {
								return { kind: 'blocked', code: 'host-capability-unavailable' };
							}
							context.markCommitted();
							onMoveGroupToNewWindow(input.groupId);
							return { kind: 'completed' };
						},
					},
					id: TAB_STRIP_ACTION_IDS.moveGroupToWindow,
					input: tabGroupIdInput,
					title: workbenchTabMessages.moveGroupToNewWindow,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'reject-duplicate',
						concurrencyKey: () => 'tab-search',
						mode: 'foreground',
					},
					getAvailability: () =>
						onVerticalTabSearchClick === undefined
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: { kind: 'enabled' },
					handler: {
						execute: (context) => {
							if (onVerticalTabSearchClick === undefined) {
								return { kind: 'blocked', code: 'host-capability-unavailable' };
							}
							context.markCommitted();
							onVerticalTabSearchClick();
							return { kind: 'completed' };
						},
					},
					id: TAB_STRIP_ACTION_IDS.openTabSearch,
					input: noActionInput,
					title: workbenchTabMessages.searchTabs,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: () => 'tab-search-pinned',
						mode: 'foreground',
					},
					getAvailability: (_context, _input) => ({
						checked: comboButtonController.getSnapshot().tabSearchPinned,
						kind: 'enabled',
					}),
					handler: {
						execute: (context, input) => {
							if (comboButtonController.getSnapshot().tabSearchPinned === input.value) {
								return { kind: 'unchanged' };
							}
							context.markCommitted();
							comboButtonController.executeTogglePinned('tab-search');
							return { kind: 'completed' };
						},
					},
					id: TAB_STRIP_ACTION_IDS.setTabSearchPinned,
					input: booleanValueInput,
					title: workbenchTabMessages.pinSearchTabs,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: () => 'vertical-collapsed',
						mode: 'foreground',
					},
					getAvailability: (_context, _input) =>
						onVerticalCollapsedChange === undefined
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: { checked: verticalCollapsed, kind: 'enabled' },
					handler: {
						execute: (context, input) => {
							if (onVerticalCollapsedChange === undefined) {
								return { kind: 'blocked', code: 'host-capability-unavailable' };
							}
							if (input.value === verticalCollapsed) return { kind: 'unchanged' };
							context.markCommitted();
							onVerticalCollapsedChange(input.value);
							return { kind: 'completed' };
						},
					},
					id: TAB_STRIP_ACTION_IDS.setVerticalCollapsed,
					input: booleanValueInput,
					title: workbenchTabMessages.collapseTabStrip,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: () => 'vertical-expand-on-hover',
						mode: 'foreground',
					},
					getAvailability: () =>
						onVerticalExpandOnHoverChange === undefined
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: { checked: verticalExpandOnHover, kind: 'enabled' },
					handler: {
						execute: (context, input) => {
							if (onVerticalExpandOnHoverChange === undefined) {
								return { kind: 'blocked', code: 'host-capability-unavailable' };
							}
							if (input.value === verticalExpandOnHover) return { kind: 'unchanged' };
							context.markCommitted();
							onVerticalExpandOnHoverChange(input.value);
							return { kind: 'completed' };
						},
					},
					id: TAB_STRIP_ACTION_IDS.setVerticalExpandOnHover,
					input: booleanValueInput,
					title: workbenchTabMessages.expandTabStripOnHover,
				}),
			),
		];

		return () => {
			for (const registration of registrations) {
				registration.dispose();
			}
		};
	}, [
		comboButtonController,
		messages.newTabTitle,
		model,
		getCloseTabsConfirmation,
		getTabCreationAvailability,
		onMoveGroupToNewWindow,
		onMoveTabsToNewWindow,
		onNewTab,
		onCloseTabs,
		onSelectTab,
		onVerticalCollapsedChange,
		onVerticalExpandOnHoverChange,
		onVerticalTabSearchClick,
		requestCloseTabs,
		scope,
		typedRuntime,
		verticalCollapsed,
		verticalExpandOnHover,
	]);

	const execute = useMemoizedFn(
		<Id extends ActionCatalogId<TabStripActionCatalog>>(
			actionId: Id,
			input: ActionInputFor<TabStripActionCatalog, Id>,
			source: ActionInvocationSource,
		): ActionExecution<ActionOutputFor<TabStripActionCatalog, Id>> =>
			typedRuntime.execute(actionId, input, { scope, source }),
	);
	const createTab = useMemoizedFn(
		(
			input: TabCreateInput,
			source: TabStripNewTabRequest['source'],
		): ReturnType<TabStripActions['createTab']> => {
			return onNewTab === undefined
				? execute(TAB_STRIP_ACTION_IDS.createTab, input, source)
				: onNewTab({
						groupId: input.groupId,
						...(input.index === undefined ? {} : { index: input.index }),
						source,
					});
		},
	);
	const getCreationAvailability = useMemoizedFn(
		(intent: TabStripTabCreationIntent) =>
			getTabCreationAvailability?.(intent) ?? ({ kind: 'enabled' } as const),
	);

	return useMemo(
		() => ({ createTab, execute, getCreationAvailability, scope }),
		[createTab, execute, getCreationAvailability, scope],
	);
}
