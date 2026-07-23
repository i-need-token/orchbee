import styles from './TabStripContextMenu.module.css';
import {
	ContextMenuCheckboxItem,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuSub,
	ContextMenuSubContent,
	ContextMenuSubTrigger,
} from '@orchbee/ui/components';
import { type CSSProperties, type ReactNode } from 'react';
import { type TabCloseSource } from '../../../layout';
import { type TabData, type TabModel, type TabModelSnapshot } from '../../../model';
import { requireDefined, TabStripOrientation } from '../../../common';
import { type TabStripComboButtonController } from '../../../shared';
import {
	GROUP_COLORS,
	GROUP_COLOR_NAMES,
	createGroupForTabs,
	createNewTabGroup,
	type MenuState,
} from '../../utils/tab-strip-internals';
import { GroupEditorMenuIcon } from '../group-editor-menu-icon/GroupEditorMenuIcon';
import { useTabStripMessages } from '../../messages';

interface TabStripContextMenuProps {
	state: MenuState;
	snapshot: TabModelSnapshot;
	model: TabModel;
	orientation: TabStripOrientation;
	requestCloseTabs: (tabIds: readonly string[], source?: TabCloseSource) => void;
	closeAllowed: boolean;
	verticalExpandOnHover: boolean;
	onVerticalExpandOnHoverChange?: ((enabled: boolean) => void) | undefined;
	comboButtonController: TabStripComboButtonController;
	onMoveTabsToNewWindow?: ((tabIds: readonly string[]) => void) | undefined;
	onMoveGroupToNewWindow?: ((groupId: string) => void) | undefined;
	enableTabGroupFocusing: boolean;
	onClose: () => void;
}

