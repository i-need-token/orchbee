import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import {
	Button,
	ProductIcon,
	Tabs,
	TabsBar,
	TabsContent,
	TabsItem,
	TabsList,
	TabsTrigger,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import { createWorkbenchTabsState } from '../../../workbench-tabs';
import type {
	WorkbenchTab,
	WorkbenchTabColor,
	WorkbenchTabGroup,
	WorkbenchTabGroupUpdate,
} from '../../types/workbench-tab';
import type { WorkbenchColorChoice } from './components/workbench-color-palette/WorkbenchColorPalette';
import { WorkbenchTabContextMenu } from './components/workbench-tab-context-menu/WorkbenchTabContextMenu';
import { WorkbenchTabRenameDialog } from './components/workbench-tab-rename-dialog/WorkbenchTabRenameDialog';
import { WorkbenchTabGroupHeader } from './components/workbench-tab-group-header/WorkbenchTabGroupHeader';
import { WorkbenchTabTrailingAction } from './components/workbench-tab-trailing-action/WorkbenchTabTrailingAction';
import { useWorkbenchTabDrag } from './hooks/useWorkbenchTabDrag';
import { useWorkbenchTabLayout } from './hooks/useWorkbenchTabLayout';
import { useWorkbenchTabRename } from './hooks/useWorkbenchTabRename';
import { getWorkbenchTabCloseTargets } from './utils/getWorkbenchTabCloseTargets';
import { resolveWorkbenchTabGroupDragActiveTabId } from './utils/resolveWorkbenchTabGroupDragActiveTabId';
import styles from './WorkbenchTabs.module.css';

interface WorkbenchTabsProps {
	readonly activeTabId: string | undefined;
	readonly ariaLabel: string;
	readonly closeLayoutResetSequence: number;
	readonly closeShortcutLabel: string;
	readonly onActiveTabChange: ((tabId: string) => void) | undefined;
	readonly onTabClose: ((tabId: string) => void) | undefined;
	readonly onTabColorChange: ((tabId: string, color: WorkbenchTabColor) => void) | undefined;
	readonly onTabCreate: ((groupId?: string) => void) | undefined;
	readonly onTabGroupAssign: ((tabId: string, groupId: string | undefined) => void) | undefined;
	readonly onTabGroupChange:
		| ((groupId: string, update: WorkbenchTabGroupUpdate) => void)
		| undefined;
	readonly onTabGroupClose: ((groupId: string) => void) | undefined;
	readonly onTabGroupCollapseChange: ((groupId: string, isCollapsed: boolean) => void) | undefined;
	readonly onTabGroupCreate: ((tabId: string) => void) | undefined;
	readonly onTabGroupDelete: ((groupId: string) => void) | undefined;
	readonly onTabGroupMove: ((groupId: string, targetIndex: number) => void) | undefined;
	readonly onTabGroupUngroup: ((groupId: string) => void) | undefined;
	readonly onTabMove:
		| ((tabId: string, targetIndex: number, groupId: string | undefined) => void)
		| undefined;
	readonly onTabPinChange: ((tabId: string, isPinned: boolean) => void) | undefined;
	readonly onTabRename: ((tabId: string, label: string) => void) | undefined;
	readonly onTabsClose: ((tabIds: readonly string[]) => void) | undefined;
	readonly reserveLeadingControlSpace: boolean;
	readonly tabGroups: readonly WorkbenchTabGroup[];
	readonly tabs: readonly WorkbenchTab[];
}

export function WorkbenchTabs({
	activeTabId,
	ariaLabel,
	closeLayoutResetSequence,
	closeShortcutLabel,
	onActiveTabChange,
	onTabClose,
	onTabColorChange,
	onTabCreate,
	onTabGroupAssign,
	onTabGroupChange,
	onTabGroupClose,
	onTabGroupCollapseChange,
	onTabGroupCreate,
	onTabGroupDelete,
	onTabGroupMove,
	onTabGroupUngroup,
	onTabMove,
	onTabPinChange,
	onTabRename,
	onTabsClose,
	reserveLeadingControlSpace,
	tabGroups,
	tabs,
}: WorkbenchTabsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const preparedState = createWorkbenchTabsState({ activeTabId, tabGroups, tabs });
	const orderedTabs = preparedState.tabs;
	const tabDrag = useWorkbenchTabDrag(
		preparedState,
		onTabMove,
		onTabGroupMove,
		(tabId) => {
			if ((activeTabId ?? orderedTabs[0]?.id) !== tabId) onActiveTabChange?.(tabId);
		},
		(groupId) => {
			const nextActiveTabId = resolveWorkbenchTabGroupDragActiveTabId(
				orderedTabs,
				activeTabId,
				groupId,
			);
			if (nextActiveTabId !== undefined && nextActiveTabId !== activeTabId) {
				onActiveTabChange?.(nextActiveTabId);
			}
		},
	);
	const displayTabs = tabDrag.tabs;
	const tabLayout = useWorkbenchTabLayout(
		preparedState.tabs,
		closeLayoutResetSequence,
		tabDrag.tabListRef,
	);
	const displayedActiveTabId =
		tabDrag.draggedTabId ?? preparedState.activeTabId ?? orderedTabs[0]?.id;
	const populatedTabGroups = tabDrag.tabGroups.filter((group) =>
		displayTabs.some((tab) => tab.groupId === group.id),
	);
	const draggedTabGroupId = displayTabs.find((tab) => tab.id === tabDrag.draggedTabId)?.groupId;
	const draggedTabGroupColor = populatedTabGroups.find(
		(group) => group.id === draggedTabGroupId,
	)?.color;
	const tabRename = useWorkbenchTabRename(onTabRename);
	const groupRename = useWorkbenchTabRename(
		onTabGroupChange === undefined
			? undefined
			: (groupId, label) => onTabGroupChange(groupId, { label }),
	);
	const colorChoices: readonly WorkbenchColorChoice[] = [
		{ label: intl.formatMessage(workbenchMessages.colorDefault), value: 'default' },
		{ label: intl.formatMessage(workbenchMessages.colorGray), value: 'gray' },
		{ label: intl.formatMessage(workbenchMessages.colorBlue), value: 'blue' },
		{ label: intl.formatMessage(workbenchMessages.colorPurple), value: 'purple' },
		{ label: intl.formatMessage(workbenchMessages.colorPink), value: 'pink' },
		{ label: intl.formatMessage(workbenchMessages.colorRed), value: 'red' },
		{ label: intl.formatMessage(workbenchMessages.colorOrange), value: 'orange' },
		{ label: intl.formatMessage(workbenchMessages.colorYellow), value: 'yellow' },
		{ label: intl.formatMessage(workbenchMessages.colorGreen), value: 'green' },
		{ label: intl.formatMessage(workbenchMessages.colorCyan), value: 'cyan' },
	];
	const groupColorChoices: readonly WorkbenchColorChoice[] = [
		{ label: intl.formatMessage(workbenchMessages.colorGray), value: 'gray' },
		{ label: intl.formatMessage(workbenchMessages.colorBlue), value: 'blue' },
		{ label: intl.formatMessage(workbenchMessages.colorPurple), value: 'purple' },
		{ label: intl.formatMessage(workbenchMessages.colorPink), value: 'pink' },
		{ label: intl.formatMessage(workbenchMessages.colorRed), value: 'red' },
		{ label: intl.formatMessage(workbenchMessages.colorOrange), value: 'orange' },
		{ label: intl.formatMessage(workbenchMessages.colorYellow), value: 'yellow' },
		{ label: intl.formatMessage(workbenchMessages.colorGreen), value: 'green' },
		{ label: intl.formatMessage(workbenchMessages.colorCyan), value: 'cyan' },
	];
	const addToGroupLabel = intl.formatMessage(workbenchMessages.addTabToGroup);
	const addToNewGroupLabel = intl.formatMessage(workbenchMessages.addTabToNewGroup);
	const closeAllLabel = intl.formatMessage(workbenchMessages.closeAllTabs);
	const closeCurrentLabel = intl.formatMessage(workbenchMessages.closeCurrentTab);
	const closeOthersLabel = intl.formatMessage(workbenchMessages.closeOtherTabs);
	const closeRightLabel = intl.formatMessage(workbenchMessages.closeTabsToRight);
	const renameLabel = intl.formatMessage(workbenchMessages.renameTab);
	const removeFromGroupLabel = intl.formatMessage(workbenchMessages.removeTabFromGroup);
	const tabColorLabel = intl.formatMessage(workbenchMessages.tabColor);
	if (displayTabs.length === 0 && !reserveLeadingControlSpace && onTabCreate === undefined)
		return null;

	return (
		<>
			<Tabs
				defaultValue={preparedState.activeTabId === undefined ? orderedTabs[0]?.id : undefined}
				value={preparedState.activeTabId}
				variant='workspace'
				onValueChange={(value) => {
					if (typeof value === 'string') onActiveTabChange?.(value);
				}}
			>
				<TabsBar>
					{reserveLeadingControlSpace && (
						<div
							aria-hidden='true'
							className={styles['leadingControlSpace']}
							data-slot='workbench-sidebar-toggle-space'
						/>
					)}
					<div className={styles['tabCluster']} onPointerLeave={tabLayout.onPointerLeave}>
						{displayTabs.length > 0 && (
							<TabsList
								aria-label={ariaLabel}
								className={styles['tabList']}
								data-hover-suppressed={tabDrag.hoverSuppressed ? true : undefined}
								data-reordering={tabDrag.isReordering ? true : undefined}
								ref={tabDrag.tabListRef}
								variant='workspace'
								onKeyDown={tabDrag.onKeyDown}
								onLostPointerCapture={tabDrag.onLostPointerCapture}
								onPointerCancel={tabDrag.onPointerCancel}
								onPointerMove={tabDrag.onPointerMove}
								onPointerUp={tabDrag.onPointerUp}
							>
								{displayTabs.map((tab, tabIndex) => {
									const tabLayoutProperties = tabLayout.getTabLayoutProperties(tab.id);
									const closeLabel = intl.formatMessage(workbenchMessages.closeTab, {
										tabLabel: tab.label,
									});
									const closeTargets = getWorkbenchTabCloseTargets(displayTabs, tabIndex);
									const dragProperties = tabDrag.getTabDragProperties(tab.id);
									const color = tab.color ?? 'default';
									const isPinned = tab.isPinned === true;
									const group =
										tab.groupId === undefined
											? undefined
											: populatedTabGroups.find(({ id }) => id === tab.groupId);
									const previousTab = displayTabs[tabIndex - 1];
									const startsGroup =
										!isPinned && group !== undefined && previousTab?.groupId !== group.id;
									const isGroupCollapsed =
										group?.isCollapsed === true && tabDrag.draggedTabId !== tab.id;
									const pinLabel = intl.formatMessage(
										isPinned ? workbenchMessages.unpinTab : workbenchMessages.pinTab,
									);
									const groupHeader =
										startsGroup && group !== undefined ? (
											<WorkbenchTabGroupHeader
												closeLabel={intl.formatMessage(workbenchMessages.closeTabGroup)}
												collapseLabel={intl.formatMessage(
													group.isCollapsed === true
														? workbenchMessages.expandTabGroup
														: workbenchMessages.collapseTabGroup,
													{ groupLabel: group.label },
												)}
												colorChoices={groupColorChoices}
												colorLabel={intl.formatMessage(workbenchMessages.tabGroupColor)}
												deleteLabel={intl.formatMessage(workbenchMessages.deleteTabGroup)}
												dragProperties={tabDrag.getTabGroupDragProperties(group.id)}
												group={group}
												newTabLabel={intl.formatMessage(workbenchMessages.newTabInGroup)}
												onClose={
													onTabGroupClose === undefined
														? undefined
														: () => onTabGroupClose(group.id)
												}
												onCollapseToggle={
													onTabGroupCollapseChange === undefined
														? undefined
														: () => onTabGroupCollapseChange(group.id, group.isCollapsed !== true)
												}
												onColorChange={
													onTabGroupChange === undefined
														? undefined
														: (nextColor) => onTabGroupChange(group.id, { color: nextColor })
												}
												onDelete={
													onTabGroupDelete === undefined
														? undefined
														: () => onTabGroupDelete(group.id)
												}
												onNewTab={
													onTabCreate === undefined ? undefined : () => onTabCreate(group.id)
												}
												onRename={
													onTabGroupChange === undefined
														? undefined
														: () => groupRename.open(group.id, group.label)
												}
												onUngroup={
													onTabGroupUngroup === undefined
														? undefined
														: () => onTabGroupUngroup(group.id)
												}
												renameLabel={intl.formatMessage(workbenchMessages.renameTabGroup)}
												ungroupLabel={intl.formatMessage(workbenchMessages.ungroupTabs)}
											/>
										) : null;

									if (isGroupCollapsed) {
										return startsGroup ? (
											<React.Fragment key={tab.id}>{groupHeader}</React.Fragment>
										) : null;
									}

									return (
										<React.Fragment key={tab.id}>
											{groupHeader}
											<WorkbenchTabContextMenu
												addToGroupLabel={addToGroupLabel}
												addToNewGroupLabel={addToNewGroupLabel}
												closeAllLabel={closeAllLabel}
												closeLabel={closeCurrentLabel}
												closeOthersLabel={closeOthersLabel}
												closeRightLabel={closeRightLabel}
												closeShortcutLabel={closeShortcutLabel}
												color={color}
												colorChoices={colorChoices}
												currentGroupId={tab.groupId}
												groupChoices={populatedTabGroups}
												isPinned={isPinned}
												onClose={
													onTabClose === undefined
														? undefined
														: () => {
																tabLayout.releaseWidthLock();
																onTabClose(tab.id);
															}
												}
												onCloseAll={
													onTabsClose === undefined
														? undefined
														: () => {
																tabLayout.releaseWidthLock();
																onTabsClose(closeTargets.allTabIds);
															}
												}
												onCloseOthers={
													onTabsClose === undefined || closeTargets.otherTabIds.length === 0
														? undefined
														: () => {
																tabLayout.releaseWidthLock();
																onTabsClose(closeTargets.otherTabIds);
															}
												}
												onCloseRight={
													onTabsClose === undefined || closeTargets.rightTabIds.length === 0
														? undefined
														: () => {
																tabLayout.releaseWidthLock();
																onTabsClose(closeTargets.rightTabIds);
															}
												}
												onColorChange={
													onTabColorChange === undefined
														? undefined
														: (nextColor) => onTabColorChange(tab.id, nextColor)
												}
												onCreateGroup={
													onTabGroupCreate === undefined
														? undefined
														: () => onTabGroupCreate(tab.id)
												}
												onGroupAssign={
													onTabGroupAssign === undefined
														? undefined
														: (groupId) => onTabGroupAssign(tab.id, groupId)
												}
												onGroupRemove={
													onTabGroupAssign === undefined || tab.groupId === undefined
														? undefined
														: () => onTabGroupAssign(tab.id, undefined)
												}
												onPinToggle={
													onTabPinChange === undefined
														? undefined
														: () => onTabPinChange(tab.id, !isPinned)
												}
												onRename={
													onTabRename === undefined
														? undefined
														: () => tabRename.open(tab.id, tab.label)
												}
												pinLabel={pinLabel}
												renameLabel={renameLabel}
												removeFromGroupLabel={removeFromGroupLabel}
												tabColorLabel={tabColorLabel}
												trigger={
													<TabsItem
														className={styles['tabItem']}
														data-dragging={dragProperties['data-dragging']}
														data-workbench-tab-active={
															displayedActiveTabId === tab.id ? true : undefined
														}
														data-workbench-tab-color={color}
														data-workbench-tab-group-color={group?.color}
														data-workbench-tab-group-dragging={
															tabDrag.draggedGroupId !== undefined &&
															tabDrag.draggedGroupId === group?.id
																? true
																: undefined
														}
														data-workbench-tab-item-id={tab.id}
														data-workbench-tab-member-group-id={group?.id}
														data-workbench-tab-motion-key={`tab:${tab.id}`}
														data-workbench-tab-pinned={isPinned ? true : undefined}
														data-workbench-tab-reorderable={
															dragProperties.reorderable ? true : undefined
														}
														data-workbench-tab-visual-item='tab'
														data-workbench-tab-width-locked={
															tabLayoutProperties['data-workbench-tab-width-locked']
														}
														onClickCapture={dragProperties.onClickCapture}
														onLostPointerCapture={dragProperties.onLostPointerCapture}
														onPointerCancel={dragProperties.onPointerCancel}
														onPointerDown={dragProperties.onPointerDown}
														onPointerMove={dragProperties.onPointerMove}
														onPointerUp={dragProperties.onPointerUp}
													>
														<TabsTrigger className={styles['trigger']} value={tab.id}>
															<span className={styles['tabContent']}>
																{tab.icon !== undefined && (
																	<span
																		className={styles['tabIcon']}
																		data-workbench-tab-icon={tab.icon}
																	>
																		<ProductIcon name={tab.icon} />
																	</span>
																)}
																{color !== 'default' && (
																	<span className={styles['tabColor']} data-color={color} />
																)}
																<span className={styles['label']}>{tab.label}</span>
															</span>
														</TabsTrigger>
														<WorkbenchTabTrailingAction
															closeLabel={closeLabel}
															isPinned={isPinned}
															onClose={
																onTabClose === undefined ? undefined : () => onTabClose(tab.id)
															}
															onPointerCloseStart={tabLayout.beginPointerClose}
															onUnpin={
																onTabPinChange === undefined
																	? undefined
																	: () => onTabPinChange(tab.id, false)
															}
															tabId={tab.id}
															unpinLabel={pinLabel}
														/>
													</TabsItem>
												}
											/>
										</React.Fragment>
									);
								})}
								<div
									aria-hidden='true'
									className={styles['dragPlaceholder']}
									data-color={draggedTabGroupColor}
									ref={tabDrag.dragPlaceholderRef}
								/>
								<div
									aria-hidden='true'
									className={styles['dragOverlay']}
									data-workbench-tab-drag-overlay='true'
									ref={tabDrag.dragOverlayRef}
								/>
							</TabsList>
						)}
						{onTabCreate !== undefined && (
							<Tooltip>
								<TooltipTrigger
									render={
										<Button
											aria-label={intl.formatMessage(workbenchMessages.addTab)}
											className={styles['addButton']}
											data-slot='workbench-tab-add'
											size='icon-xs'
											type='button'
											variant='ghost'
											onClick={() => {
												onTabCreate();
											}}
										/>
									}
								>
									<ProductIcon name='add' />
								</TooltipTrigger>
								<TooltipContent side='bottom'>
									{intl.formatMessage(workbenchMessages.addTab)}
								</TooltipContent>
							</Tooltip>
						)}
					</div>
				</TabsBar>
				{orderedTabs.map((tab) => (
					<TabsContent key={tab.id} data-workbench-tab-id={tab.id} value={tab.id} />
				))}
			</Tabs>
			<WorkbenchTabRenameDialog
				cancelLabel={intl.formatMessage(workbenchMessages.renameTabCancel)}
				canSubmit={tabRename.canSubmit}
				confirmLabel={intl.formatMessage(workbenchMessages.renameTabConfirm)}
				description={intl.formatMessage(workbenchMessages.renameTabDescription, {
					tabLabel: tabRename.originalLabel,
				})}
				draftLabel={tabRename.draftLabel}
				inputLabel={intl.formatMessage(workbenchMessages.renameTabInputLabel)}
				isOpen={tabRename.isOpen}
				title={intl.formatMessage(workbenchMessages.renameTabTitle)}
				onCancel={tabRename.cancel}
				onDraftLabelChange={tabRename.setDraftLabel}
				onSubmit={tabRename.submit}
			/>
			<WorkbenchTabRenameDialog
				cancelLabel={intl.formatMessage(workbenchMessages.renameTabCancel)}
				canSubmit={groupRename.canSubmit}
				confirmLabel={intl.formatMessage(workbenchMessages.renameTabConfirm)}
				description={intl.formatMessage(workbenchMessages.renameTabGroupDescription, {
					groupLabel: groupRename.originalLabel,
				})}
				draftLabel={groupRename.draftLabel}
				inputLabel={intl.formatMessage(workbenchMessages.tabGroupName)}
				isOpen={groupRename.isOpen}
				title={intl.formatMessage(workbenchMessages.renameTabGroupTitle)}
				onCancel={groupRename.cancel}
				onDraftLabelChange={groupRename.setDraftLabel}
				onSubmit={groupRename.submit}
			/>
		</>
	);
}
