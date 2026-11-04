import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionCatalogId,
	type ActionExecution,
	type ActionExecutionContext,
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
	tabGroupCreateInput,
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
import { describeTabDragRelease } from '../model/describeTabDragRelease';
import {
	TabModelInvariantError,
	type TabDragRestoreState,
	type TabModel,
} from '../model/tab-model';
import type { TabStripComboButtonController } from '../model/TabStripComboButtonController';
import type { TabStripMessages } from '../types/tab-strip-messages';
import type {
	TabStripArrangementChange,
	TabStripChange,
	TabStripChangeCallback,
	TabStripCloseCallback,
	TabStripNewTabRequest,
	TabStripSelectCallback,
	TabStripTabCreationIntent,
} from '../types/tab-strip-options';
import type { TabStripActions } from '../types/tab-strip-actions';
import {
	createGroupForTabs,
	createNewTabGroup,
	NEW_TAB_GROUP_COLOR,
} from '../utils/tab-strip-internals';

function hasTargetGroup(model: TabModel, groupId: string | null): boolean {
	return groupId === null || model.getGroup(groupId) !== null;
}

/** Collapsing is this Client's own view, and an authority needs it to know where a move lands. */
function adjoinsCollapsedGroup(
	model: TabModel,
	tabId: string,
	direction: Extract<TabStripArrangementChange, { kind: 'tab-moved' }>['direction'],
): boolean {
	const { tabs } = model.getSnapshot();
	const index = tabs.findIndex((tab) => tab.id === tabId);
	const group = tabs[direction === 'previous' ? index - 1 : index + 1]?.group ?? null;
	return group !== null && model.getGroup(group)?.collapsed === true;
}

/**
 * The dragged arrangement stays on screen until the authority answers, because its publication
 * replaces this model. Any other answer publishes nothing, so the Tabs go back, keeping what the
 * person has selected since they were pressed.
 */
