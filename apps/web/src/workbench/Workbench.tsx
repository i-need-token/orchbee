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
import { WorkbenchAgentBoardDialog } from './components/workbench-agent-board-dialog/WorkbenchAgentBoardDialog';
import { WorkbenchSidebarToggle } from './components/workbench-sidebar-toggle/WorkbenchSidebarToggle';
import { WorkbenchEmptyWorkspace } from './components/workbench-empty-workspace/WorkbenchEmptyWorkspace';
import { WorkbenchSidebar } from './components/workbench-sidebar/WorkbenchSidebar';
import { WorkbenchPanelWorkspace } from './components/workbench-panel-workspace/WorkbenchPanelWorkspace';
import { WorkbenchSearchSurface } from './components/workbench-search-surface/WorkbenchSearchSurface';
import { WorkbenchTabBar } from './components/workbench-tab-bar/WorkbenchTabBar';
import { NameDialog } from './components/name-dialog/NameDialog';
import { WORKBENCH_ACTION_IDS } from './actions/workbench-action-ids';
import { EMPTY_WORKBENCH_SEARCH_PROVIDERS } from './constants/search-providers';
import { useWorkbench } from './hooks/useWorkbench';
import { useWorkbenchAgentPresentation } from './hooks/useWorkbenchAgentPresentation';
import { useWorkbenchActionShortcutLabel } from './hooks/useWorkbenchActionShortcutLabel';
import { useWorkbenchTabStripMessages } from './hooks/useWorkbenchTabStripMessages';
import type { WorkbenchAuthority } from './types/workbench-authority';
import type { WorkbenchTerminalActionShortcutLabels } from './types/workbench-terminal-action-shortcut-labels';
import type {
	WorkbenchHostActionCallback,
	WorkbenchNavigationPort,
	WorkbenchTerminalFontSizeChangeCallback,
} from './types/workbench-host-actions';
import type { WorkbenchShortcutPlatform } from './types/workbench-shortcut-platform';
import type { WorkbenchSearchProvider } from './types/workbench-search-provider';
import type { WorkbenchTerminalSurface } from './types/workbench-terminal-surface';
import type { WorkbenchAgentNotificationObserver } from './types/workbench-agent-notification';
import type { WorkbenchAgentSource } from './types/workbench-agent-observation';
import type { WorkbenchWorkspaceLocationSource } from './types/workbench-workspace-location';
import type { WorkbenchProjectFolderAcquisition } from './types/workbench-project-folder-acquisition';
import type { WorkbenchProjectWorkspaceRuntime } from './model/WorkbenchProjectWorkspaceRuntime';
import {
	defaultWorkbenchShortcutBindings,
	type WorkbenchShortcutBindings,
} from './utils/workbench-shortcuts';
import styles from './Workbench.module.css';

interface WorkbenchProps {
	readonly actionRuntime: ActionRuntime;
	readonly agentNotificationObserver?: WorkbenchAgentNotificationObserver | undefined;
	readonly agentObservationSource?: WorkbenchAgentSource | undefined;
	/** Decides the shared Workbench structure. */
	readonly authority: WorkbenchAuthority;
	readonly createCrossWindowDragRuntime?: TabStripCrossWindowDragRuntimeFactory | undefined;
	readonly isShortcutScopeActive?: boolean | undefined;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly onSettings?: WorkbenchHostActionCallback | undefined;
	readonly onTerminalFontSizeChange?: WorkbenchTerminalFontSizeChangeCallback | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly projectFolderAcquisition?: WorkbenchProjectFolderAcquisition | undefined;
	readonly searchProviders?: readonly WorkbenchSearchProvider[] | undefined;
	readonly shortcutBindings?: WorkbenchShortcutBindings | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalActionShortcutLabels?: WorkbenchTerminalActionShortcutLabels | undefined;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
	/** Where each Workspace stands; without it the sidebar names Workspaces alone. */
	readonly workspaceLocationSource?: WorkbenchWorkspaceLocationSource | undefined;
}

