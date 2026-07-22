import { type CSSProperties, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { type TabData, type TabModelSnapshot, type TabModel } from '../../../../model';
import { type TabCloseSource } from '../../../../layout';
import { requireDefined, TabStripOrientation } from '../../../../common';
import { type TabStripComboButtonController } from '../../../../shared';
import {
	GROUP_COLORS,
	GROUP_COLOR_NAMES,
	createNewTabGroup,
	createGroupForTabs,
	contextMenuStyle,
	type MenuState,
} from '../../utils/tab-strip-internals';
import { GroupEditorMenuIcon } from '../group-editor-menu-icon/GroupEditorMenuIcon';
import { useContextMenu } from './hooks/useContextMenu';

export function ContextMenu({
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
	enableTabGroupFocusing = false,
	onClose,
}: {
	state: MenuState;
	snapshot: TabModelSnapshot;
	model: TabModel;
	orientation: TabStripOrientation;
	requestCloseTabs: (tabIds: readonly string[], source?: TabCloseSource) => void;
	closeAllowed: boolean;
	verticalExpandOnHover: boolean;
	onVerticalExpandOnHoverChange?: (enabled: boolean) => void;
	comboButtonController: TabStripComboButtonController;
	onMoveTabsToNewWindow?: (tabIds: readonly string[]) => void;
	onMoveGroupToNewWindow?: (groupId: string) => void;
	enableTabGroupFocusing: boolean;
	onClose: () => void;
}): ReactNode {
	const {
		enterTabGroupSubmenuFromKeyboard,
		setTabGroupSubmenuOpen,
		tabGroupSubmenuOpen,
		tabGroupSubmenuRef,
		tabGroupTriggerRef,
		tabMenuRef,
	} = useContextMenu({ state, onClose });

	const run = (action: () => void) => () => {
		action();
		onClose();
	};
	const moveMenuFocus = (event: React.KeyboardEvent<HTMLDivElement>, container: HTMLDivElement) => {
		const items = Array.from(container.children).filter(
			(child): child is HTMLButtonElement => child instanceof HTMLButtonElement && !child.disabled,
		);
		if (items.length === 0) return;
		const currentIndex =
			document.activeElement instanceof HTMLButtonElement
				? items.indexOf(document.activeElement)
				: -1;
		let nextIndex: number | null = null;
		if (event.key === 'ArrowDown')
			nextIndex = currentIndex < 0 ? 0 : (currentIndex + 1) % items.length;
		else if (event.key === 'ArrowUp')
			nextIndex =
				currentIndex < 0 ? items.length - 1 : (currentIndex - 1 + items.length) % items.length;
		else if (event.key === 'Home') nextIndex = 0;
		else if (event.key === 'End') nextIndex = items.length - 1;
		if (nextIndex === null) return;
		event.preventDefault();
		event.stopPropagation();
		items[nextIndex]?.focus();
	};
	const vertical = orientation === TabStripOrientation.Vertical;
	const style = contextMenuStyle(state);
	if (state.kind === 'combo-end') {
		const combo = comboButtonController.getSnapshot();
		const pinned = combo.tabSearchPinned;
		const noun = 'Search tabs';
		return createPortal(
			<div
				className='ct-menu'
				role='menu'
				aria-label={`${noun} menu`}
				style={style}
				onPointerDown={(event) => event.stopPropagation()}
			>
				<button
					role='menuitemcheckbox'
					aria-checked={pinned}
					onClick={run(() => comboButtonController.executeTogglePinned('tab-search'))}
				>
					{pinned ? `Unpin ${noun}` : `Pin ${noun}`}
				</button>
			</div>,
			document.body,
		);
	}
	if (state.kind === 'collapse') {
		return createPortal(
			<div
				className='ct-menu'
				role='menu'
				aria-label='Vertical tab strip menu'
				style={style}
				onPointerDown={(event) => event.stopPropagation()}
			>
				<button
					role='menuitemcheckbox'
					aria-checked={verticalExpandOnHover}
					onClick={run(() => onVerticalExpandOnHoverChange?.(!verticalExpandOnHover))}
				>
					Expand tab strip on hover
				</button>
			</div>,
			document.body,
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
		if (recentGroup)
			recentGroupLabel = recentGroup.title
				? `New tab in ${recentGroup.title}`
				: `New tab in ${recentGroupTabCount} ${recentGroupTabCount === 1 ? 'tab' : 'tabs'}`;
		return createPortal(
			<div
				className='ct-menu'
				role='menu'
				aria-label='New tab menu'
				style={style}
				onPointerDown={(event) => event.stopPropagation()}
			>
				<button
					role='menuitem'
					onClick={run(() => model.addTab({ title: 'New Tab', pinned: false, group: null }))}
				>
					New tab
				</button>
				<button
					role='menuitem'
					disabled={!recentGroup}
					onClick={run(() => {
						if (recentGroup)
							model.addTab({ title: 'New Tab', pinned: false, group: recentGroup.id });
					})}
				>
					{recentGroupLabel}
				</button>
				<div className='ct-menu__separator' />
				<button role='menuitem' onClick={run(() => createNewTabGroup(model))}>
					New group
				</button>
				<div className='ct-menu__separator' />
			</div>,
			document.body,
		);
	}
	if (state.kind === 'group') {
		const group = snapshot.groups.find((candidate) => candidate.id === state.id);
		if (!group) return null;
		const groupTabs = snapshot.tabs.filter((tab) => tab.group === group.id);
		const commitGroupAction = (action: 'close' | 'ungroup') => {
			if (action === 'close')
				requestCloseTabs(
					groupTabs.map((tab) => tab.id),
					'non-ui',
				);
			else model.ungroup(group.id);
			onClose();
		};
		return createPortal(
			<div
				className='ct-menu ct-group-editor'
				role='menu'
				style={style}
				onPointerDown={(event) => event.stopPropagation()}
			>
				<input
					className='ct-group-editor__title'
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
				<div className='ct-group-editor__colors' role='radiogroup' aria-label='Group color'>
					{GROUP_COLOR_NAMES.map((color) => (
						<button
							key={color}
							role='radio'
							aria-checked={group.color === color}
							aria-label={color}
							tabIndex={group.color === color ? 0 : -1}
							className='ct-color'
							data-selected={group.color === color || undefined}
							style={{ '--ct-color': GROUP_COLORS[color] } satisfies CSSProperties}
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
				<div className='ct-menu__separator' />
				<button
					role='menuitem'
					onClick={run(() =>
						model.addTab({
							title: 'New Tab',
							pinned: false,
							group: group.id,
						}),
					)}
				>
					<GroupEditorMenuIcon kind='new-tab' />
					<span>New tab in group</span>
				</button>
				{onMoveGroupToNewWindow && snapshot.tabs.some((tab) => tab.group !== group.id) ? (
					<button role='menuitem' onClick={run(() => onMoveGroupToNewWindow(group.id))}>
						<GroupEditorMenuIcon kind='move-window' />
						<span>Move group to new window</span>
					</button>
				) : null}
				{enableTabGroupFocusing ? (
					<button
						role='menuitem'
						onClick={run(() =>
							model.setFocusedGroup(snapshot.focusedGroupId === group.id ? null : group.id),
						)}
					>
						<GroupEditorMenuIcon kind='focus' />
						<span>{snapshot.focusedGroupId === group.id ? 'Unfocus group' : 'Focus group'}</span>
					</button>
				) : null}
				<div className='ct-menu__separator' />
				<button role='menuitem' onClick={() => commitGroupAction('ungroup')}>
					<GroupEditorMenuIcon kind='ungroup' />
					<span>Ungroup</span>
				</button>
				<button role='menuitem' disabled={!closeAllowed} onClick={() => commitGroupAction('close')}>
					<GroupEditorMenuIcon kind='close' />
					<span>Close group</span>
				</button>
			</div>,
			document.body,
		);
	}

	const tab = snapshot.tabs.find((candidate) => candidate.id === state.id);
	if (!tab) return null;
	const index = snapshot.tabs.findIndex((candidate) => candidate.id === tab.id);
	// TabStripModel::GetTabsForCommand applies a context-menu command to the
	// complete selection when the invoked tab is selected.
	const invokedTabIsSelected = snapshot.selectedTabIds.has(tab.id);
	let commandTabs: readonly Readonly<TabData>[] = [tab];
	if (invokedTabIsSelected)
		commandTabs = snapshot.tabs.filter((candidate) => snapshot.selectedTabIds.has(candidate.id));
	const commandTabIds = commandTabs.map((candidate) => candidate.id);
	const groupableTabIds = commandTabIds;
	// TabMenuModel calculates plural labels from the selected indices.
	const menuDisplayTabs = invokedTabIsSelected
		? snapshot.tabs.filter((candidate) => snapshot.selectedTabIds.has(candidate.id))
		: [tab];
	const menuDisplayTabCount = menuDisplayTabs.length;
	const willPin = commandTabs.some((candidate) => !candidate.pinned);
	// Port of TabStripModel::GetTabsClosedByCommand. Selected context tabs are
	// all preserved by "close other", and selected "close right" starts after
	// the selection's trailing edge.
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
	const submenuStyle: CSSProperties = {
		left:
			typeof window === 'undefined'
				? state.x + 252
				: Math.max(8, Math.min(state.x + 252, window.innerWidth - 268)),
		top:
			typeof window === 'undefined'
				? state.y + 64
				: Math.max(8, Math.min(state.y + 64, window.innerHeight - 300)),
	};
	return createPortal(
		<div
			ref={tabMenuRef}
			className='ct-menu'
			role='menu'
			tabIndex={-1}
			style={style}
			onPointerDown={(event) => event.stopPropagation()}
			onKeyDown={(event) => {
				if (event.key === 'ArrowRight' && event.target === tabGroupTriggerRef.current) {
					event.preventDefault();
					event.stopPropagation();
					enterTabGroupSubmenuFromKeyboard.current = true;
					setTabGroupSubmenuOpen(true);
					return;
				}
				moveMenuFocus(event, event.currentTarget);
			}}
		>
			<button
				role='menuitem'
				onClick={run(() =>
					model.addTab({ title: 'New Tab', group: tab.group, pinned: false }, { index: index + 1 }),
				)}
			>
				{vertical ? 'New tab below' : 'New tab to the right'}
			</button>
			{groupableTabIds.length > 0 ? (
				<>
					<button
						ref={tabGroupTriggerRef}
						role='menuitem'
						aria-haspopup='menu'
						aria-expanded={tabGroupSubmenuOpen}
						className='ct-menu__submenu-trigger'
						onMouseEnter={() => {
							setTabGroupSubmenuOpen(true);
						}}
						onClick={() => {
							setTabGroupSubmenuOpen(true);
						}}
					>
						{menuDisplayTabCount === 1 ? 'Add tab to group' : 'Add tabs to group'}
					</button>
					{tabGroupSubmenuOpen ? (
						<div
							ref={tabGroupSubmenuRef}
							className='ct-menu ct-menu--submenu'
							role='menu'
							aria-label='Tab groups'
							style={submenuStyle}
							onPointerDown={(event) => event.stopPropagation()}
							onKeyDown={(event) => {
								if (event.key === 'ArrowLeft' || event.key === 'Escape') {
									event.preventDefault();
									event.stopPropagation();
									setTabGroupSubmenuOpen(false);
									tabGroupTriggerRef.current?.focus();
									return;
								}
								moveMenuFocus(event, event.currentTarget);
							}}
						>
							<button
								role='menuitem'
								onClick={run(() => {
									createGroupForTabs(model, groupableTabIds);
								})}
							>
								New group
							</button>
							{snapshot.groups
								.filter((group) => group.id !== tab.group)
								.map((group) => (
									<button
										key={group.id}
										role='menuitem'
										onClick={run(() => model.moveTabsToGroup(groupableTabIds, group.id))}
									>
										{group.title || 'Unnamed group'}
									</button>
								))}
						</div>
					) : null}
					{commandTabs.some((candidate) => candidate.group !== null) ? (
						<button
							role='menuitem'
							onClick={run(() => model.moveTabsToGroup(groupableTabIds, null))}
						>
							Remove from group
						</button>
					) : null}
				</>
			) : null}
			<button
				disabled={!canMoveToNewWindow || !onMoveTabsToNewWindow}
				role='menuitem'
				onClick={run(() => onMoveTabsToNewWindow?.(commandTabIds))}
			>
				{menuDisplayTabCount === 1 ? 'Move tab to new window' : 'Move tabs to new window'}
			</button>
			<div className='ct-menu__separator' />
			<button
				role='menuitem'
				onClick={run(() => commandTabIds.forEach((id) => model.duplicateTab(id)))}
			>
				Duplicate
			</button>
			<button role='menuitem' onClick={run(() => model.setTabsPinned(commandTabIds, willPin))}>
				{willPin ? 'Pin' : 'Unpin'}
			</button>
			<div className='ct-menu__separator' />
			<button
				disabled={!closeAllowed}
				role='menuitem'
				onClick={run(() => requestCloseTabs(commandTabIds))}
			>
				Close
			</button>
			<button
				disabled={!closeAllowed || closeOtherTabIds.length === 0}
				role='menuitem'
				onClick={run(() => requestCloseTabs(closeOtherTabIds))}
			>
				Close other tabs
			</button>
			<button
				disabled={!closeAllowed || closeTrailingTabIds.length === 0}
				role='menuitem'
				onClick={run(() => requestCloseTabs(closeTrailingTabIds))}
			>
				{vertical ? 'Close tabs below' : 'Close tabs to the right'}
			</button>
		</div>,
		document.body,
	);
}