async function proposeDraggedArrangement(
	context: ActionExecutionContext,
	model: TabModel,
	onTabsChange: TabStripChangeCallback,
	change: TabStripChange,
	before: TabDragRestoreState,
) {
	const result = await onTabsChange(context, change);
	if (result.kind !== 'completed') model.restoreArrangement(before);
	return result;
}

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
	readonly onSelectTab: TabStripSelectCallback | undefined;
	readonly onTabsChange: TabStripChangeCallback | undefined;
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
	onSelectTab,
	onTabsChange,
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
	const getCreationAvailability = useMemoizedFn((intent: TabStripTabCreationIntent) => {
		if (intent.kind === 'new-tab' && !hasTargetGroup(model, intent.groupId)) {
			return { kind: 'disabled' as const, reason: workbenchMessages.actionUnavailable };
		}
		return getTabCreationAvailability?.(intent) ?? ({ kind: 'enabled' } as const);
	});

	useEffect(() => {
		lifecycleGeneration.current += 1;
		const generation = lifecycleGeneration.current;
		return () => {
			queueMicrotask(() => {
				if (lifecycleGeneration.current === generation) {
					ownedRuntime.dispose();
				}
			});
		};
	}, [ownedRuntime]);

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
		const commit = (context: { markCommitted(): void }, operation: () => void) => {
			const revision = model.getSnapshot().revision;
			context.markCommitted();
			operation();
			return model.getSnapshot().revision === revision
				? { kind: 'unchanged' as const }
				: { kind: 'completed' as const };
		};
		const apply = (context: ActionExecutionContext, change: TabStripChange, local: () => void) =>
			onTabsChange === undefined ? commit(context, local) : onTabsChange(context, change);
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
								getCreationAvailability({
									groupId: input.groupId,
									...(input.index === undefined ? {} : { index: input.index }),
									kind: 'new-tab',
								}),
							handler: {
								execute: (context, input) => {
									if (!hasTargetGroup(model, input.groupId)) {
										return { kind: 'blocked', code: 'group-not-found' };
									}
									try {
										model.addTab(
											{
												group: input.groupId,
												pinned: false,
												title: input.title,
											},
											input.index === undefined ? {} : { index: input.index },
										);
									} catch (error) {
										if (error instanceof TabModelInvariantError) {
											return { kind: 'blocked', code: 'invalid-tab-arrangement' };
										}
										throw error;
									}
									context.markCommitted();
									return { kind: 'completed' };
								},
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
					// A close can outlast the press that asked for it, and a middle click reports itself twice.
					execution: { ...parallelByTabs, concurrency: 'reject-duplicate' },
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
							apply(
								context,
								{
									adjacentGroupCollapsed: adjoinsCollapsedGroup(
										model,
										input.tabId,
										input.direction,
									),
									direction: input.direction,
									kind: 'tab-moved',
									tabId: input.tabId,
								},
								() => {
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
								},
							),
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
							apply(
								context,
								{
									color: NEW_TAB_GROUP_COLOR,
									kind: 'tabs-grouped',
									tabIds: existingTabIds(input.tabIds),
									title: input.title,
								},
								() => createGroupForTabs(model, input.tabIds, input.title),
							),
					},
					id: TAB_STRIP_ACTION_IDS.createGroup,
					input: tabGroupCreateInput,
					title: workbenchTabMessages.newGroup,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					// An authority can take longer to answer than a person takes to press again.
					execution: {
						concurrency: 'reject-duplicate',
						concurrencyKey: () => 'new-group',
						mode: 'foreground',
					},
					getAvailability: () => getCreationAvailability({ kind: 'new-group' }),
					handler: {
						execute: (context) =>
							apply(context, { kind: 'tab-created-in-new-group' }, () =>
								createNewTabGroup(model, messages.newTabTitle),
							),
					},
					id: TAB_STRIP_ACTION_IDS.createGroupWithTab,
					input: noActionInput,
					title: workbenchTabMessages.newGroupWithTab,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: parallelByTabs,
					getAvailability: (_context, input) =>
						hasTargetGroup(model, input.groupId)
							? tabAvailability(input.tabIds)
							: { kind: 'disabled', reason: workbenchMessages.actionUnavailable },
					handler: {
						execute: (context, input) => {
							if (!hasTargetGroup(model, input.groupId)) {
								return { kind: 'blocked', code: 'group-not-found' };
							}
							return apply(
								context,
								{
									groupId: input.groupId,
									kind: 'tabs-group-set',
									tabIds: existingTabIds(input.tabIds),
								},
								() => model.moveTabsToGroup(input.tabIds, input.groupId),
							);
						},
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
					// An authority can take longer to answer than a person takes to press again.
					execution: { ...parallelByTabs, concurrency: 'reject-duplicate' },
					getAvailability: (_context, input) => {
						const availability = getCreationAvailability({
							kind: 'duplicate',
							tabIds: input.tabIds,
						});
						return availability.kind === 'enabled' ? tabAvailability(input.tabIds) : availability;
					},
					handler: {
						execute: (context, input) => {
							const tabIds = existingTabIds(input.tabIds);
							return apply(context, { kind: 'tabs-duplicated', tabIds }, () => {
								for (const id of tabIds) {
									model.duplicateTab(id);
								}
							});
						},
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
							apply(
								context,
								{
									kind: 'tabs-pinned-set',
									pinned: input.pinned,
									tabIds: existingTabIds(input.tabIds),
								},
								() => model.setTabsPinned(input.tabIds, input.pinned),
							),
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
						execute: (context, input) =>
							apply(context, { groupId: input.groupId, kind: 'group-ungrouped' }, () =>
								model.ungroup(input.groupId),
							),
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
							apply(
								context,
								{ direction: input.direction, groupId: input.groupId, kind: 'group-moved' },
								() => model.shiftGroupRelative(input.groupId, input.direction),
							),
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
						execute: (context, input) => {
							try {
								model.restoreCanceledDrag(input.state);
							} catch (error) {
								if (error instanceof TabModelInvariantError) {
									return { kind: 'blocked', code: 'invalid-tab-arrangement' };
								}
								throw error;
							}
							if (onTabsChange === undefined) {
								// Synchronous publication may request cancellation; the committed
								// model still owns the result when this handler returns.
								context.markCommitted();
								return { kind: 'completed' };
							}
							const release = describeTabDragRelease(input.before, input.state, input.tabIds);
							return release === null
								? { kind: 'unchanged' }
								: proposeDraggedArrangement(context, model, onTabsChange, release, input.before);
						},
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
							apply(
								context,
								{
									color: input.color,
									groupId: input.groupId,
									kind: 'group-updated',
									title: input.title,
								},
								() =>
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
		getCreationAvailability,
		getTabCreationAvailability,
		onMoveGroupToNewWindow,
		onMoveTabsToNewWindow,
		onNewTab,
		onCloseTabs,
		onSelectTab,
		onTabsChange,
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
	return useMemo(
		() => ({ createTab, execute, getCreationAvailability, scope }),
		[createTab, execute, getCreationAvailability, scope],
	);
}
