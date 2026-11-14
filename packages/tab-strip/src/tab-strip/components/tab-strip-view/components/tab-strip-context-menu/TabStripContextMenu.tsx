import styles from './TabStripContextMenu.module.css';
import {
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuRadioGroup,
	ContextMenuRadioItem,
	ContextMenuSeparator,
	ContextMenuSub,
	ContextMenuSubContent,
	ContextMenuSubTrigger,
	ProductIcon,
} from '@orchbee/ui/components';
import { type CSSProperties, type ReactNode } from 'react';
import { type TabGroupColor, type TabModelSnapshot } from '../../../../model/tab-model';
import { getGroupCloseTabIds, getTabCloseTargets } from '../../../../model/tab-close-targets';
import { TabStripOrientation } from '../../../../types/tab-strip-types';
import {
	GROUP_COLORS,
	GROUP_COLOR_NAMES,
	type MenuState,
} from '../../../../utils/tab-strip-internals';
import { useTabStripMessages } from '../../../../hooks/useTabStripMessages';
import type { TabStripActions } from '../../../../types/tab-strip-actions';
import { TAB_STRIP_ACTION_IDS } from '../../../../actions/tab-strip-action-contracts';
import type { TabStripOptions } from '../../../../types/tab-strip-options';
import { TabStripNewTabMenuItems } from './components/tab-strip-new-tab-menu-items/TabStripNewTabMenuItems';

interface TabStripContextMenuProps {
	actions: TabStripActions;
	state: MenuState;
	snapshot: TabModelSnapshot;
	orientation: TabStripOrientation;
	closeAllowed: boolean;
	closeResume: TabStripOptions['closeResume'];
	onNameGroup: TabStripOptions['onNameGroup'];
	onSetTabTitle: ((tabId: string) => void) | undefined;
	newTabSections: TabStripOptions['newTabSections'];
	onClose: () => void;
}

/** A group's color in the leading icon slot, where a group is known by its color. */
function colorSwatch(color: TabGroupColor): ReactNode {
	return (
		<span
			aria-hidden='true'
			className={styles['ct-color-swatch']}
			style={
				{
					'--ob-comp-tab-strip-group-option-color': GROUP_COLORS[color],
				} satisfies CSSProperties
			}
		/>
	);
}

/**
 * The Tab and group menus follow the TUI's composition: rows count the selected set only when the
 * menu opened on one of several selected Tabs, close rows are destructive, and rows that act on the
 * active Tab rather than the menu's target live in Search's Actions. Duplicate and Close Tabs to the
 * right act on the menu's target, as a browser's Tab menu does.
 */