export function TabStripContextMenu({
	state,
	snapshot,
	model,
	orientation,
	requestCloseTabs,
	closeAllowed,
	verticalExpandOnHover,
	onVerticalExpandOnHoverChange,
	comboButtonController,
	onMoveTabsToNewWindow,
	onMoveGroupToNewWindow,
	enableTabGroupFocusing,
	onClose,
}: TabStripContextMenuProps): ReactNode {
	const messages = useTabStripMessages();
	const run = (action: () => void) => () => {
		action();
		onClose();
	};

	if (state.kind === 'combo-end') {
		const combo = comboButtonController.getSnapshot();
		const pinned = combo.tabSearchPinned;
		return (
			<ContextMenuContent aria-label={messages.searchTabsMenu}>
				<ContextMenuCheckboxItem
					checked={pinned}
					onClick={run(() => comboButtonController.executeTogglePinned('tab-search'))}
				>
					{pinned ? messages.unpinSearchTabs : messages.pinSearchTabs}
				</ContextMenuCheckboxItem>
			</ContextMenuContent>
		);
	}

	if (state.kind === 'collapse') {
		return (
			<ContextMenuContent aria-label={messages.verticalTabStripMenu}>
				<ContextMenuCheckboxItem
					checked={verticalExpandOnHover}
					onClick={run(() => onVerticalExpandOnHoverChange?.(!verticalExpandOnHover))}
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
					onClick={run(() =>
						model.addTab({ title: messages.newTabTitle, pinned: false, group: null }),
					)}
				>
					{messages.newTab}
				</ContextMenuItem>
				<ContextMenuItem
					disabled={!recentGroup}
					onClick={run(() => {
						if (recentGroup) {
							model.addTab({
								title: messages.newTabTitle,
								pinned: false,
								group: recentGroup.id,
							});
						}
					})}
				>
					{recentGroupLabel}
				</ContextMenuItem>
				<ContextMenuSeparator />
				<ContextMenuItem onClick={run(() => createNewTabGroup(model, messages.newTabTitle))}>
					{messages.newGroup}
				</ContextMenuItem>
			</ContextMenuContent>
		);
	}

	if (state.kind === 'group') {
		const group = snapshot.groups.find((candidate) => candidate.id === state.id);
		if (!group) return null;
		const groupTabs = snapshot.tabs.filter((tab) => tab.group === group.id);
		const commitGroupAction = (action: 'close' | 'ungroup') => {
			if (action === 'close') {
				requestCloseTabs(
					groupTabs.map((tab) => tab.id),
					'non-ui',
				);
			} else {
				model.ungroup(group.id);
			}
			onClose();
		};
		return (
			<ContextMenuContent className={styles['ct-group-editor']} aria-label={messages.groupOptions}>
				<input
					className={styles['ct-group-editor__title']}
					aria-label={messages.groupName}
					autoFocus
					value={group.title}
					placeholder={messages.groupNamePlaceholder}
					onChange={(event) => model.updateGroup(group.id, { title: event.target.value })}
					onKeyDown={(event) => {
						event.stopPropagation();
						if (event.key !== 'Enter' && event.key !== 'Escape') return;
						event.preventDefault();
						onClose();
					}}
					onKeyUp={(event) => event.stopPropagation()}
					onPointerDown={(event) => event.stopPropagation()}
					onClick={(event) => event.stopPropagation()}
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
							onClick={() => model.updateGroup(group.id, { color })}
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
								model.updateGroup(group.id, { color: nextColor });
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
					onClick={run(() =>
						model.addTab({ title: messages.newTabTitle, pinned: false, group: group.id }),
					)}
				>
					<GroupEditorMenuIcon kind='new-tab' />
					<span>{messages.newTabInGroup}</span>
				</ContextMenuItem>
				{onMoveGroupToNewWindow && snapshot.tabs.some((tab) => tab.group !== group.id) ? (
					<ContextMenuItem onClick={run(() => onMoveGroupToNewWindow(group.id))}>
						<GroupEditorMenuIcon kind='move-window' />
						<span>{messages.moveGroupToNewWindow}</span>
					</ContextMenuItem>
				) : null}
				{enableTabGroupFocusing ? (
					<ContextMenuItem
						onClick={run(() =>
							model.setFocusedGroup(snapshot.focusedGroupId === group.id ? null : group.id),
						)}
					>
						<GroupEditorMenuIcon kind='focus' />
						<span>
							{snapshot.focusedGroupId === group.id ? messages.unfocusGroup : messages.focusGroup}
						</span>
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
	const commandTabIds = commandTabs.map((candidate) => candidate.id);
	const menuDisplayTabCount = invokedTabIsSelected ? commandTabs.length : 1;
	const willPin = commandTabs.some((candidate) => !candidate.pinned);
	const selectedIndices = snapshot.tabs.flatMap((candidate, candidateIndex) =>
		snapshot.selectedTabIds.has(candidate.id) ? [candidateIndex] : [],
	);
	const lastUnclosedForTrailing = invokedTabIsSelected ? (selectedIndices.at(-1) ?? index) : index;
	const getRelativeCloseTabIds = (lastUnclosedTab: number) =>
		snapshot.tabs.flatMap((candidate, candidateIndex) => {
			if (candidateIndex <= lastUnclosedTab || candidate.pinned) return [];
			if (candidate.id === tab.id) return [];
			if (invokedTabIsSelected && snapshot.selectedTabIds.has(candidate.id)) return [];
			return [candidate.id];
		});
	const closeOtherTabIds = getRelativeCloseTabIds(-1);
	const closeTrailingTabIds = getRelativeCloseTabIds(lastUnclosedForTrailing);
	const canMoveToNewWindow = snapshot.tabs.length > commandTabs.length;
	const vertical = orientation === TabStripOrientation.Vertical;

	return (
		<ContextMenuContent aria-label={messages.tabOptions}>
			<ContextMenuItem
				onClick={run(() =>
					model.addTab(
						{ title: messages.newTabTitle, group: tab.group, pinned: false },
						{ index: index + 1 },
					),
				)}
			>
				{vertical ? messages.newTabBelow : messages.newTabToRight}
			</ContextMenuItem>
			<ContextMenuSub>
				<ContextMenuSubTrigger>
					{menuDisplayTabCount === 1 ? messages.addTabToGroup : messages.addTabsToGroup}
				</ContextMenuSubTrigger>
				<ContextMenuSubContent aria-label={messages.tabGroups}>
					<ContextMenuItem onClick={run(() => createGroupForTabs(model, commandTabIds))}>
						{messages.newGroup}
					</ContextMenuItem>
					{snapshot.groups
						.filter((group) => group.id !== tab.group)
						.map((group) => (
							<ContextMenuItem
								key={group.id}
								onClick={run(() => model.moveTabsToGroup(commandTabIds, group.id))}
							>
								{group.title || messages.unnamedGroup}
							</ContextMenuItem>
						))}
				</ContextMenuSubContent>
			</ContextMenuSub>
			{commandTabs.some((candidate) => candidate.group !== null) ? (
				<ContextMenuItem onClick={run(() => model.moveTabsToGroup(commandTabIds, null))}>
					{messages.removeFromGroup}
				</ContextMenuItem>
			) : null}
			<ContextMenuItem
				disabled={!canMoveToNewWindow || !onMoveTabsToNewWindow}
				onClick={run(() => onMoveTabsToNewWindow?.(commandTabIds))}
			>
				{menuDisplayTabCount === 1 ? messages.moveTabToNewWindow : messages.moveTabsToNewWindow}
			</ContextMenuItem>
			<ContextMenuSeparator />
			<ContextMenuItem onClick={run(() => commandTabIds.forEach((id) => model.duplicateTab(id)))}>
				{messages.duplicate}
			</ContextMenuItem>
			<ContextMenuItem onClick={run(() => model.setTabsPinned(commandTabIds, willPin))}>
				{willPin ? messages.pin : messages.unpin}
			</ContextMenuItem>
			<ContextMenuSeparator />
			<ContextMenuItem
				disabled={!closeAllowed}
				onClick={run(() => requestCloseTabs(commandTabIds))}
			>
				{messages.close}
			</ContextMenuItem>
			<ContextMenuItem
				disabled={!closeAllowed || closeOtherTabIds.length === 0}
				onClick={run(() => requestCloseTabs(closeOtherTabIds))}
			>
				{messages.closeOtherTabs}
			</ContextMenuItem>
			<ContextMenuItem
				disabled={!closeAllowed || closeTrailingTabIds.length === 0}
				onClick={run(() => requestCloseTabs(closeTrailingTabIds))}
			>
				{vertical ? messages.closeTabsBelow : messages.closeTabsToRight}
			</ContextMenuItem>
		</ContextMenuContent>
	);
}
