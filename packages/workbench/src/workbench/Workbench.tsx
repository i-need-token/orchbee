import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@orchbee/ui/components';
import type { TabModel, TabStripCrossWindowDragRuntimeFactory } from '@orchbee/tab-strip';
import * as React from 'react';

import {
	MAXIMUM_LEFT_PANEL_WIDTH,
	MINIMUM_LEFT_PANEL_WIDTH,
	type WorkbenchLayoutStorage,
} from '../workbench-layout-storage';
import { WorkbenchSidebarToggle } from './components/workbench-sidebar-toggle/WorkbenchSidebarToggle';
import { WorkbenchSidebar } from './components/workbench-sidebar/WorkbenchSidebar';
import { WorkbenchPanelWorkspace } from './components/workbench-panel-workspace/WorkbenchPanelWorkspace';
import { WorkbenchTabBar } from './components/workbench-tab-bar/WorkbenchTabBar';
import { useWorkbench } from './hooks/useWorkbench';
import { useWorkbenchTabStripMessages } from './hooks/useWorkbenchTabStripMessages';
import type { WorkbenchShortcutPlatform } from './types/workbench-shortcut-platform';
import styles from './Workbench.module.css';

export interface WorkbenchProps {
	readonly createCrossWindowDragRuntime?: TabStripCrossWindowDragRuntimeFactory | undefined;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly onNewTerminal?: (() => void) | undefined;
	readonly onSearch?: (() => void) | undefined;
	readonly onSettings?: (() => void) | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly tabModel: TabModel;
}

export function Workbench({
	createCrossWindowDragRuntime,
	layoutStorage,
	onNewTerminal,
	onSearch,
	onSettings,
	shortcutPlatform,
	tabModel,
}: WorkbenchProps): React.ReactNode {
	const tabStripMessages = useWorkbenchTabStripMessages();
	const view = useWorkbench({
		onNewTerminal,
		onSearch,
		onSettings,
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
			<div className={styles['sidebarToggle']} data-slot='workbench-sidebar-toggle-position'>
				{sidebarToggle}
			</div>
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
										onNewTerminal={onNewTerminal}
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
						<div
							className={styles['rightPanel']}
							data-slot='workbench-right-panel'
							data-tab-strip-orientation={view.tabStripOrientation}
						>
							<WorkbenchTabBar
								createCrossWindowDragRuntime={createCrossWindowDragRuntime}
								onNewTerminal={onNewTerminal}
								onOrientationChange={view.onTabStripOrientationChange}
								orientation={view.tabStripOrientation}
								reserveSidebarToggleSpace={!view.isSidebarVisible}
								tabModel={tabModel}
								tabStripMessages={tabStripMessages}
							/>
							<div className={styles['tabContent']} data-slot='workbench-tab-content'>
								<WorkbenchPanelWorkspace tabModel={tabModel} />
							</div>
						</div>
					</ResizablePanel>
				</ResizablePanelGroup>
			</div>
		</div>
	);
}