export function Workbench({
	actionRuntime,
	agentNotificationObserver,
	agentObservationSource,
	authority,
	createCrossWindowDragRuntime,
	isShortcutScopeActive = true,
	layoutStorage,
	navigation,
	onSettings,
	onTerminalFontSizeChange,
	projectWorkspaceRuntime,
	projectFolderAcquisition,
	searchProviders = EMPTY_WORKBENCH_SEARCH_PROVIDERS,
	shortcutBindings,
	shortcutPlatform,
	terminalActionShortcutLabels,
	terminalSurface,
	workspaceLocationSource,
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
		agentObservationSource,
		authority,
		isShortcutScopeActive,
		navigation,
		onSettings,
		onTerminalFontSizeChange,
		projectWorkspaceRuntime,
		shortcutBindings: effectiveShortcutBindings,
		shortcutPlatform,
		storage: layoutStorage,
		terminalSurfaceAvailable: terminalSurface !== undefined,
	});
	const agentPresentation = useWorkbenchAgentPresentation({
		navigateAgent: view.navigateAgent,
		notificationObserver: agentNotificationObserver,
		panelWorkspaceStore: view.terminalContext.panelWorkspaceStore,
		projectWorkspaceRuntime,
		source: agentObservationSource,
		root: view.shortcutScopeReference,
		tabModel: view.tabModel,
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
	const projectWorkspaceConfirmationSurface =
		view.projectWorkspaceConfirmation === null ? null : (
			<AlertDialog
				open
				onOpenChange={(open) => {
					if (!open) view.cancelProjectWorkspaceConfirmation();
				}}
			>
				<AlertDialogContent
					className={styles['confirmationContent']}
					initialFocus={view.projectWorkspaceConfirmationCancelButtonRef}
				>
					<AlertDialogHeader className={styles['confirmationHeader']}>
						<AlertDialogTitle className={styles['confirmationTitle']}>
							{view.projectWorkspaceConfirmation.title}
						</AlertDialogTitle>
						<AlertDialogDescription className={styles['confirmationDescription']}>
							{view.projectWorkspaceConfirmation.description}
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter className={styles['confirmationFooter']}>
						<AlertDialogCancel
							ref={view.projectWorkspaceConfirmationCancelButtonRef}
							onClick={view.cancelProjectWorkspaceConfirmation}
						>
							{view.projectWorkspaceConfirmation.cancelLabel}
							<Kbd aria-hidden='true' className={styles['confirmationShortcut']}>
								Esc
							</Kbd>
						</AlertDialogCancel>
						<AlertDialogAction variant='destructive' onClick={view.confirmProjectWorkspaceAction}>
							{view.projectWorkspaceConfirmation.confirmLabel}
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		);
	const tabTitleEditorSurface =
		view.tabTitleEditor === null ? null : (
			<NameDialog
				blank={view.tabTitleEditorMessages.blank}
				finalFocus={view.tabTitleEditorFinalFocus}
				messages={view.tabTitleEditorMessages}
				name={view.tabTitleEditor.draftTitle}
				onClose={view.cancelTabTitleEditor}
				onNameChange={view.updateTabTitleEditor}
				onSubmit={view.submitTabTitleEditor}
				placeholder={view.tabTitleEditorMessages.placeholder}
				submitLabel={view.tabTitleEditorMessages.submit}
				title={view.tabTitleEditorMessages.title}
			/>
		);

	return (
		<div
			className={styles['root']}
			onBlurCapture={view.onRootBlurCapture}
			onFocusCapture={view.onRootFocusCapture}
			ref={view.shortcutScopeReference}
		>
			<div aria-live='polite' className={styles['liveRegion']}>
				{view.agentNavigationAnnouncement === null ? null : (
					<span key={view.agentNavigationAnnouncement.sequence}>
						{view.agentNavigationAnnouncement.message}
					</span>
				)}
			</div>
			<div className={styles['statusNotice']} data-slot='workbench-status-notice' role='status'>
				{view.authorityNotice === null ? null : (
					<span
						className={styles['statusNoticeText']}
						data-persistent={view.authorityNotice.persistent || undefined}
					>
						{view.authorityNotice.message}
					</span>
				)}
			</div>
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
								<div
									className={styles['leftPanel']}
									data-slot='workbench-left-panel'
									data-workbench-focus-region='sidebar'
								>
									<WorkbenchSidebar
										subscribeNavigation={projectWorkspaceRuntime.subscribeNavigation}
										activateWorkspace={view.activateWorkspace}
										createProject={view.createProject}
										activateCreatedProject={view.activateCreatedProject}
										isCreatedProjectActive={view.isCreatedProjectActive}
										createWorkspace={view.createWorkspace}
										setWorkspaceProject={view.setWorkspaceProject}
										createWorkspaceProject={view.createWorkspaceProject}
										deleteWorkspace={view.deleteWorkspace}
										navigateAgent={view.navigateAgent}
										label={view.sidebarLabel}
										onSearch={view.search}
										onSettings={view.settings}
										projectLabel={view.projectLabel}
										newWorkspaceLabel={view.newWorkspaceLabel}
										projectNavigationMessages={view.projectNavigationMessages}
										projectFolderAcquisition={projectFolderAcquisition}
										projectWorkspaceState={view.projectWorkspaceState}
										workspaceAgentOverviews={agentPresentation.workspaceAgentOverviews}
										removeProject={view.removeProject}
										reorderProject={view.reorderProject}
										reorderWorkspace={view.reorderWorkspace}
										renameProject={view.renameProject}
										searchLabel={view.searchLabel}
										searchShortcutId={view.searchShortcutId}
										searchShortcutLabel={view.searchShortcutLabel}
										focusRequest={view.sidebarFocusRequest}
										settingsLabel={view.settingsLabel}
										settingsShortcutId={view.settingsShortcutId}
										settingsShortcutLabel={view.settingsShortcutLabel}
										toggleProjectExpansion={view.toggleProjectExpansion}
										toggleRecentsExpansion={view.toggleRecentsExpansion}
										toggleWorkspaceAgents={agentPresentation.toggleWorkspaceAgents}
										updateWorkspace={view.updateWorkspace}
										workspaceLocationSource={workspaceLocationSource}
										worktreeCommands={view.worktreeCommands}
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
								closeResume={view.closeResume}
								createCrossWindowDragRuntime={createCrossWindowDragRuntime}
								newTabChoices={view.newTabChoices}
								onNewTerminal={view.newTab}
								onOrientationChange={view.onTabStripOrientationChange}
								onVerticalCollapsedChange={view.onVerticalTabStripCollapsedChange}
								orientation={view.tabStripOrientation}
								onCloseTabs={view.onCloseTabs}
								getCloseTabsConfirmation={view.getCloseTabsConfirmation}
								focusRequest={view.tabStripFocusRequest}
								getTabCreationAvailability={view.getTabCreationAvailability}
								onSelectTab={navigation?.selectTab}
								onSetTabTitle={view.setTabTitle}
								onTabsChange={view.onTabsChange}
								parentActionScope={view.actionScope}
								reserveSidebarToggleSpace={!view.isSidebarVisible}
								tabModel={view.tabModel}
								statusAdornments={agentPresentation.tabStatusAdornments}
								tabIdentityIcons={agentPresentation.tabIdentityIcons}
								tabStripMessages={tabStripMessages}
								verticalCollapsed={view.verticalTabStripCollapsed}
							/>
							<div
								className={styles['tabContent']}
								data-slot='workbench-tab-content'
								data-workbench-focus-region='terminal'
							>
								{view.hasVisibleTerminalTabs ? (
									<WorkbenchPanelWorkspace
										actionRuntime={actionRuntime}
										authority={view.authority}
										createPanelTerminal={view.createPanelTerminal}
										panelWorkspaceStore={view.terminalContext.panelWorkspaceStore}
										parentActionScope={view.actionScope}
										shortcutLabelForAction={shortcutLabelForAction}
										terminalAutoFocusAllowed={view.terminalAutoFocusAllowed}
										terminalContextId={view.terminalContext.terminalContextId}
										terminalFocusRequest={view.terminalFocusRequest}
										terminalSurface={terminalSurface}
										onTerminalEdit={agentPresentation.terminalEdited}
									/>
								) : (
									<div className={styles['emptyWindow']} data-slot='workbench-empty-window'>
										{view.emptyWorkspace === null ? null : (
											<WorkbenchEmptyWorkspace
												fact={view.emptyWorkspace.fact}
												newTabLabel={view.emptyWorkspace.newTabLabel}
												newTabShortcutLabel={shortcutLabelForAction(
													WORKBENCH_ACTION_IDS.createTerminal,
												)}
											/>
										)}
									</div>
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
			{view.agentBoard === undefined ? null : (
				<WorkbenchAgentBoardDialog
					board={view.agentBoard}
					finalFocus={view.agentBoardFinalFocus}
					onClose={view.closeAgentBoard}
					onShown={view.showAgentBoardSession}
					projectWorkspaceRuntime={projectWorkspaceRuntime}
				/>
			)}
			{projectWorkspaceConfirmationSurface}
			{tabTitleEditorSurface}
		</div>
	);
}
