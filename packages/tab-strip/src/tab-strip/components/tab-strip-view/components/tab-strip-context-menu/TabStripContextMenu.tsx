import styles from './TabStripContextMenu.module.css';
import {
	ContextMenuCheckboxItem,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuRadioGroup,
	ContextMenuRadioItem,
	ContextMenuSeparator,
	ContextMenuShortcut,
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
	onNameGroup: TabStripOptions['onNameGroup'];
	onSetTabTitle: ((tabId: string) => void) | undefined;
	newTabChoices: TabStripOptions['newTabChoices'];
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
 * active Tab rather than the menu's target live in Search's Actions.
 */
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
	onNameGroup,
	onSetTabTitle,
	newTabChoices,
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
		// Grouped and other new-Tab variants are Search Actions; this menu only picks how the Tab starts.
		return (
			<ContextMenuContent aria-label={messages.newTabMenu}>
				{newTabChoices?.map((choice) => (
					<ContextMenuItem key={choice.id} onClick={run(choice.select)}>
						<ProductIcon name={choice.icon} size='menu-leading' />
						{choice.label}
						{choice.note === undefined ? null : (
							<ContextMenuShortcut>{choice.note}</ContextMenuShortcut>
						)}
					</ContextMenuItem>
				))}
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
						<ProductIcon name='new-window' size='menu-leading' />
						{messages.moveGroupToNewWindow}
					</ContextMenuItem>
				) : null}
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
	const moveToNewWindow = moveTabsToNewWindowAvailable && snapshot.tabs.length > count;
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
			{groupChoices || grouped || moveToNewWindow ? <ContextMenuSeparator /> : null}
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
			{moveToNewWindow ? (
				<ContextMenuItem
					onClick={run(() =>
						actions.execute(
							TAB_STRIP_ACTION_IDS.moveTabsToWindow,
							{ tabIds: targetIds },
							'context-menu',
						),
					)}
				>
					<ProductIcon name='new-window' size='menu-leading' />
					{count === 1 ? messages.moveTabToNewWindow : messages.moveTabsToNewWindow}
				</ContextMenuItem>
			) : null}
			<ContextMenuSeparator />
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
		</ContextMenuContent>
	);
}
