import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from '@orchbee/ui/components';
import type { ActionRuntime } from '@orchbee/actions';
import type { TabStripCrossWindowDragRuntimeFactory } from '@orchbee/tab-strip';
import * as React from 'react';

import {
	MAXIMUM_LEFT_PANEL_WIDTH,
	MINIMUM_LEFT_PANEL_WIDTH,
	type WorkbenchLayoutStorage,
} from '../workbench-layout-storage';
import { WorkbenchSidebarToggle } from './components/workbench-sidebar-toggle/WorkbenchSidebarToggle';
import { WorkbenchConfirmationPopover } from './components/workbench-confirmation-popover/WorkbenchConfirmationPopover';
import { WorkbenchSidebar } from './components/workbench-sidebar/WorkbenchSidebar';
import { WorkbenchPanelWorkspace } from './components/workbench-panel-workspace/WorkbenchPanelWorkspace';
import { WorkbenchSearchSurface } from './components/workbench-search-surface/WorkbenchSearchSurface';
import { WorkbenchTabBar } from './components/workbench-tab-bar/WorkbenchTabBar';
import { EMPTY_WORKBENCH_SEARCH_PROVIDERS } from './constants/search-providers';
import { useWorkbench } from './hooks/useWorkbench';
import { useWorkbenchActionShortcutLabel } from './hooks/useWorkbenchActionShortcutLabel';
import { useWorkbenchTabStripMessages } from './hooks/useWorkbenchTabStripMessages';
import type { WorkbenchTerminalActionShortcutLabels } from './types/workbench-terminal-action-shortcut-labels';
import type {
	WorkbenchHostActionCallback,
	WorkbenchNavigationPort,
} from './types/workbench-host-actions';
import type { WorkbenchShortcutPlatform } from './types/workbench-shortcut-platform';
import type { WorkbenchSearchProvider } from './types/workbench-search-provider';
import type { WorkbenchTerminalSurface } from './types/workbench-terminal-surface';
import type { WorkbenchProjectFolderAcquisition } from './types/workbench-project-folder-acquisition';
import type { WorkbenchProjectWorkspaceRuntime } from './model/WorkbenchProjectWorkspaceRuntime';
import {
	defaultWorkbenchShortcutBindings,
	type WorkbenchShortcutBindings,
} from './utils/workbench-shortcuts';
import styles from './Workbench.module.css';

export interface WorkbenchProps {
	readonly actionRuntime: ActionRuntime;
	readonly createCrossWindowDragRuntime?: TabStripCrossWindowDragRuntimeFactory | undefined;
	readonly isShortcutScopeActive?: boolean | undefined;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly onSettings?: WorkbenchHostActionCallback | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly projectFolderAcquisition?: WorkbenchProjectFolderAcquisition | undefined;
	readonly searchProviders?: readonly WorkbenchSearchProvider[] | undefined;
	readonly shortcutBindings?: WorkbenchShortcutBindings | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalActionShortcutLabels?: WorkbenchTerminalActionShortcutLabels | undefined;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
}

