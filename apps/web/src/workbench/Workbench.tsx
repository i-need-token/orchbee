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
import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ActionRuntime } from '@orchbee/actions';
import * as React from 'react';
import type { CSSProperties } from 'react';

import {
	MAXIMUM_LEFT_PANEL_WIDTH,
	MINIMUM_LEFT_PANEL_WIDTH,
	type WorkbenchLayoutStorage,
} from '../workbench-layout-storage';
import { WorkbenchAgentBoardDialog } from './components/workbench-agent-board-dialog/WorkbenchAgentBoardDialog';
import { WorkbenchSidebarToggle } from './components/workbench-sidebar-toggle/WorkbenchSidebarToggle';
import { WorkbenchEmptyWorkspace } from './components/workbench-empty-workspace/WorkbenchEmptyWorkspace';
import { WorkbenchSidebar } from './components/workbench-sidebar/WorkbenchSidebar';
import { WorkbenchProjectCreation } from './components/workbench-project-creation/WorkbenchProjectCreation';
import { WorkbenchPanelWorkspace } from './components/workbench-panel-workspace/WorkbenchPanelWorkspace';
import { WorkbenchSearchSurface } from './components/workbench-search-surface/WorkbenchSearchSurface';
import { WorkbenchTabBar } from './components/workbench-tab-bar/WorkbenchTabBar';
import { WorkbenchMobileBar } from './components/workbench-mobile-bar/WorkbenchMobileBar';
import { WorkbenchTabList } from './components/workbench-tab-list/WorkbenchTabList';
import { NameDialog } from './components/name-dialog/NameDialog';
import { WORKBENCH_ACTION_IDS } from './actions/workbench-action-ids';
import { EMPTY_WORKBENCH_SEARCH_PROVIDERS } from './constants/search-providers';
import { useWorkbench } from './hooks/useWorkbench';
import { useWorkbenchAgentPresentation } from './hooks/useWorkbenchAgentPresentation';
import { useWorkbenchActionShortcutLabel } from './hooks/useWorkbenchActionShortcutLabel';
import type { WorkbenchConfiguredCommandKeysView } from './hooks/useWorkbenchConfiguredCommandKeys';
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
import type {
	WorkbenchAgentNameSource,
	WorkbenchAgentSource,
} from './types/workbench-agent-observation';
import type { WorkbenchWorkspaceLocationSource } from './types/workbench-workspace-location';
import type { WorkbenchProjectFolderAcquisition } from './types/workbench-project-folder-acquisition';
import type { WorkbenchProjectWorkspaceRuntime } from './model/WorkbenchProjectWorkspaceRuntime';
import {
	defaultWorkbenchShortcutBindings,
	type WorkbenchShortcutBindings,
} from './utils/workbench-shortcuts';
import styles from './Workbench.module.css';

const NO_CONFIGURED_COMMAND_KEYS: WorkbenchConfiguredCommandKeysView = {
	catalog: [],
	keys: { chords: new Map(), heldLabels: new Set(), notes: new Map(), taken: new Map() },
	takenBy: new Map(),
};

