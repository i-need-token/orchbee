import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@orchbee/ui/components';
import type { ActionRuntime } from '@orchbee/actions';
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
import { WorkbenchSearchSurface } from './components/workbench-search-surface/WorkbenchSearchSurface';
import { WorkbenchTabBar } from './components/workbench-tab-bar/WorkbenchTabBar';
import { EMPTY_WORKBENCH_SEARCH_PROVIDERS } from './constants/search-providers';
import { useWorkbench } from './hooks/useWorkbench';
import { useWorkbenchTabStripMessages } from './hooks/useWorkbenchTabStripMessages';
import type {
	WorkbenchHostActionCallback,
	WorkbenchTerminalCreateCallback,
} from './types/workbench-host-actions';
import type { WorkbenchShortcutPlatform } from './types/workbench-shortcut-platform';
import type { WorkbenchSearchProvider } from './types/workbench-search-provider';
import styles from './Workbench.module.css';

export interface WorkbenchProps {
	readonly actionRuntime: ActionRuntime;
	readonly createCrossWindowDragRuntime?: TabStripCrossWindowDragRuntimeFactory | undefined;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly onNewTerminal?: WorkbenchTerminalCreateCallback | undefined;
	readonly onSettings?: WorkbenchHostActionCallback | undefined;
	readonly searchProviders?: readonly WorkbenchSearchProvider[] | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly tabModel: TabModel;
}

export function Workbench({
	actionRuntime,
	createCrossWindowDragRuntime,
	layoutStorage,
	onNewTerminal,
	onSettings,
	searchProviders = EMPTY_WORKBENCH_SEARCH_PROVIDERS,
	shortcutPlatform,
	tabModel,
}: WorkbenchProps): React.ReactNode {
	const tabStripMessages = useWorkbenchTabStripMessages();
	const view = useWorkbench({
		actionRuntime,
		onNewTerminal,
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
										onNewTerminal={view.newTerminal}
										onSearch={view.search}
										onSettings={view.settings}
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
								actionRuntime={actionRuntime}
								createCrossWindowDragRuntime={createCrossWindowDragRuntime}
								onNewTerminal={view.newTab}
								onOrientationChange={view.onTabStripOrientationChange}
								onVerticalCollapsedChange={view.onVerticalTabStripCollapsedChange}
								orientation={view.tabStripOrientation}
								parentActionScope={view.actionScope}
								reserveSidebarToggleSpace={!view.isSidebarVisible}
								tabModel={tabModel}
								tabStripMessages={tabStripMessages}
								verticalCollapsed={view.verticalTabStripCollapsed}
							/>
							<div className={styles['tabContent']} data-slot='workbench-tab-content'>
								<WorkbenchPanelWorkspace
									actionRuntime={actionRuntime}
									parentActionScope={view.actionScope}
									tabModel={tabModel}
								/>
							</div>
						</div>
					</ResizablePanel>
				</ResizablePanelGroup>
			</div>
			{view.searchState.kind === 'closed' ? null : (
				<WorkbenchSearchSurface
					actionRuntime={actionRuntime}
					actionScope={view.actionScope}
					providers={searchProviders}
					shortcutPlatform={shortcutPlatform}
					state={view.searchState}
					update={view.updateSearch}
				/>
			)}
		</div>
	);
}
