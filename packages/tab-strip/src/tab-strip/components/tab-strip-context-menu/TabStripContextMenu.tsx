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
	const run = (action: () => void) => () => {
		action();
		onClose();
	};

	if (state.kind === 'combo-end') {
		const combo = comboButtonController.getSnapshot();
		const pinned = combo.tabSearchPinned;
		const noun = 'Search tabs';
		return (
			<ContextMenuContent aria-label={`${noun} menu`}>
				<ContextMenuCheckboxItem
					checked={pinned}
					onClick={run(() => comboButtonController.executeTogglePinned('tab-search'))}
				>
					{pinned ? `Unpin ${noun}` : `Pin ${noun}`}
				</ContextMenuCheckboxItem>
			</ContextMenuContent>
		);
	}

	if (state.kind === 'collapse') {
		return (
			<ContextMenuContent aria-label='Vertical tab strip menu'>
				<ContextMenuCheckboxItem
					checked={verticalExpandOnHover}
					onClick={run(() => onVerticalExpandOnHoverChange?.(!verticalExpandOnHover))}
				>
					Expand tab strip on hover
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
		let recentGroupLabel = 'New tab in group (no groups)';
		if (recentGroup) {
			recentGroupLabel = recentGroup.title
				? `New tab in ${recentGroup.title}`
				: `New tab in ${recentGroupTabCount} ${recentGroupTabCount === 1 ? 'tab' : 'tabs'}`;
		}
		return (
			<ContextMenuContent aria-label='New tab menu'>
				<ContextMenuItem
					onClick={run(() => model.addTab({ title: 'New Tab', pinned: false, group: null }))}
				>
					New tab
				</ContextMenuItem>
				<ContextMenuItem
					disabled={!recentGroup}
					onClick={run(() => {
						if (recentGroup) {
							model.addTab({ title: 'New Tab', pinned: false, group: recentGroup.id });
						}
					})}
				>
					{recentGroupLabel}
				</ContextMenuItem>
				<ContextMenuSeparator />
				<ContextMenuItem onClick={run(() => createNewTabGroup(model))}>New group</ContextMenuItem>
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
			<ContextMenuContent className={styles['ct-group-editor']} aria-label='Group options'>
				<input
					className={styles['ct-group-editor__title']}
					aria-label='Group name'
					autoFocus
					value={group.title}
					placeholder='Name this group'
					onChange={(event) => model.updateGroup(group.id, { title: event.target.value })}
					onKeyDown={(event) => {
						if (event.key !== 'Enter' && event.key !== 'Escape') return;
						event.preventDefault();
						event.stopPropagation();
						onClose();
					}}
				/>
				<div
					className={styles['ct-group-editor__colors']}
					role='radiogroup'
					aria-label='Group color'
				>
					{GROUP_COLOR_NAMES.map((color) => (
						<button
							key={color}
							type='button'
							role='radio'
							aria-checked={group.color === color}
							aria-label={color}
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
					onClick={run(() => model.addTab({ title: 'New Tab', pinned: false, group: group.id }))}
				>
					<GroupEditorMenuIcon kind='new-tab' />
					<span>New tab in group</span>
				</ContextMenuItem>
				{onMoveGroupToNewWindow && snapshot.tabs.some((tab) => tab.group !== group.id) ? (
					<ContextMenuItem onClick={run(() => onMoveGroupToNewWindow(group.id))}>
						<GroupEditorMenuIcon kind='move-window' />
						<span>Move group to new window</span>
					</ContextMenuItem>
				) : null}
				{enableTabGroupFocusing ? (
					<ContextMenuItem
						onClick={run(() =>
							model.setFocusedGroup(snapshot.focusedGroupId === group.id ? null : group.id),
						)}
					>
						<GroupEditorMenuIcon kind='focus' />
						<span>{snapshot.focusedGroupId === group.id ? 'Unfocus group' : 'Focus group'}</span>
					</ContextMenuItem>
				) : null}
				<ContextMenuSeparator />
				<ContextMenuItem onClick={() => commitGroupAction('ungroup')}>
					<GroupEditorMenuIcon kind='ungroup' />
					<span>Ungroup</span>
				</ContextMenuItem>
				<ContextMenuItem disabled={!closeAllowed} onClick={() => commitGroupAction('close')}>
					<GroupEditorMenuIcon kind='close' />
					<span>Close group</span>
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
		<ContextMenuContent aria-label='Tab options'>
			<ContextMenuItem
				onClick={run(() =>
					model.addTab({ title: 'New Tab', group: tab.group, pinned: false }, { index: index + 1 }),
				)}
			>
				{vertical ? 'New tab below' : 'New tab to the right'}
			</ContextMenuItem>
			<ContextMenuSub>
				<ContextMenuSubTrigger>
					{menuDisplayTabCount === 1 ? 'Add tab to group' : 'Add tabs to group'}
				</ContextMenuSubTrigger>
				<ContextMenuSubContent aria-label='Tab groups'>
					<ContextMenuItem onClick={run(() => createGroupForTabs(model, commandTabIds))}>
						New group
					</ContextMenuItem>
					{snapshot.groups
						.filter((group) => group.id !== tab.group)
						.map((group) => (
							<ContextMenuItem
								key={group.id}
								onClick={run(() => model.moveTabsToGroup(commandTabIds, group.id))}
							>
								{group.title || 'Unnamed group'}
							</ContextMenuItem>
						))}
				</ContextMenuSubContent>
			</ContextMenuSub>
			{commandTabs.some((candidate) => candidate.group !== null) ? (
				<ContextMenuItem onClick={run(() => model.moveTabsToGroup(commandTabIds, null))}>
					Remove from group
				</ContextMenuItem>
			) : null}
			<ContextMenuItem
				disabled={!canMoveToNewWindow || !onMoveTabsToNewWindow}
				onClick={run(() => onMoveTabsToNewWindow?.(commandTabIds))}
			>
				{menuDisplayTabCount === 1 ? 'Move tab to new window' : 'Move tabs to new window'}
			</ContextMenuItem>
			<ContextMenuSeparator />
			<ContextMenuItem onClick={run(() => commandTabIds.forEach((id) => model.duplicateTab(id)))}>
				Duplicate
			</ContextMenuItem>
			<ContextMenuItem onClick={run(() => model.setTabsPinned(commandTabIds, willPin))}>
				{willPin ? 'Pin' : 'Unpin'}
			</ContextMenuItem>
			<ContextMenuSeparator />
			<ContextMenuItem
				disabled={!closeAllowed}
				onClick={run(() => requestCloseTabs(commandTabIds))}
			>
				Close
			</ContextMenuItem>
			<ContextMenuItem
				disabled={!closeAllowed || closeOtherTabIds.length === 0}
				onClick={run(() => requestCloseTabs(closeOtherTabIds))}
			>
				Close other tabs
			</ContextMenuItem>
			<ContextMenuItem
				disabled={!closeAllowed || closeTrailingTabIds.length === 0}
				onClick={run(() => requestCloseTabs(closeTrailingTabIds))}
			>
				{vertical ? 'Close tabs below' : 'Close tabs to the right'}
			</ContextMenuItem>
		</ContextMenuContent>
	);
}