interface WorkbenchProps {
	readonly actionRuntime: ActionRuntime;
	/** The names the Server reports for Agents, which name Agent rows. */
	readonly agentNameSource?: WorkbenchAgentNameSource | undefined;
	readonly agentNotificationObserver?: WorkbenchAgentNotificationObserver | undefined;
	readonly agentObservationSource?: WorkbenchAgentSource | undefined;
	/** Decides the shared Workbench structure. */
	readonly authority: WorkbenchAuthority;
	/**
	 * What `useWorkbenchConfiguredCommandKeys` follows for the authority's configured commands, so the
	 * shortcut settings read the same result; without it the window offers no configured command.
	 */
	readonly configuredCommandKeys?: WorkbenchConfiguredCommandKeysView | undefined;
	readonly isShortcutScopeActive?: boolean | undefined;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly onSettings?: WorkbenchHostActionCallback | undefined;
	readonly onShortcuts?: WorkbenchHostActionCallback | undefined;
	readonly onTerminalFontSizeChange?: WorkbenchTerminalFontSizeChangeCallback | undefined;
	readonly preferencesUnreadable?: boolean | undefined;
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
	agentNameSource,
	agentNotificationObserver,
	agentObservationSource,
	authority,
	configuredCommandKeys = NO_CONFIGURED_COMMAND_KEYS,
	isShortcutScopeActive = true,
	layoutStorage,
	navigation,
	onSettings,
	onShortcuts,
	onTerminalFontSizeChange,
	preferencesUnreadable = false,
	projectWorkspaceRuntime,
	projectFolderAcquisition,
	searchProviders = EMPTY_WORKBENCH_SEARCH_PROVIDERS,
	shortcutBindings,
	shortcutPlatform,
	terminalActionShortcutLabels,
	terminalSurface,
	workspaceLocationSource,
}: WorkbenchProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const tabStripMessages = useWorkbenchTabStripMessages();
	const effectiveShortcutBindings =
		shortcutBindings ?? defaultWorkbenchShortcutBindings(shortcutPlatform);
	const shortcutLabelForAction = useWorkbenchActionShortcutLabel({
		configuredCommandKeys: configuredCommandKeys.keys,
		shortcutBindings: effectiveShortcutBindings,
		shortcutPlatform,
		terminalActionShortcutLabels,
	});
	const view = useWorkbench({
		actionRuntime,
		agentObservationSource,
		authority,
		configuredCommandCatalog: configuredCommandKeys.catalog,
		configuredCommandKeys: configuredCommandKeys.keys,
		isShortcutScopeActive,
		navigation,
		onSettings,
		onShortcuts,
		onTerminalFontSizeChange,
		projectFolderAcquisition,
		projectWorkspaceRuntime,
		shortcutBindings: effectiveShortcutBindings,
		shortcutPlatform,
		storage: layoutStorage,
		terminalSurfaceAvailable: terminalSurface !== undefined,
	});
	const terminalContext = view.terminalContext;
	const agentPresentation = useWorkbenchAgentPresentation({
		nameSource: agentNameSource,
		navigateAgent: view.navigateAgent,
		notificationObserver: agentNotificationObserver,
		projectWorkspaceRuntime,
		source: agentObservationSource,
		root: view.shortcutScopeReference,
		terminalContext,
	});
	const sidebarToggle = (
		<WorkbenchSidebarToggle
			isSidebarVisible={view.isSidebarVisible}
			label={view.sidebarToggleLabel}
			onToggle={view.toggleSidebar}
			shortcutId={view.sidebarShortcutId}
			shortcutLabel={view.sidebarShortcutLabel}
			touch={view.touchSizes}
		/>
	);
	// One sidebar, docked in its resizable panel, opened over the content in a narrow window, or
	// filling the window in the mobile layout.
	const sidebarPanel = (
		<div
			className={styles['leftPanel']}
			data-slot='workbench-left-panel'
			data-workbench-focus-region='sidebar'
		>
			<WorkbenchSidebar
				subscribeNavigation={projectWorkspaceRuntime.subscribeNavigation}
				activateWorkspace={view.activateWorkspace}
				createWorkspace={view.createWorkspace}
				setWorkspaceProject={view.setWorkspaceProject}
				createWorkspaceProject={view.createWorkspaceProject}
				deleteWorkspace={view.deleteWorkspace}
				navigateAgent={view.navigateAgent}
				closeAgentPanel={view.closeAgentPanel}
				renameAgentPanel={view.renameAgentPanel}
				label={view.sidebarLabel}
				onSearch={view.search}
				onSettings={view.settings}
				onShortcuts={view.shortcuts}
				reorderModifier={shortcutPlatform === 'apple' ? 'meta' : 'ctrl'}
				resetSidebarWidth={view.resetSidebarWidth}
				resizeSidebar={view.resizeSidebar}
				returnToTerminal={view.returnToTerminal}
				shortcutsLabel={view.shortcutsLabel}
				shortcutsShortcutId={view.shortcutsShortcutId}
				shortcutsShortcutLabel={view.shortcutsShortcutLabel}
				projectLabel={view.projectLabel}
				newWorkspaceLabel={view.newWorkspaceLabel}
				newWorkspaceShortcutId={view.newWorkspaceShortcutId}
				newWorkspaceShortcutLabel={view.newWorkspaceShortcutLabel}
				projectNavigationMessages={view.projectNavigationMessages}
				projectWorkspaceState={view.projectWorkspaceState}
				workspaceAgentOverviews={agentPresentation.workspaceAgentOverviews}
				removeProject={view.removeProject}
				reorderProject={view.reorderProject}
				moveWorkspace={view.moveWorkspace}
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
					initialFocus={view.projectWorkspaceConfirmationFocusRef}
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
						{/* A close confirmation focuses Close, so Enter closes as in the TUI. */}
						<AlertDialogCancel
							ref={
								view.projectWorkspaceConfirmation.defaultAnswer === 'cancel'
									? view.projectWorkspaceConfirmationFocusRef
									: undefined
							}
							onClick={view.cancelProjectWorkspaceConfirmation}
						>
							{view.projectWorkspaceConfirmation.cancelLabel}
							<Kbd aria-hidden='true' className={styles['confirmationShortcut']}>
								Esc
							</Kbd>
						</AlertDialogCancel>
						<AlertDialogAction
							ref={
								view.projectWorkspaceConfirmation.defaultAnswer === 'confirm'
									? view.projectWorkspaceConfirmationFocusRef
									: undefined
							}
							variant='destructive'
							onClick={view.confirmProjectWorkspaceAction}
						>
							{view.projectWorkspaceConfirmation.confirmLabel}
							{view.projectWorkspaceConfirmation.defaultAnswer === 'confirm' ? (
								<Kbd aria-hidden='true' className={styles['confirmationShortcut']}>
									Enter
								</Kbd>
							) : null}
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		);
	const nameEditorSurface =
		view.nameEditor === null ? null : (
			<NameDialog
				blank={view.nameEditorMessages.blank}
				finalFocus={view.nameEditorFinalFocus}
				messages={view.nameEditorMessages}
				name={view.nameEditor.draftTitle}
				onClose={view.cancelNameEditor}
				onNameChange={view.updateNameEditor}
				onSubmit={view.submitNameEditor}
				placeholder={view.nameEditorMessages.placeholder}
				submitLabel={view.nameEditorMessages.submit}
				title={view.nameEditorMessages.title}
			/>
		);

	const tabListShown = view.tabListOpen && terminalContext !== null;
	// No Workspace has no Tab Strip, so its row keeps the toggle's place and the window's drag. The
	// mobile layout's bar takes the strip's place.
	let tabChrome: React.ReactNode;
	if (terminalContext === null) {
		tabChrome = (
			<div
				aria-hidden='true'
				className={styles['noWorkspaceChrome']}
				data-slot='workbench-window-drag-region'
			/>
		);
	} else if (view.mobileLayout) {
		tabChrome = (
			<WorkbenchMobileBar
				covered={tabListShown || view.sidebarPresentation === 'fill'}
				identityIcons={agentPresentation.tabIdentityIcons}
				onOpenTabList={view.toggleTabList}
				statusAdornments={agentPresentation.tabStatusAdornments}
				tabListOpen={view.tabListOpen}
				tabModel={terminalContext.tabModel}
				touch={view.touchSizes}
				tabStripMessages={tabStripMessages}
			/>
		);
	} else {
		tabChrome = (
			<WorkbenchTabBar
				actionRuntime={actionRuntime}
				closeResume={view.closeResume}
				newTabChoices={view.newTabChoices}
				onNewTerminal={view.newTab}
				onOrientationChange={view.onTabStripOrientationChange}
				onVerticalCollapsedChange={view.onVerticalTabStripCollapsedChange}
				orientation={view.tabStripOrientation}
				onCloseTabs={view.onCloseTabs}
				focusRequest={view.tabStripFocusRequest}
				getTabCreationAvailability={view.getTabCreationAvailability}
				onSelectTab={navigation?.selectTab}
				onSetTabTitle={view.setTabTitle}
				onTabsChange={view.onTabsChange}
				parentActionScope={view.actionScope}
				reserveSidebarToggleSpace={view.sidebarPresentation !== 'docked'}
				tabModel={terminalContext.tabModel}
				statusAdornments={agentPresentation.tabStatusAdornments}
				tabIdentityIcons={agentPresentation.tabIdentityIcons}
				tabStripMessages={tabStripMessages}
				verticalCollapsed={view.verticalTabStripCollapsed}
			/>
		);
	}

	return (
		<div
			className={styles['root']}
			data-workbench-touch={view.touchSizes || undefined}
			onBlurCapture={view.onRootBlurCapture}
			onFocusCapture={view.onRootFocusCapture}
			ref={view.shortcutScopeReference}
		>
			<div className={styles['statusNotice']} data-slot='workbench-status-notice' role='status'>
				{preferencesUnreadable ? (
					<button
						className={styles['statusNoticeText']}
						data-persistent
						onClick={view.settings}
						type='button'
					>
						{intl.formatMessage(terminalSettingsMessages.sharedUnreadable)}
					</button>
				) : null}
				{view.authorityNotice === null ? null : (
					<span
						className={styles['statusNoticeText']}
						data-persistent={view.authorityNotice.persistent || undefined}
					>
						{view.authorityNotice.message}
					</span>
				)}
			</div>
			{view.sidebarPresentation === 'overlay' ? (
				<>
					{/* Nothing dims: the scrim only takes the click that closes the overlay. */}
					<div
						aria-hidden='true'
						className={styles['sidebarScrim']}
						data-slot='workbench-sidebar-scrim'
						onPointerDown={view.dismissSidebarOverlay}
					/>
					<div
						className={styles['sidebarOverlay']}
						data-slot='workbench-sidebar-overlay'
						onKeyDown={view.onSidebarOverlayKeyDown}
						style={
							{
								'--ob-comp-workbench-sidebar-overlay-inline-size': `${view.sidebarWidth}px`,
							} satisfies CSSProperties
						}
					>
						{sidebarPanel}
					</div>
				</>
			) : null}
			{view.sidebarPresentation === 'fill' ? (
				<div
					className={styles['sidebarFill']}
					data-slot='workbench-sidebar-fill'
					onKeyDown={view.onSidebarOverlayKeyDown}
				>
					{sidebarPanel}
				</div>
			) : null}
			{/* The Tab list covers the sidebar button, as it covers the bar. */}
			{tabListShown ? null : (
				<div className={styles['sidebarToggle']} data-slot='workbench-sidebar-toggle-position'>
					{sidebarToggle}
				</div>
			)}
			{/* The list's strip stays mounted through the mobile layout, hidden and inert while the list
			    is closed, because the Tab Actions and their shortcuts resolve through it. */}
			{view.mobileLayout && terminalContext !== null ? (
				<WorkbenchTabList
					actionRuntime={actionRuntime}
					closeResume={view.closeResume}
					focusRequest={view.tabStripFocusRequest}
					getTabCreationAvailability={view.getTabCreationAvailability}
					identityIcons={agentPresentation.tabIdentityIcons}
					newTabChoices={view.newTabChoices}
					newTabShortcutLabel={shortcutLabelForAction(WORKBENCH_ACTION_IDS.createTerminal)}
					onClose={view.dismissTabList}
					onCloseTabs={view.onCloseTabs}
					onKeyDown={view.onTabListKeyDown}
					onNewTab={view.newTab}
					onSelectTab={navigation?.selectTab}
					onSetTabTitle={view.setTabTitle}
					onTabsChange={view.onTabsChange}
					open={tabListShown}
					parentActionScope={view.actionScope}
					statusAdornments={agentPresentation.tabStatusAdornments}
					tabModel={terminalContext.tabModel}
					tabStripMessages={tabStripMessages}
					touch={view.touchSizes}
					workspaceName={view.activeWorkspaceName}
				/>
			) : null}
			<div className={styles['layout']}>
				<ResizablePanelGroup
					disableCursor
					onLayoutChanged={view.onLayoutChanged}
					orientation='horizontal'
				>
					{view.sidebarPresentation === 'docked' && (
						<>
							{/* Keep the navigation panel on an explicit pixel contract. */}
							<ResizablePanel
								id='left'
								defaultSize={view.leftPanelSize}
								minSize={`${MINIMUM_LEFT_PANEL_WIDTH}px`}
								maxSize={`${MAXIMUM_LEFT_PANEL_WIDTH}px`}
								groupResizeBehavior='preserve-pixel-size'
								onResize={view.onLeftPanelResize}
								panelRef={view.leftPanelReference}
							>
								{sidebarPanel}
							</ResizablePanel>
							<ResizableHandle
								className={styles['resizeHandle']}
								onDoubleClick={view.resetSidebarWidth}
								onKeyDown={view.onSidebarDividerKeyDown}
							/>
						</>
					)}
					<ResizablePanel id='right'>
						<div
							className={styles['rightPanel']}
							data-slot='workbench-right-panel'
							data-tab-strip-orientation={
								terminalContext === null || view.mobileLayout ? undefined : view.tabStripOrientation
							}
						>
							{tabChrome}
							<div
								className={styles['tabContent']}
								data-slot='workbench-tab-content'
								data-workbench-focus-region='terminal'
							>
								{terminalContext !== null && view.hasVisibleTerminalTabs ? (
									<WorkbenchPanelWorkspace
										actionRuntime={actionRuntime}
										authority={view.authority}
										createPanelTerminal={view.createPanelTerminal}
										panelWorkspaceStore={terminalContext.panelWorkspaceStore}
										parentActionScope={view.actionScope}
										presentationAgentStatuses={agentPresentation.presentationAgentStatuses}
										shortcutLabelForAction={shortcutLabelForAction}
										terminalAutoFocusAllowed={view.terminalAutoFocusAllowed}
										terminalContextId={terminalContext.terminalContextId}
										terminalFocusRequest={view.terminalFocusRequest}
										terminalSurface={terminalSurface}
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
					argumentPickerForAction={view.searchArgumentPickerForAction}
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
			{nameEditorSurface}
			<WorkbenchProjectCreation
				messages={view.projectCreationMessages}
				view={view.projectCreation}
			/>
		</div>
	);
}