export function TabStripContextMenu({
	actions,
	state,
	snapshot,
	orientation,
	closeAllowed,
	closeResume,
	onNameGroup,
	onSetTabTitle,
	newTabSections,
	onClose,
}: TabStripContextMenuProps): ReactNode {
	const messages = useTabStripMessages();
	const run = (action: () => void) => () => {
		action();
		onClose();
	};

	if (state.kind === 'new-tab') {
		// Grouped and other new-Tab variants are Search Actions; this menu only picks how the Tab starts.
		return (
			<ContextMenuContent aria-label={messages.newTabMenu}>
				<TabStripNewTabMenuItems
					sections={newTabSections}
					onChoose={(choice) => run(choice.select)()}
				/>
			</ContextMenuContent>
		);
	}

	if (state.kind === 'group') {
		const group = snapshot.groups.find((candidate) => candidate.id === state.id);
		if (!group) return null;
		const groupTabIds = getGroupCloseTabIds(snapshot.tabs, group.id);
		return (
			<ContextMenuContent aria-label={messages.groupOptions}>
				<ContextMenuItem
					disabled={
						actions.getCreationAvailability({ groupId: group.id, kind: 'new-tab' }).kind !==
						'enabled'
					}
					onClick={run(() =>
						actions.createTab({ groupId: group.id, title: messages.newTabTitle }, 'context-menu'),
					)}
				>
					<ProductIcon name='add' size='menu-leading' />
					{messages.newTabInGroup}
				</ContextMenuItem>
				{onNameGroup === undefined ? null : (
					<ContextMenuItem
						onClick={run(() =>
							onNameGroup({
								commit: (title) =>
									actions.execute(
										TAB_STRIP_ACTION_IDS.updateGroup,
										{ groupId: group.id, title },
										'context-menu',
									),
								kind: 'rename',
								target: { groupId: group.id, kind: 'group' },
								title: group.title,
							}),
						)}
					>
						<ProductIcon name='edit' size='menu-leading' />
						{messages.rename}
					</ContextMenuItem>
				)}
				<ContextMenuSub>
					<ContextMenuSubTrigger>
						<ProductIcon name='color' size='menu-leading' />
						{messages.groupColor}
					</ContextMenuSubTrigger>
					<ContextMenuSubContent aria-label={messages.groupColor}>
						<ContextMenuRadioGroup value={group.color}>
							{GROUP_COLOR_NAMES.map((color) => (
								<ContextMenuRadioItem
									key={color}
									value={color}
									onClick={run(() =>
										actions.execute(
											TAB_STRIP_ACTION_IDS.updateGroup,
											{ color, groupId: group.id },
											'context-menu',
										),
									)}
								>
									{colorSwatch(color)}
									{messages.groupColorNames[color]}
								</ContextMenuRadioItem>
							))}
						</ContextMenuRadioGroup>
					</ContextMenuSubContent>
				</ContextMenuSub>
				<ContextMenuSeparator />
				<ContextMenuItem
					onClick={run(() =>
						actions.execute(TAB_STRIP_ACTION_IDS.ungroup, { groupId: group.id }, 'context-menu'),
					)}
				>
					<ProductIcon name='ungroup' size='menu-leading' />
					{messages.formatUngroup(groupTabIds.length)}
				</ContextMenuItem>
				<ContextMenuItem
					variant='destructive'
					disabled={!closeAllowed}
					onClick={run(() =>
						actions.execute(
							TAB_STRIP_ACTION_IDS.closeTabs,
							{ closeSource: 'non-ui', tabIds: groupTabIds },
							'context-menu',
						),
					)}
				>
					<ProductIcon name='close' size='menu-leading' />
					{messages.close}
				</ContextMenuItem>
			</ContextMenuContent>
		);
	}

	const tab = snapshot.tabs.find((candidate) => candidate.id === state.id);
	if (!tab) return null;
	const closeTargets = getTabCloseTargets(snapshot.tabs, snapshot.selectedTabIds, tab.id);
	const targetIds = closeTargets.selected;
	const targetIdSet = new Set(targetIds);
	const targets = snapshot.tabs.filter((candidate) => targetIdSet.has(candidate.id));
	const count = targetIds.length;
	const willPin = targets.some((candidate) => !candidate.pinned);
	const otherGroups = snapshot.groups.filter((group) => group.id !== tab.group);
	const grouped = targets.some((candidate) => candidate.group !== null);
	// New group… names the group after the Tab the menu opened on, as the TUI does.
	const nameNewGroup =
		onNameGroup === undefined
			? null
			: () =>
					onNameGroup({
						commit: (title) =>
							actions.execute(
								TAB_STRIP_ACTION_IDS.createGroup,
								{ tabIds: targetIds, title },
								'context-menu',
							),
						kind: 'create',
						target: { kind: 'tabs', tabIds: targetIds },
						title: tab.title,
					});
	const groupChoices = otherGroups.length > 0 || nameNewGroup !== null;
	// Continuing a stopped close concerns the Tab the menu opened on alone, as in the TUI.
	const resumeClose = closeResume?.available(tab.id) === true ? closeResume : null;

	return (
		<ContextMenuContent aria-label={messages.tabOptions}>
			{onSetTabTitle === undefined ? null : (
				<ContextMenuItem onClick={run(() => onSetTabTitle(tab.id))}>
					<ProductIcon name='edit' size='menu-leading' />
					{messages.rename}
				</ContextMenuItem>
			)}
			<ContextMenuItem
				onClick={run(() =>
					actions.execute(
						TAB_STRIP_ACTION_IDS.setTabsPinned,
						{ pinned: willPin, tabIds: targetIds },
						'context-menu',
					),
				)}
			>
				<ProductIcon name='pin' size='menu-leading' />
				{willPin ? messages.formatPinTabs(count) : messages.formatUnpinTabs(count)}
			</ContextMenuItem>
			<ContextMenuItem
				disabled={
					actions.getCreationAvailability({ kind: 'duplicate', tabIds: targetIds }).kind !==
					'enabled'
				}
				onClick={run(() =>
					actions.execute(
						TAB_STRIP_ACTION_IDS.duplicateTabs,
						{ tabIds: targetIds },
						'context-menu',
					),
				)}
			>
				<ProductIcon name='copy' size='menu-leading' />
				{messages.formatDuplicateTabs(count)}
			</ContextMenuItem>
			{groupChoices || grouped ? <ContextMenuSeparator /> : null}
			{groupChoices ? (
				<ContextMenuSub>
					<ContextMenuSubTrigger>
						<ProductIcon name='tabs' size='menu-leading' />
						{messages.formatAddTabsToGroup(count)}
					</ContextMenuSubTrigger>
					<ContextMenuSubContent aria-label={messages.tabGroups}>
						{otherGroups.map((group) => (
							<ContextMenuItem
								key={group.id}
								onClick={run(() =>
									actions.execute(
										TAB_STRIP_ACTION_IDS.moveTabsToGroup,
										{ groupId: group.id, tabIds: targetIds },
										'context-menu',
									),
								)}
							>
								{colorSwatch(group.color)}
								{group.title || messages.unnamedGroup}
							</ContextMenuItem>
						))}
						{otherGroups.length > 0 && nameNewGroup !== null ? <ContextMenuSeparator /> : null}
						{nameNewGroup === null ? null : (
							<ContextMenuItem onClick={run(nameNewGroup)}>
								<ProductIcon name='add' size='menu-leading' />
								{messages.newGroup}
							</ContextMenuItem>
						)}
					</ContextMenuSubContent>
				</ContextMenuSub>
			) : null}
			{grouped ? (
				<ContextMenuItem
					onClick={run(() =>
						actions.execute(
							TAB_STRIP_ACTION_IDS.moveTabsToGroup,
							{ groupId: null, tabIds: targetIds },
							'context-menu',
						),
					)}
				>
					<ProductIcon name='ungroup' size='menu-leading' />
					{messages.formatRemoveTabsFromGroup(count)}
				</ContextMenuItem>
			) : null}
			<ContextMenuSeparator />
			{resumeClose === null ? null : (
				<ContextMenuItem variant='destructive' onClick={run(() => resumeClose.resume(tab.id))}>
					<ProductIcon name='close' size='menu-leading' />
					{messages.resumeTabClose}
				</ContextMenuItem>
			)}
			<ContextMenuItem
				variant='destructive'
				disabled={!closeAllowed}
				onClick={run(() =>
					actions.execute(
						TAB_STRIP_ACTION_IDS.closeTabs,
						{ closeSource: 'non-ui', tabIds: targetIds },
						'context-menu',
					),
				)}
			>
				<ProductIcon name='close' size='menu-leading' />
				{messages.formatCloseTabs(count)}
			</ContextMenuItem>
			{closeTargets.other.length === 0 ? null : (
				<ContextMenuItem
					variant='destructive'
					disabled={!closeAllowed}
					onClick={run(() =>
						actions.execute(
							TAB_STRIP_ACTION_IDS.closeTabs,
							{ closeSource: 'non-ui', tabIds: closeTargets.other },
							'context-menu',
						),
					)}
				>
					<ProductIcon name='close' size='menu-leading' />
					{messages.formatCloseOtherTabs(closeTargets.other.length)}
				</ContextMenuItem>
			)}
			{closeTargets.trailing.length === 0 ? null : (
				<ContextMenuItem
					variant='destructive'
					disabled={!closeAllowed}
					onClick={run(() =>
						actions.execute(
							TAB_STRIP_ACTION_IDS.closeTabs,
							{ closeSource: 'non-ui', tabIds: closeTargets.trailing },
							'context-menu',
						),
					)}
				>
					<ProductIcon name='close' size='menu-leading' />
					{orientation === TabStripOrientation.Vertical
						? messages.closeTabsBelow
						: messages.closeTabsToRight}
				</ContextMenuItem>
			)}
		</ContextMenuContent>
	);
}
