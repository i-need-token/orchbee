import styles from './TabStripContextMenu.module.css';
import {
	ContextMenuCheckboxItem,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuShortcut,
	ContextMenuSub,
	ContextMenuSubContent,
	ContextMenuSubTrigger,
} from '@orchbee/ui/components';
import { type CSSProperties, type ReactNode } from 'react';
import { type TabData, type TabModelSnapshot } from '../../../../model/tab-model';
import { getGroupCloseTabIds, getTabCloseTargets } from '../../../../model/tab-close-targets';
import { requireDefined } from '../../../../utils/requireDefined';
import { TabStripOrientation } from '../../../../types/tab-strip-types';
import {
	GROUP_COLORS,
	GROUP_COLOR_NAMES,
	type MenuState,
} from '../../../../utils/tab-strip-internals';
import { GroupEditorMenuIcon } from './components/group-editor-menu-icon/GroupEditorMenuIcon';
import { GroupTitleInput } from './components/group-title-input/GroupTitleInput';
import { useTabStripMessages } from '../../../../hooks/useTabStripMessages';
import type { TabStripActions } from '../../../../types/tab-strip-actions';
import { TAB_STRIP_ACTION_IDS } from '../../../../actions/tab-strip-action-contracts';
import type { TabStripShortcutLabels } from '../../../../types/tab-strip-options';

interface TabStripContextMenuProps {
	actions: TabStripActions;
	state: MenuState;
	snapshot: TabModelSnapshot;
	orientation: TabStripOrientation;
	onOrientationChange?: ((orientation: TabStripOrientation) => void) | undefined;
	closeAllowed: boolean;
	verticalExpandOnHover: boolean;
	onVerticalExpandOnHoverChange?: ((enabled: boolean) => void) | undefined;
	tabSearchPinned: boolean;
	moveGroupToNewWindowAvailable: boolean;
	moveTabsToNewWindowAvailable: boolean;
	onSetTabTitle: ((tabId: string) => void) | undefined;
	shortcutLabels: TabStripShortcutLabels | undefined;
	onClose: () => void;
}

