import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
	Kbd,
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
import { WorkbenchSidebar } from './components/workbench-sidebar/WorkbenchSidebar';
import { WorkbenchPanelWorkspace } from './components/workbench-panel-workspace/WorkbenchPanelWorkspace';
import { WorkbenchSearchSurface } from './components/workbench-search-surface/WorkbenchSearchSurface';
import { WorkbenchTabBar } from './components/workbench-tab-bar/WorkbenchTabBar';
import { WorkbenchTabTitleDialog } from './components/workbench-tab-title-dialog/WorkbenchTabTitleDialog';
import { WORKBENCH_ACTION_IDS } from './actions/workbench-action-ids';
import { EMPTY_WORKBENCH_SEARCH_PROVIDERS } from './constants/search-providers';
import { useWorkbench } from './hooks/useWorkbench';
import { useWorkbenchActionShortcutLabel } from './hooks/useWorkbenchActionShortcutLabel';
import { useWorkbenchTabStripMessages } from './hooks/useWorkbenchTabStripMessages';
import { useWorkbenchTabWindowControl } from './hooks/useWorkbenchTabWindowControl';
import type { WorkbenchTerminalActionShortcutLabels } from './types/workbench-terminal-action-shortcut-labels';
import type {
	WorkbenchHostActionCallback,
	WorkbenchNavigationPort,
	WorkbenchTerminalFontSizeChangeCallback,
} from './types/workbench-host-actions';
import type { WorkbenchShortcutPlatform } from './types/workbench-shortcut-platform';
import type { WorkbenchSearchProvider } from './types/workbench-search-provider';
import type { WorkbenchTerminalSurface } from './types/workbench-terminal-surface';
import type { WorkbenchProjectFolderAcquisition } from './types/workbench-project-folder-acquisition';
import type { WorkbenchProjectWorktreeCapability } from './types/workbench-project-worktree';
import type { WorkbenchTabScopeSource } from './types/workbench-tab-scope';
import type { WorkbenchTabWindowControl } from './types/workbench-tab-window-control';
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
	readonly onTerminalFontSizeChange?: WorkbenchTerminalFontSizeChangeCallback | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly projectFolderAcquisition?: WorkbenchProjectFolderAcquisition | undefined;
	readonly projectWorktree?: WorkbenchProjectWorktreeCapability | undefined;
	readonly searchProviders?: readonly WorkbenchSearchProvider[] | undefined;
	readonly shortcutBindings?: WorkbenchShortcutBindings | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalActionShortcutLabels?: WorkbenchTerminalActionShortcutLabels | undefined;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
	readonly tabScope?: WorkbenchTabScopeSource | undefined;
	readonly tabWindowControl?: WorkbenchTabWindowControl | undefined;
}