export function Workbench({
	actionRuntime,
	createCrossWindowDragRuntime,
	isShortcutScopeActive = true,
	layoutStorage,
	navigation,
	onSettings,
	projectWorkspaceRuntime,
	projectFolderAcquisition,
	searchProviders = EMPTY_WORKBENCH_SEARCH_PROVIDERS,
	shortcutBindings,
	shortcutPlatform,
	terminalActionShortcutLabels,
	terminalSurface,
}: WorkbenchProps): React.ReactNode {
	const tabStripMessages = useWorkbenchTabStripMessages();
	const effectiveShortcutBindings =
		shortcutBindings ?? defaultWorkbenchShortcutBindings(shortcutPlatform);
	const shortcutLabelForAction = useWorkbenchActionShortcutLabel({
		shortcutBindings: effectiveShortcutBindings,
		shortcutPlatform,
		terminalActionShortcutLabels,
	});
	const view = useWorkbench({
		actionRuntime,
		isShortcutScopeActive,
		navigation,
		onSettings,
		projectWorkspaceRuntime,
		shortcutBindings: effectiveShortcutBindings,
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
	let projectWorkspaceConfirmationSurface: React.ReactNode = null;
	if (view.projectWorkspaceConfirmation?.anchor === null) {
		projectWorkspaceConfirmationSurface = (
			<AlertDialog
				open
				onOpenChange={(open) => {
					if (!open) view.cancelProjectWorkspaceConfirmation();
				}}
			>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>{view.projectWorkspaceConfirmation.title}</AlertDialogTitle>
						<AlertDialogDescription>
							{view.projectWorkspaceConfirmation.description}
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel onClick={view.cancelProjectWorkspaceConfirmation}>
							{view.projectWorkspaceConfirmation.cancelLabel}
						</AlertDialogCancel>
						<AlertDialogAction variant='destructive' onClick={view.confirmProjectWorkspaceAction}>
							{view.projectWorkspaceConfirmation.confirmLabel}
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		);
	} else if (view.projectWorkspaceConfirmation !== null) {
		projectWorkspaceConfirmationSurface = (
			<WorkbenchConfirmationPopover
				anchor={view.projectWorkspaceConfirmation.anchor}
				cancelLabel={view.projectWorkspaceConfirmation.cancelLabel}
				confirmLabel={view.projectWorkspaceConfirmation.confirmLabel}
				description={view.projectWorkspaceConfirmation.description}
				onCancel={view.cancelProjectWorkspaceConfirmation}
				onConfirm={view.confirmProjectWorkspaceAction}
				title={view.projectWorkspaceConfirmation.title}
			/>
		);
	}

	return (
		<div className={styles['root']} ref={view.shortcutScopeReference}>
			<div className={styles['sidebarToggle']} data-slot='workbench-sidebar-toggle-position'>
				{sidebarToggle}
			</div>
			<div className={styles['layout']}>
				<ResizablePanelGroup
					disableCursor
					onLayoutChanged={view.onLayoutChanged}
					orientation='horizontal'
				>
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
										activateStandalone={view.activateStandalone}
										activateWorkspace={view.activateWorkspace}
										addProjectDirectory={view.addProjectDirectory}
										createProject={view.createProject}
										createWorkspace={view.createWorkspace}
										deleteWorkspace={view.deleteWorkspace}
										duplicateWorkspace={view.duplicateWorkspace}
										label={view.sidebarLabel}
										onSearch={view.search}
										onSettings={view.settings}
										projectLabel={view.projectLabel}
										projectNavigationMessages={view.projectNavigationMessages}
										projectFolderAcquisition={projectFolderAcquisition}
										projectWorkspaceState={view.projectWorkspaceState}
										removeProject={view.removeProject}
										removeProjectDirectory={view.removeProjectDirectory}
										reorderProject={view.reorderProject}
										reorderWorkspace={view.reorderWorkspace}
										renameProject={view.renameProject}
										searchLabel={view.searchLabel}
										searchShortcutId={view.searchShortcutId}
										searchShortcutLabel={view.searchShortcutLabel}
										settingsLabel={view.settingsLabel}
										settingsShortcutId={view.settingsShortcutId}
										settingsShortcutLabel={view.settingsShortcutLabel}
										terminalLabel={view.terminalLabel}
										setProjectPrimaryDirectory={view.setProjectPrimaryDirectory}
										toggleProjectExpansion={view.toggleProjectExpansion}
										updateWorkspace={view.updateWorkspace}
									/>
									<div
										aria-hidden='true'
										className={styles['windowDragRegion']}
										data-slot='workbench-window-drag-region'
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
								onCloseTabs={view.onCloseTabs}
								getCloseTabsConfirmation={view.getCloseTabsConfirmation}
								onSelectTab={navigation?.selectTab}
								parentActionScope={view.actionScope}
								reserveSidebarToggleSpace={!view.isSidebarVisible}
								tabModel={view.terminalContext.tabModel}
								tabStripMessages={tabStripMessages}
								verticalCollapsed={view.verticalTabStripCollapsed}
							/>
							<div className={styles['tabContent']} data-slot='workbench-tab-content'>
								<WorkbenchPanelWorkspace
									actionRuntime={actionRuntime}
									panelWorkspaceStore={view.terminalContext.panelWorkspaceStore}
									parentActionScope={view.actionScope}
									shortcutLabelForAction={shortcutLabelForAction}
									terminalSurface={terminalSurface}
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
					shortcutLabelForAction={shortcutLabelForAction}
					state={view.searchState}
					update={view.updateSearch}
				/>
			)}
			{projectWorkspaceConfirmationSurface}
		</div>
	);
}