export function TabStripContextMenu({
	actions,
	state,
	snapshot,
	orientation,
	onOrientationChange,
	closeAllowed,
	verticalExpandOnHover,
	onVerticalExpandOnHoverChange,
	tabSearchPinned,
	moveGroupToNewWindowAvailable,
	moveTabsToNewWindowAvailable,
	onSetTabTitle,
	shortcutLabels,
	onClose,
}: TabStripContextMenuProps): ReactNode {
	const messages = useTabStripMessages();
	const run = (action: () => void) => () => {
		action();
		onClose();
	};

	if (state.kind === 'strip') {
		const vertical = orientation === TabStripOrientation.Vertical;
		return (
			<ContextMenuContent aria-label={messages.tabStripMenu}>
				<ContextMenuItem
					disabled={onOrientationChange === undefined}
					onClick={run(() =>
						onOrientationChange?.(
							vertical ? TabStripOrientation.Horizontal : TabStripOrientation.Vertical,
						),
					)}
				>
					{vertical ? messages.closeVerticalTabs : messages.openVerticalTabs}
				</ContextMenuItem>
			</ContextMenuContent>
		);
	}

	if (state.kind === 'combo-end') {
		return (
			<ContextMenuContent aria-label={messages.searchTabsMenu}>
				<ContextMenuCheckboxItem
					checked={tabSearchPinned}
					onClick={run(() =>
						actions.execute(
							TAB_STRIP_ACTION_IDS.setTabSearchPinned,
							{ value: !tabSearchPinned },
							'context-menu',
						),
					)}
				>
					{tabSearchPinned ? messages.unpinSearchTabs : messages.pinSearchTabs}
				</ContextMenuCheckboxItem>
			</ContextMenuContent>
		);
	}

	if (state.kind === 'collapse') {
		return (
			<ContextMenuContent aria-label={messages.verticalTabStripMenu}>
				<ContextMenuCheckboxItem
					checked={verticalExpandOnHover}
					disabled={onVerticalExpandOnHoverChange === undefined}
					onClick={run(() =>
						actions.execute(
							TAB_STRIP_ACTION_IDS.setVerticalExpandOnHover,
							{ value: !verticalExpandOnHover },
							'context-menu',
						),
					)}
				>
					{messages.expandTabStripOnHover}
				</ContextMenuCheckboxItem>
			</ContextMenuContent>
		);
	}

	if (state.kind === 'new-tab') {
		const recentGroup =
			snapshot.mostRecentGroupId === null
				? null
				: (snapshot.groups.find((group) => group.id === snapshot.mostRecentGroupId) ?? null);
		const recentGroupTabCount = recentGroup
			? snapshot.tabs.filter((tab) => tab.group === recentGroup.id).length
			: 0;
		let recentGroupLabel = messages.newTabInNoGroups;
		if (recentGroup) {
			recentGroupLabel = messages.formatNewTabInGroup(recentGroup.title, recentGroupTabCount);
		}
		return (
			<ContextMenuContent aria-label={messages.newTabMenu}>
				<ContextMenuItem
					disabled={
						actions.getCreationAvailability({ groupId: null, kind: 'new-tab' }).kind !== 'enabled'
					}
					onClick={run(() =>
						actions.createTab({ groupId: null, title: messages.newTabTitle }, 'context-menu'),
					)}
				>
					{messages.newTab}
					{shortcutLabels?.newTab === undefined ? null : (
						<ContextMenuShortcut>{shortcutLabels.newTab}</ContextMenuShortcut>
					)}
				</ContextMenuItem>
				<ContextMenuItem
					disabled={
						!recentGroup ||
						actions.getCreationAvailability({
							groupId: recentGroup.id,
							kind: 'new-tab',
						}).kind !== 'enabled'
					}
					onClick={run(() => {
						if (recentGroup) {
							actions.createTab(
								{
									groupId: recentGroup.id,
									title: messages.newTabTitle,
								},
								'context-menu',
							);
						}
					})}
				>
					{recentGroupLabel}
				</ContextMenuItem>
				<ContextMenuSeparator />
				<ContextMenuItem
					disabled={actions.getCreationAvailability({ kind: 'new-group' }).kind !== 'enabled'}
					onClick={run(() =>
						actions.execute(TAB_STRIP_ACTION_IDS.createGroupWithTab, undefined, 'context-menu'),
					)}
				>
					{messages.newGroup}
				</ContextMenuItem>
			</ContextMenuContent>
		);
	}

	if (state.kind === 'group') {
		const group = snapshot.groups.find((candidate) => candidate.id === state.id);
		if (!group) return null;
		const commitGroupAction = (action: 'close' | 'ungroup') => {
			if (action === 'close') {
				actions.execute(
					TAB_STRIP_ACTION_IDS.closeTabs,
					{ closeSource: 'non-ui', tabIds: getGroupCloseTabIds(snapshot.tabs, group.id) },
					'context-menu',
				);
			} else {
				actions.execute(TAB_STRIP_ACTION_IDS.ungroup, { groupId: group.id }, 'context-menu');
			}
			onClose();
		};
		return (
			<ContextMenuContent className={styles['ct-group-editor']} aria-label={messages.groupOptions}>
				<GroupTitleInput
					key={group.id}
					label={messages.groupName}
					placeholder={messages.groupNamePlaceholder}
					title={group.title}
					onClose={onClose}
					onCommit={(title) =>
						actions.execute(
							TAB_STRIP_ACTION_IDS.updateGroup,
							{ groupId: group.id, title },
							'context-menu',
						)
					}
				/>
				<div
					className={styles['ct-group-editor__colors']}
					role='radiogroup'
					aria-label={messages.groupColor}
				>
					{GROUP_COLOR_NAMES.map((color) => (
						<button
							key={color}
							type='button'
							role='radio'
							aria-checked={group.color === color}
							aria-label={messages.groupColorNames[color]}
							tabIndex={group.color === color ? 0 : -1}
							className={styles['ct-color']}
							data-selected={group.color === color || undefined}
							style={
								{
									'--ob-comp-tab-strip-group-option-color': GROUP_COLORS[color],
								} satisfies CSSProperties
							}
							onClick={() =>
								actions.execute(
									TAB_STRIP_ACTION_IDS.updateGroup,
									{ color, groupId: group.id },
									'context-menu',
								)
							}
							onKeyDown={(event) => {
								const currentIndex = GROUP_COLOR_NAMES.indexOf(color);
								let offset = 0;
								if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') offset = -1;
								else if (event.key === 'ArrowRight' || event.key === 'ArrowDown') offset = 1;
								let nextIndex =
									offset === 0
										? currentIndex
										: (currentIndex + offset + GROUP_COLOR_NAMES.length) % GROUP_COLOR_NAMES.length;
								if (event.key === 'Home') nextIndex = 0;
								else if (event.key === 'End') nextIndex = GROUP_COLOR_NAMES.length - 1;
								else if (offset === 0) return;
								event.preventDefault();
								const nextColor = requireDefined(GROUP_COLOR_NAMES[nextIndex]);
								actions.execute(
									TAB_STRIP_ACTION_IDS.updateGroup,
									{ color: nextColor, groupId: group.id },
									'context-menu',
								);
								const radios =
									event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>(
										'[role="radio"]',
									);
								radios?.[nextIndex]?.focus();
							}}
						/>
					))}
				</div>
				<ContextMenuSeparator />
				<ContextMenuItem
					disabled={
						actions.getCreationAvailability({ groupId: group.id, kind: 'new-tab' }).kind !==
						'enabled'
					}
					onClick={run(() =>
						actions.createTab({ groupId: group.id, title: messages.newTabTitle }, 'context-menu'),
					)}
				>
					<GroupEditorMenuIcon kind='new-tab' />
					<span>{messages.newTabInGroup}</span>
					{shortcutLabels?.newTab === undefined ? null : (
						<ContextMenuShortcut>{shortcutLabels.newTab}</ContextMenuShortcut>
					)}
				</ContextMenuItem>
				{moveGroupToNewWindowAvailable && snapshot.tabs.some((tab) => tab.group !== group.id) ? (
					<ContextMenuItem
						onClick={run(() =>
							actions.execute(
								TAB_STRIP_ACTION_IDS.moveGroupToWindow,
								{ groupId: group.id },
								'context-menu',
							),
						)}
					>
						<GroupEditorMenuIcon kind='move-window' />
						<span>{messages.moveGroupToNewWindow}</span>
					</ContextMenuItem>
				) : null}
				<ContextMenuSeparator />
				<ContextMenuItem onClick={() => commitGroupAction('ungroup')}>
					<GroupEditorMenuIcon kind='ungroup' />
					<span>{messages.ungroup}</span>
				</ContextMenuItem>
				<ContextMenuItem disabled={!closeAllowed} onClick={() => commitGroupAction('close')}>
					<GroupEditorMenuIcon kind='close' />
					<span>{messages.closeGroup}</span>
				</ContextMenuItem>
			</ContextMenuContent>
		);
	}

	const tab = snapshot.tabs.find((candidate) => candidate.id === state.id);
	if (!tab) return null;
	const index = snapshot.tabs.findIndex((candidate) => candidate.id === tab.id);
	const invokedTabIsSelected = snapshot.selectedTabIds.has(tab.id);
	let commandTabs: readonly Readonly<TabData>[] = [tab];
	if (invokedTabIsSelected) {
		commandTabs = snapshot.tabs.filter((candidate) => snapshot.selectedTabIds.has(candidate.id));
	}
	const closeTargets = getTabCloseTargets(snapshot.tabs, snapshot.selectedTabIds, tab.id);
	const commandTabIds = closeTargets.selected;
	const menuDisplayTabCount = invokedTabIsSelected ? commandTabs.length : 1;
	const willPin = commandTabs.some((candidate) => !candidate.pinned);
	const closeOtherTabIds = closeTargets.other;
	const closeTrailingTabIds = closeTargets.trailing;
	const canMoveToNewWindow = snapshot.tabs.length > commandTabs.length;
	const vertical = orientation === TabStripOrientation.Vertical;

	return (
		<ContextMenuContent aria-label={messages.tabOptions}>
			<ContextMenuItem
				disabled={
					actions.getCreationAvailability({
						groupId: tab.group,
						index: index + 1,
						kind: 'new-tab',
					}).kind !== 'enabled'
				}
				onClick={run(() =>
					actions.createTab(
						{
							groupId: tab.group,
							index: index + 1,
							title: messages.newTabTitle,
						},
						'context-menu',
					),
				)}
			>
				{vertical ? messages.newTabBelow : messages.newTabToRight}
				{shortcutLabels?.newTab === undefined ? null : (
					<ContextMenuShortcut>{shortcutLabels.newTab}</ContextMenuShortcut>
				)}
			</ContextMenuItem>
			{onSetTabTitle === undefined ? null : (
				<ContextMenuItem onClick={run(() => onSetTabTitle(tab.id))}>
					{messages.setTabTitle}
					{shortcutLabels?.setTabTitle === undefined ? null : (
						<ContextMenuShortcut>{shortcutLabels.setTabTitle}</ContextMenuShortcut>
					)}
				</ContextMenuItem>
			)}
			<ContextMenuSub>
				<ContextMenuSubTrigger>
					{menuDisplayTabCount === 1 ? messages.addTabToGroup : messages.addTabsToGroup}
				</ContextMenuSubTrigger>
				<ContextMenuSubContent aria-label={messages.tabGroups}>
					<ContextMenuItem
						onClick={run(() =>
							actions.execute(
								TAB_STRIP_ACTION_IDS.createGroup,
								{ tabIds: commandTabIds },
								'context-menu',
							),
						)}
					>
						{messages.newGroup}
					</ContextMenuItem>
					{snapshot.groups
						.filter((group) => group.id !== tab.group)
						.map((group) => (
							<ContextMenuItem
								key={group.id}
								onClick={run(() =>
									actions.execute(
										TAB_STRIP_ACTION_IDS.moveTabsToGroup,
										{ groupId: group.id, tabIds: commandTabIds },
										'context-menu',
									),
								)}
							>
								{group.title || messages.unnamedGroup}
							</ContextMenuItem>
						))}
				</ContextMenuSubContent>
			</ContextMenuSub>
			{commandTabs.some((candidate) => candidate.group !== null) ? (
				<ContextMenuItem
					onClick={run(() =>
						actions.execute(
							TAB_STRIP_ACTION_IDS.moveTabsToGroup,
							{ groupId: null, tabIds: commandTabIds },
							'context-menu',
						),
					)}
				>
					{messages.removeFromGroup}
				</ContextMenuItem>
			) : null}
			<ContextMenuItem
				disabled={!canMoveToNewWindow || !moveTabsToNewWindowAvailable}
				onClick={run(() =>
					actions.execute(
						TAB_STRIP_ACTION_IDS.moveTabsToWindow,
						{ tabIds: commandTabIds },
						'context-menu',
					),
				)}
			>
				{menuDisplayTabCount === 1 ? messages.moveTabToNewWindow : messages.moveTabsToNewWindow}
			</ContextMenuItem>
			<ContextMenuSeparator />
			<ContextMenuItem
				disabled={
					actions.getCreationAvailability({
						kind: 'duplicate',
						tabIds: commandTabIds,
					}).kind !== 'enabled'
				}
				onClick={run(() =>
					actions.execute(
						TAB_STRIP_ACTION_IDS.duplicateTabs,
						{ tabIds: commandTabIds },
						'context-menu',
					),
				)}
			>
				{messages.duplicate}
			</ContextMenuItem>
			<ContextMenuItem
				onClick={run(() =>
					actions.execute(
						TAB_STRIP_ACTION_IDS.setTabsPinned,
						{ pinned: willPin, tabIds: commandTabIds },
						'context-menu',
					),
				)}
			>
				{willPin ? messages.pin : messages.unpin}
			</ContextMenuItem>
			<ContextMenuSeparator />
			<ContextMenuItem
				disabled={!closeAllowed}
				onClick={run(() =>
					actions.execute(
						TAB_STRIP_ACTION_IDS.closeTabs,
						{ closeSource: 'non-ui', tabIds: commandTabIds },
						'context-menu',
					),
				)}
			>
				{messages.close}
				{shortcutLabels?.closeTab === undefined ? null : (
					<ContextMenuShortcut>{shortcutLabels.closeTab}</ContextMenuShortcut>
				)}
			</ContextMenuItem>
			<ContextMenuItem
				disabled={!closeAllowed || closeOtherTabIds.length === 0}
				onClick={run(() =>
					actions.execute(
						TAB_STRIP_ACTION_IDS.closeTabs,
						{ closeSource: 'non-ui', tabIds: closeOtherTabIds },
						'context-menu',
					),
				)}
			>
				{messages.closeOtherTabs}
			</ContextMenuItem>
			<ContextMenuItem
				disabled={!closeAllowed || closeTrailingTabIds.length === 0}
				onClick={run(() =>
					actions.execute(
						TAB_STRIP_ACTION_IDS.closeTabs,
						{ closeSource: 'non-ui', tabIds: closeTrailingTabIds },
						'context-menu',
					),
				)}
			>
				{vertical ? messages.closeTabsBelow : messages.closeTabsToRight}
			</ContextMenuItem>
		</ContextMenuContent>
	);
}