export function Workbench({
	actionRuntime,
	createCrossWindowDragRuntime,
	isShortcutScopeActive = true,
	layoutStorage,
	navigation,
	onSettings,
	onTerminalFontSizeChange,
	projectWorkspaceRuntime,
	projectFolderAcquisition,
	projectWorktree,
	searchProviders = EMPTY_WORKBENCH_SEARCH_PROVIDERS,
	shortcutBindings,
	shortcutPlatform,
	terminalActionShortcutLabels,
	terminalSurface,
	tabScope,
	tabWindowControl,
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
		onTerminalFontSizeChange,
		projectWorkspaceRuntime,
		shortcutBindings: effectiveShortcutBindings,
		shortcutPlatform,
		storage: layoutStorage,
		tabScope,
	});
	const windowIntegration = useWorkbenchTabWindowControl({
		control: tabWindowControl,
		formatWindowLabel: tabStripMessages.formatWindowLabel,
		model: view.tabModel,
		terminalContextId: view.terminalContext.terminalContextId,
	});
	const effectiveCrossWindowDragRuntime =
		createCrossWindowDragRuntime ?? windowIntegration.createCrossWindowDragRuntime;
	const sidebarToggle = (
		<WorkbenchSidebarToggle
			isSidebarVisible={view.isSidebarVisible}
			label={view.sidebarToggleLabel}
			onToggle={view.toggleSidebar}
			shortcutId={view.sidebarShortcutId}
			shortcutLabel={view.sidebarShortcutLabel}
		/>
	);
	const projectWorkspaceConfirmationSurface =
		view.projectWorkspaceConfirmation === null ? null : (
			<AlertDialog
				open
				onOpenChange={(open) => {
					if (!open) view.cancelProjectWorkspaceConfirmation();
				}}
			>
				<AlertDialogContent initialFocus={view.projectWorkspaceConfirmationConfirmButtonRef}>
					<AlertDialogHeader>
						<AlertDialogTitle>{view.projectWorkspaceConfirmation.title}</AlertDialogTitle>
						<AlertDialogDescription>
							{view.projectWorkspaceConfirmation.description}
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel onClick={view.cancelProjectWorkspaceConfirmation}>
							{view.projectWorkspaceConfirmation.cancelLabel}
							<Kbd aria-hidden='true' className={styles['confirmationShortcut']}>
								Esc
							</Kbd>
						</AlertDialogCancel>
						<AlertDialogAction
							ref={view.projectWorkspaceConfirmationConfirmButtonRef}
							variant='destructive'
							onClick={view.confirmProjectWorkspaceAction}
						>
							{view.projectWorkspaceConfirmation.confirmLabel}
							<Kbd aria-hidden='true' className={styles['confirmationShortcut']}>
								Enter
							</Kbd>
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		);
	const tabTitleEditorSurface =
		view.tabTitleEditor === null ? null : (
			<WorkbenchTabTitleDialog
				finalFocus={view.tabTitleEditorFinalFocus}
				messages={view.tabTitleEditorMessages}
				onCancel={view.cancelTabTitleEditor}
				onSubmit={view.submitTabTitleEditor}
				onTitleChange={view.updateTabTitleEditor}
				title={view.tabTitleEditor.draftTitle}
			/>
		);

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
										projectWorktree={projectWorktree}
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
								createCrossWindowDragRuntime={effectiveCrossWindowDragRuntime}
								onMoveGroupToWindow={windowIntegration.moveGroupToWindow}
								onMoveTabsToWindow={windowIntegration.moveTabsToWindow}
								onNewTerminal={view.newTab}
								onOrientationChange={view.onTabStripOrientationChange}
								onVerticalCollapsedChange={view.onVerticalTabStripCollapsedChange}
								orientation={view.tabStripOrientation}
								onCloseTabs={view.onCloseTabs}
								getCloseTabsConfirmation={view.getCloseTabsConfirmation}
								onSelectTab={navigation?.selectTab}
								onSetTabTitle={view.setTabTitle}
								parentActionScope={view.actionScope}
								reserveSidebarToggleSpace={!view.isSidebarVisible}
								shortcutLabels={{
									closeTab: shortcutLabelForAction(WORKBENCH_ACTION_IDS.closeActiveTerminal),
									newTab: shortcutLabelForAction(WORKBENCH_ACTION_IDS.createTerminal),
									setTabTitle: shortcutLabelForAction(WORKBENCH_ACTION_IDS.setTabTitle),
								}}
								tabModel={view.tabModel}
								tabStripMessages={tabStripMessages}
								verticalCollapsed={view.verticalTabStripCollapsed}
								windowTargets={windowIntegration.windowTargets}
							/>
							<div className={styles['tabContent']} data-slot='workbench-tab-content'>
								{view.hasVisibleTerminalTabs ? (
									<WorkbenchPanelWorkspace
										actionRuntime={actionRuntime}
										panelWorkspaceStore={view.terminalContext.panelWorkspaceStore}
										parentActionScope={view.actionScope}
										shortcutLabelForAction={shortcutLabelForAction}
										terminalSurface={terminalSurface}
									/>
								) : (
									<div className={styles['emptyWindow']} data-slot='workbench-empty-window' />
								)}
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
			{tabTitleEditorSurface}
		</div>
	);
}
