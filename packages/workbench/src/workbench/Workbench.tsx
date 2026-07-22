import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@orchbee/ui/components';
import * as React from 'react';

import {
	MAXIMUM_LEFT_PANEL_WIDTH,
	MINIMUM_LEFT_PANEL_WIDTH,
	type WorkbenchLayoutStorage,
} from '../workbench-layout-storage';
import { WorkbenchSidebarToggle } from './components/workbench-sidebar-toggle/WorkbenchSidebarToggle';
import { WorkbenchSidebar } from './components/workbench-sidebar/WorkbenchSidebar';
import { WorkbenchTabs } from './components/workbench-tabs/WorkbenchTabs';
import { useWorkbench } from './hooks/useWorkbench';
import type { WorkbenchShortcutPlatform } from './types/workbench-shortcut-platform';
import type {
	WorkbenchTab,
	WorkbenchTabColor,
	WorkbenchTabGroup,
	WorkbenchTabGroupUpdate,
} from './types/workbench-tab';
import styles from './Workbench.module.css';

const EMPTY_WORKBENCH_TABS: readonly WorkbenchTab[] = [];
const EMPTY_WORKBENCH_TAB_GROUPS: readonly WorkbenchTabGroup[] = [];

export interface WorkbenchProps {
	readonly activeTabId?: string | undefined;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly onActiveTabChange?: ((tabId: string) => void) | undefined;
	readonly onTabClose?: ((tabId: string) => void) | undefined;
	readonly onTabColorChange?: ((tabId: string, color: WorkbenchTabColor) => void) | undefined;
	readonly onTabCreate?: ((groupId?: string) => void) | undefined;
	readonly onTabGroupAssign?: ((tabId: string, groupId: string | undefined) => void) | undefined;
	readonly onTabGroupChange?:
		| ((groupId: string, update: WorkbenchTabGroupUpdate) => void)
		| undefined;
	readonly onTabGroupClose?: ((groupId: string) => void) | undefined;
	readonly onTabGroupCollapseChange?: ((groupId: string, isCollapsed: boolean) => void) | undefined;
	readonly onTabGroupCreate?: ((tabId: string) => void) | undefined;
	readonly onTabGroupDelete?: ((groupId: string) => void) | undefined;
	readonly onTabGroupMove?: ((groupId: string, targetIndex: number) => void) | undefined;
	readonly onTabGroupUngroup?: ((groupId: string) => void) | undefined;
	readonly onTabMove?:
		| ((tabId: string, targetIndex: number, groupId: string | undefined) => void)
		| undefined;
	readonly onTabPinChange?: ((tabId: string, isPinned: boolean) => void) | undefined;
	readonly onTabRename?: ((tabId: string, label: string) => void) | undefined;
	readonly onTabsClose?: ((tabIds: readonly string[]) => void) | undefined;
	readonly onSearch?: (() => void) | undefined;
	readonly onSettings?: (() => void) | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly tabGroups?: readonly WorkbenchTabGroup[] | undefined;
	readonly tabs?: readonly WorkbenchTab[] | undefined;
}

export function Workbench({
	activeTabId,
	layoutStorage,
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
	onSearch,
	onSettings,
	shortcutPlatform,
	tabGroups = EMPTY_WORKBENCH_TAB_GROUPS,
	tabs = EMPTY_WORKBENCH_TABS,
}: WorkbenchProps): React.ReactNode {
	const view = useWorkbench({
		activeTabId,
		onNewTerminal: onTabCreate,
		onSearch,
		onSettings,
		onTabClose,
		shortcutPlatform,
		storage: layoutStorage,
	});
	const sidebarToggle = (
		<WorkbenchSidebarToggle
			isSidebarVisible={view.isSidebarVisible}
			label={view.sidebarToggleLabel}
			onToggle={view.toggleSidebar}
			shortcutId={view.sidebarShortcutId}
			shortcutLabel={view.sidebarShortcutLabel}
		/>
	);

	return (
		<div className={styles['root']}>
			<div className={styles['sidebarToggle']}>{sidebarToggle}</div>
			<div className={styles['layout']}>
				<ResizablePanelGroup orientation='horizontal' onLayoutChanged={view.onLayoutChanged}>
					{view.isSidebarVisible && (
						<>
							{/* Keep the navigation panel on an explicit pixel contract. */}
							<ResizablePanel
								id='left'
								defaultSize={view.leftPanelSize}
								minSize={`${MINIMUM_LEFT_PANEL_WIDTH}px`}
								maxSize={`${MAXIMUM_LEFT_PANEL_WIDTH}px`}
								groupResizeBehavior='preserve-pixel-size'
								onResize={view.onLeftPanelResize}
							>
								<div className={styles['leftPanel']} data-slot='workbench-left-panel'>
									<WorkbenchSidebar
										label={view.sidebarLabel}
										newTerminalLabel={view.newTerminalLabel}
										newTerminalShortcutId={view.newTerminalShortcutId}
										newTerminalShortcutLabel={view.newTerminalShortcutLabel}
										onNewTerminal={onTabCreate}
										onSearch={onSearch}
										onSettings={onSettings}
										projectLabel={view.projectLabel}
										searchLabel={view.searchLabel}
										searchShortcutId={view.searchShortcutId}
										searchShortcutLabel={view.searchShortcutLabel}
										settingsLabel={view.settingsLabel}
										settingsShortcutId={view.settingsShortcutId}
										settingsShortcutLabel={view.settingsShortcutLabel}
									/>
								</div>
							</ResizablePanel>
							<ResizableHandle className={styles['resizeHandle']} />
						</>
					)}
					<ResizablePanel id='right'>
						<div className={styles['rightPanel']} data-slot='workbench-right-panel'>
							<WorkbenchTabs
								activeTabId={activeTabId}
								ariaLabel={view.tabListLabel}
								closeShortcutLabel={view.closeTabShortcutLabel}
								closeLayoutResetSequence={view.tabCloseLayoutResetSequence}
								onActiveTabChange={onActiveTabChange}
								onTabClose={onTabClose}
								onTabColorChange={onTabColorChange}
								onTabCreate={onTabCreate}
								onTabGroupAssign={onTabGroupAssign}
								onTabGroupChange={onTabGroupChange}
								onTabGroupClose={onTabGroupClose}
								onTabGroupCollapseChange={onTabGroupCollapseChange}
								onTabGroupCreate={onTabGroupCreate}
								onTabGroupDelete={onTabGroupDelete}
								onTabGroupMove={onTabGroupMove}
								onTabGroupUngroup={onTabGroupUngroup}
								onTabMove={onTabMove}
								onTabPinChange={onTabPinChange}
								onTabRename={onTabRename}
								onTabsClose={onTabsClose}
								reserveLeadingControlSpace={!view.isSidebarVisible}
								tabGroups={tabGroups}
								tabs={tabs}
							/>
						</div>
					</ResizablePanel>
				</ResizablePanelGroup>
			</div>
		</div>
	);
}
