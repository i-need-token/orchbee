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
import { useMemoizedFn } from '@orchbee/foundation/react';
import { keyboardShortcutMessages, terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import * as React from 'react';
import type { CSSProperties } from 'react';

import type { FileBarTerminals } from '../file-bar';
import { FileSessionRegistry, type FileSessionPort } from '../file-session';
import type { UserSettingsControl } from '../user-settings';
import {
	MAXIMUM_LEFT_PANEL_WIDTH,
	MINIMUM_LEFT_PANEL_WIDTH,
	type WorkbenchLayoutStorage,
} from '../workbench-layout-storage';
import { WorkbenchKeyboardScopeContext } from '../workbench-keyboard-scopes';
import { WorkbenchAgentConfirmationOffer } from './components/workbench-agent-confirmation-offer/WorkbenchAgentConfirmationOffer';
import { WorkbenchAgentStartDialog } from './components/workbench-agent-start-dialog/WorkbenchAgentStartDialog';
import { WorkbenchColumnToggle } from './components/workbench-column-toggle/WorkbenchColumnToggle';
import { WorkbenchEmptyWorkspace } from './components/workbench-empty-workspace/WorkbenchEmptyWorkspace';
import { WorkbenchMachineCard } from './components/workbench-machine-card/WorkbenchMachineCard';
import { WorkbenchSidebar } from './components/workbench-sidebar/WorkbenchSidebar';
import type { WorkbenchOtherMachines } from './types/workbench-other-machines';
import { WorkbenchProjectCreation } from './components/workbench-project-creation/WorkbenchProjectCreation';
import { WorkbenchPanelWorkspace } from './components/workbench-panel-workspace/WorkbenchPanelWorkspace';
import { WorkbenchSearchSurface } from './components/workbench-search-surface/WorkbenchSearchSurface';
import { WorkbenchQuickInputSurface } from './components/workbench-search-surface/components/workbench-quick-input-surface/WorkbenchQuickInputSurface';
import { WorkbenchNotificationCards } from './components/workbench-notification-cards/WorkbenchNotificationCards';
import { useWorkbenchNotificationQueue } from './components/workbench-notification-cards/hooks/useWorkbenchNotificationQueue';
import { useWorkbenchQuickInputSessions } from './components/workbench-search-surface/hooks/useWorkbenchQuickInputSessions';
import { WorkbenchTabBar } from './components/workbench-tab-bar/WorkbenchTabBar';
import { WorkbenchMobileBar } from './components/workbench-mobile-bar/WorkbenchMobileBar';
import { WorkbenchTabList } from './components/workbench-tab-list/WorkbenchTabList';
import { NameDialog } from './components/name-dialog/NameDialog';
import { WorkbenchWorktreeFormDialog } from './components/workbench-worktree-form-dialog/WorkbenchWorktreeFormDialog';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchAgentConfirmationChooseInput,
} from './actions/workbench-action-ids';
import { EMPTY_WORKBENCH_SEARCH_PROVIDERS } from './constants/search-providers';
import { useWorkbench } from './hooks/useWorkbench';
import { useWorkbenchAgentPresentation } from './hooks/useWorkbenchAgentPresentation';
import { useWorkbenchActionShortcutLabel } from './hooks/useWorkbenchActionShortcutLabel';
import { useWorkbenchFileBar } from './hooks/useWorkbenchFileBar';
import { useWorkbenchFileOverlay } from './hooks/useWorkbenchFileOverlay';
import { useWorkbenchTerminalFileLinks } from './hooks/useWorkbenchTerminalFileLinks';
import type { WorkbenchConfiguredCommandKeysView } from './hooks/useWorkbenchConfiguredCommandKeys';
import { useWorkbenchTabStripMessages } from './hooks/useWorkbenchTabStripMessages';
import type { WorkbenchAuthority } from './types/workbench-authority';
import type { WorkbenchClientUpdate } from './types/workbench-client-update';
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
import { WorkbenchTerminalPathDrops } from './model/WorkbenchTerminalPathDrops';
import { workbenchEnvironmentMarkerView } from './model/workbenchEnvironmentMarkerView';
import { WORKBENCH_RESIZE_TARGET_SIZE_PX } from './model/workbench-resize-target';
import type { WorkbenchEnvironmentMarker } from './types/workbench-environment-marker';
import { resolveActiveTerminalPresentationId } from './utils/resolveActiveTerminalPresentationId';
import {
	defaultWorkbenchScopedShortcutBindings,
	defaultWorkbenchShortcutBindings,
	type WorkbenchScopedShortcutBinding,
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
	/** A newer version of this window its host can switch to; absent where the host offers none. */
	readonly clientUpdate?: WorkbenchClientUpdate | undefined;
	/**
	 * What `useWorkbenchConfiguredCommandKeys` follows for the authority's configured commands, so the
	 * shortcut settings read the same result; without it the window offers no configured command.
	 */
	readonly configuredCommandKeys?: WorkbenchConfiguredCommandKeysView | undefined;
	/** The build the sidebar marks beside Settings; a release has none. */
	readonly environmentMarker?: WorkbenchEnvironmentMarker | undefined;
	/** Whether the window offers the File Bar, which is withheld unless offered. */
	readonly fileBarOffered?: boolean | undefined;
	/** Where the File Bar's file sessions come from; without it the File Bar opens none. */
	readonly fileSessionPort?: FileSessionPort | undefined;
	/**
	 * The host's Action scope: the Workbench's Actions resolve under it, and Update registers in it,
	 * so the host's own surfaces, such as Settings' About, run the footer's Action.
	 */
	readonly hostActionScope?: ActionScope | undefined;
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
	/** Whether the window offers Search in the Workbench. Search defaults to withheld. */
	readonly searchOffered?: boolean | undefined;
	/**
	 * The keys of the File Bar family's shortcuts. A window that offers the File Bar defaults to their
	 * platform defaults; one that does not has none, whatever this says.
	 */
	readonly scopedShortcutBindings?: readonly WorkbenchScopedShortcutBinding[] | undefined;
	readonly shortcutBindings?: WorkbenchShortcutBindings | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	/** Suppresses the window-level Update failure toast while another surface presents it. */
	readonly suppressClientUpdateFailureNotice?: boolean | undefined;
	readonly terminalActionShortcutLabels?: WorkbenchTerminalActionShortcutLabels | undefined;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
	/** The user's settings, which the File Bar's views read and answer into; without them they keep defaults. */
	readonly userSettings?: UserSettingsControl | undefined;
	/** Where each Workspace stands; without it the sidebar names Workspaces alone. */
	readonly workspaceLocationSource?: WorkbenchWorkspaceLocationSource | undefined;
	/** Other machines' Workspaces, which the sidebar lists where the window holds several Sessions. */
	readonly otherMachines?: WorkbenchOtherMachines | undefined;
	/** Change machine's Manage machines…: Settings on Machines. */
	readonly onManageMachines?: (() => void) | undefined;
}

export function Workbench({
	actionRuntime,
	agentNameSource,
	agentNotificationObserver,
	agentObservationSource,
	authority,
	clientUpdate,
	configuredCommandKeys = NO_CONFIGURED_COMMAND_KEYS,
	environmentMarker,
	fileBarOffered = false,
	fileSessionPort,
	hostActionScope,
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
	searchOffered = false,
	scopedShortcutBindings,
	shortcutBindings,
	shortcutPlatform,
	suppressClientUpdateFailureNotice = false,
	terminalActionShortcutLabels,
	terminalSurface,
	userSettings,
	workspaceLocationSource,
	otherMachines,
	onManageMachines,
}: WorkbenchProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const tabStripMessages = useWorkbenchTabStripMessages();
	const effectiveShortcutBindings =
		shortcutBindings ?? defaultWorkbenchShortcutBindings(shortcutPlatform);
	const quickInputSessions = useWorkbenchQuickInputSessions();
	const notificationQueue = useWorkbenchNotificationQueue();
	const effectiveScopedShortcutBindings = fileBarOffered
		? (scopedShortcutBindings ?? defaultWorkbenchScopedShortcutBindings(shortcutPlatform))
		: undefined;
	const shortcutLabelForAction = useWorkbenchActionShortcutLabel({
		configuredCommandKeys: configuredCommandKeys.keys,
		scopedShortcutBindings: effectiveScopedShortcutBindings,
		shortcutBindings: effectiveShortcutBindings,
		shortcutPlatform,
		terminalActionShortcutLabels,
	});
	const hostReplacementFocusRef = React.useRef<HTMLButtonElement | null>(null);
	const view = useWorkbench({
		actionRuntime,
		agentObservationSource,
		authority,
		clientUpdate,
		configuredCommandCatalog: configuredCommandKeys.catalog,
		configuredCommandKeys: configuredCommandKeys.keys,
		hostActionScope,
		isShortcutScopeActive,
		navigation,
		onSettings,
		onShortcuts,
		onTerminalFontSizeChange,
		projectFolderAcquisition,
		projectWorkspaceRuntime,
		searchOffered,
		scopedShortcutBindings: effectiveScopedShortcutBindings,
		shortcutBindings: effectiveShortcutBindings,
		shortcutPlatform,
		storage: layoutStorage,
		suppressClientUpdateFailureNotice,
		terminalSurfaceAvailable: terminalSurface !== undefined,
	});
	const terminalContext = view.terminalContext;
	const tabListShown = view.tabListOpen && terminalContext !== null;
	// Only the File Bar's views drag files, so a window that does not offer it takes no such drops.
	const terminalPathDrops = React.useMemo(
		() =>
			fileBarOffered
				? new WorkbenchTerminalPathDrops(actionRuntime, view.reportActionOutcome)
				: undefined,
		[actionRuntime, fileBarOffered, view.reportActionOutcome],
	);
	const activeWorkspaceId =
		view.projectWorkspaceState.activeLocation.kind === 'workspace'
			? view.projectWorkspaceState.activeLocation.workspaceId
			: null;
	// The Client's file sessions, one per Workspace, which the File Bar and terminal file links share.
	const fileSessions = React.useMemo(
		() => (fileSessionPort === undefined ? null : new FileSessionRegistry(fileSessionPort)),
		[fileSessionPort],
	);
	// The File Bar's row controls first take the full tier of the overlay below off the File Bar; the
	// overlay needs the File Bar's side, so it is built after the File Bar.
	const uncoverFileBarViews = useMemoizedFn(() => fileOverlay.uncoverFileBar());
	const fileBar = useWorkbenchFileBar({
		actionRuntime,
		actionScope: view.actionScope,
		closeSurfacesOverContent: view.closeSurfacesOverContent,
		focusTerminal: view.returnToTerminal,
		mobile: view.mobileLayout,
		offered: fileBarOffered,
		reportActionOutcome: view.reportActionOutcome,
		rootReference: view.shortcutScopeReference,
		sessions: fileSessions,
		shortcutLabelForAction,
		shortcutPlatform,
		storage: layoutStorage,
		supplyFocusRegion: view.supplyFocusRegion,
		surfacesOverContent:
			tabListShown || view.sidebarPresentation === 'overlay' || view.sidebarPresentation === 'fill',
		uncoverViews: uncoverFileBarViews,
		userSettings,
		workspaceId: activeWorkspaceId,
	});
	// The scrim covers the terminal a press on it gives the keys to, so files dropped there go to it.
	const currentTerminal = useMemoizedFn(() =>
		resolveActiveTerminalPresentationId(projectWorkspaceRuntime),
	);
	// The Explorer's terminal commands: Insert Path into Terminal types into the current terminal as a
	// drop on the scrim does, and Open in Integrated Terminal opens a Tab as New Tab does.
	const fileBarTerminals = React.useMemo<FileBarTerminals>(
		() => ({
			insertPaths: (context, paths) =>
				terminalPathDrops?.insertIntoCurrentTerminal(currentTerminal, paths, context.source) ??
				Promise.resolve({ code: 'terminal-unavailable', kind: 'blocked' }),
			insertPathsAvailability: (paths) =>
				terminalPathDrops?.currentTerminalAvailability(currentTerminal, paths) ?? {
					kind: 'hidden',
				},
			openTab: view.openTerminalTab,
		}),
		[currentTerminal, terminalPathDrops, view.openTerminalTab],
	);
	const fileOverlay = useWorkbenchFileOverlay({
		actionRuntime,
		actionScope: view.actionScope,
		currentTerminal,
		fileBarDocked: fileBar.column !== null,
		fileBarPosition: fileBar.position,
		fileOpened: fileBar.fileOpened,
		focusTerminal: view.returnToTerminal,
		notifications: notificationQueue,
		offered: fileBarOffered,
		quickInputSessions,
		reportActionOutcome: view.reportActionOutcome,
		rootReference: view.shortcutScopeReference,
		scopedShortcutBindings: effectiveScopedShortcutBindings,
		shortcutLabelForAction,
		shortcutPlatform,
		sidebarDocked: view.sidebarPresentation === 'docked',
		storage: layoutStorage,
		supplyFocusRegion: view.supplyFocusRegion,
		terminalContext,
		terminalPathDrops,
		workspaceId: activeWorkspaceId,
		workspaces: view.projectWorkspaceState.workspaces,
	});
	const openTerminalFile = useWorkbenchTerminalFileLinks({
		actionScope: view.actionScope,
		offered: fileBarOffered,
		openFile: fileOverlay.openFile,
		sessions: fileSessions,
		workspaceId: activeWorkspaceId,
	});
	const fileBarContent = fileBar.column ?? fileBar.overlay;
	const FileBarColumn = fileBarContent?.content ?? null;
	const fileBarBadge = fileBar.toggle?.badge ?? null;
	const FileBarToggleBadge = fileBarBadge?.Badge ?? null;
	const fileBarBadgeId = React.useId();
	const FileOverlayHost = fileOverlay.host?.content ?? null;
	// The column that touches the window's start holds the Sidebar button's place in its top row: the
	// File Bar's toggles row while it docks on the left of the terminal area and the Sidebar does not.
	const fileBarHoldsSidebarTogglePlace =
		fileBar.column !== null && fileBar.position === 'left' && view.sidebarPresentation !== 'docked';
	// The File Bar's button stays at the window's end, over the toggles row only while the File Bar
	// docks there; otherwise the Tab bar keeps its place, and the window's controls, at its end.
	const tabBarAtWindowEnd = fileBar.column === null || fileBar.position === 'left';
	const agentPresentation = useWorkbenchAgentPresentation({
		confirmations: view.authority.agentConfirmations,
		nameSource: agentNameSource,
		navigateAgent: view.navigateAgent,
		notificationObserver: agentNotificationObserver,
		projectWorkspaceRuntime,
		source: agentObservationSource,
		root: view.shortcutScopeReference,
		terminalContext,
	});
	const confirmationOffer = agentPresentation.confirmationOffer;
	const environmentMarkerView = React.useMemo(
		() =>
			environmentMarker === undefined
				? undefined
				: workbenchEnvironmentMarkerView(environmentMarker, intl),
		[environmentMarker, intl],
	);
	const chooseAgentConfirmationFromMenu = useMemoizedFn(
		(input: WorkbenchAgentConfirmationChooseInput) => view.chooseAgentConfirmation(input, 'menu'),
	);
	const sidebarToggle = (
		<WorkbenchColumnToggle
			column='sidebar'
			label={view.sidebarToggleLabel}
			onToggle={view.toggleSidebar}
			pressed={view.isSidebarVisible}
			shortcutId={view.sidebarShortcutId}
			shortcutLabel={view.sidebarShortcutLabel}
			touch={view.touchSizes}
		/>
	);
	// The sidebar button floats over the top start of whichever chrome lies beneath it: the sidebar's
	// top row, the horizontal Tab bar, the mobile bar, or the no-Workspace row. Each keeps its place.
	const sidebarTogglePlace = (
		<span className={styles['sidebarTogglePlace']} data-sidebar-toggle-place='' />
	);
	// One sidebar, docked in its resizable panel, opened over the content in a narrow window, or
	// filling the window in the mobile layout.
	const sidebarPanel = (
		<div
			className={styles['leftPanel']}
			data-slot='workbench-left-panel'
			data-workbench-focus-region='sidebar'
			ref={fileOverlay.sidebarReference}
		>
			<WorkbenchSidebar
				subscribeNavigation={projectWorkspaceRuntime.subscribeNavigation}
				activateWorkspace={view.activateWorkspace}
				createWorkspace={view.createWorkspace}
				setWorkspaceProject={view.setWorkspaceProject}
				createWorkspaceProject={view.createWorkspaceProject}
				deleteWorkspace={view.deleteWorkspace}
				environmentMarker={environmentMarkerView}
				navigateAgent={view.navigateAgent}
				acknowledgeAgentCompletion={view.authority.acknowledgeAgentCompletion}
				chooseAgentConfirmation={chooseAgentConfirmationFromMenu}
				closeAgentPanel={view.closeAgentPanel}
				renameAgentPanel={view.renameAgentPanel}
				label={view.sidebarLabel}
				onSearch={view.search}
				onSettings={view.settings}
				onShortcuts={view.shortcuts}
				otherMachines={otherMachines}
				onManageMachines={onManageMachines}
				reorderModifier={shortcutPlatform === 'apple' ? 'meta' : 'ctrl'}
				returnToTerminal={view.returnToTerminal}
				projectLabel={view.projectLabel}
				newWorkspaceLabel={view.newWorkspaceLabel}
				newWorkspaceShortcutId={view.newWorkspaceShortcutId}
				newWorkspaceShortcutLabel={view.newWorkspaceShortcutLabel}
				projectNavigationMessages={view.projectNavigationMessages}
				projectWorkspaceState={view.projectWorkspaceState}
				workspaceAgentOverviews={agentPresentation.workspaceAgentOverviews}
				workspaceAgentLaunch={view.workspaceAgentLaunch}
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
				update={view.update}
				updateWorkspace={view.updateWorkspace}
				workspaceLocationSource={workspaceLocationSource}
				worktreeCommands={view.worktreeCommands}
			/>
			<div
				aria-hidden='true'
				className={styles['windowDragRegion']}
				data-slot='workbench-window-drag-region'
			>
				{sidebarTogglePlace}
			</div>
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
	const hostReplacementSurface =
		view.hostReplacement === null ? null : (
			<AlertDialog
				open
				onOpenChange={(open) => {
					if (!open) view.hostReplacement?.cancel();
				}}
			>
				<AlertDialogContent
					className={styles['confirmationContent']}
					initialFocus={hostReplacementFocusRef}
				>
					<AlertDialogHeader className={styles['confirmationHeader']}>
						<AlertDialogTitle className={styles['confirmationTitle']}>
							{view.hostReplacement.title}
						</AlertDialogTitle>
						<AlertDialogDescription className={styles['confirmationDescription']}>
							{view.hostReplacement.description}
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter className={styles['confirmationFooter']}>
						<AlertDialogCancel onClick={view.hostReplacement.cancel}>
							{view.hostReplacement.cancelLabel}
							<Kbd aria-hidden='true' className={styles['confirmationShortcut']}>
								Esc
							</Kbd>
						</AlertDialogCancel>
						{/* Restart holds the focus, as in the TUI. */}
						<AlertDialogAction
							ref={hostReplacementFocusRef}
							variant='destructive'
							onClick={view.hostReplacement.confirm}
						>
							{view.hostReplacement.confirmLabel}
							<Kbd aria-hidden='true' className={styles['confirmationShortcut']}>
								Enter
							</Kbd>
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

	// The surfaces over the layout come before it in the document, which keeps them first in the
	// focus order; the layout marks itself covered while one of them is shown.
	const layoutCovered =
		tabListShown ||
		view.sidebarPresentation === 'overlay' ||
		view.sidebarPresentation === 'fill' ||
		fileBar.overlay !== null;
	// No Workspace has no Tab Strip, so its row keeps the toggle's place and the window's drag. The
	// mobile layout's bar takes the strip's place.
	let tabChrome: React.ReactNode;
	if (terminalContext === null) {
		tabChrome = (
			<div
				aria-hidden='true'
				className={styles['noWorkspaceChrome']}
				data-slot='workbench-window-drag-region'
			>
				{view.sidebarPresentation === 'docked' || fileBarHoldsSidebarTogglePlace
					? null
					: sidebarTogglePlace}
			</div>
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
				atWindowEnd={tabBarAtWindowEnd}
				newTabSections={view.newTabSections}
				onNewTerminal={view.newTab}
				onCloseTabs={view.onCloseTabs}
				focusRequest={view.tabStripFocusRequest}
				getTabCreationAvailability={view.getTabCreationAvailability}
				onSelectTab={view.selectTab}
				onSetTabTitle={view.setTabTitle}
				onTabsChange={view.onTabsChange}
				parentActionScope={view.actionScope}
				reserveFileBarToggleSpace={fileBar.toggle !== null && tabBarAtWindowEnd}
				reserveSidebarToggleSpace={
					view.sidebarPresentation !== 'docked' && !fileBarHoldsSidebarTogglePlace
				}
				tabModel={terminalContext.tabModel}
				statusAdornments={agentPresentation.tabStatusAdornments}
				tabIdentityIcons={agentPresentation.tabIdentityIcons}
				tabStripMessages={tabStripMessages}
			/>
		);
	}

	// The File Bar's landmark, docked in its panel or opened over the terminal area. Over the terminal
	// area its row holds the places of the buttons floating over the window's ends it reaches, while the
	// chrome beneath keeps them too, so nothing beneath moves as it opens.
	const fileBarLandmark =
		fileBarContent === null ? null : (
			<aside
				aria-label={fileBarContent.label}
				className={styles['fileBarPanel']}
				data-slot='file-bar'
				data-workbench-focus-region='file-bar'
				ref={fileBarContent.attachElement}
			>
				{FileBarColumn === null ? null : (
					<FileBarColumn
						actionRuntime={actionRuntime}
						actionScope={view.actionScope}
						activeFile={fileOverlay.activeFile}
						activeView={fileBarContent.activeView}
						atWindowEnd={
							fileBar.overlay === null ? fileBar.position === 'right' : fileBar.overlay.atWindowEnd
						}
						leadingPlace={
							(
								fileBar.overlay === null
									? fileBarHoldsSidebarTogglePlace
									: fileBar.overlay.atWindowStart
							)
								? sidebarTogglePlace
								: null
						}
						notifications={notificationQueue}
						onSelectView={fileBarContent.onSelectView}
						openFile={fileOverlay.openFile}
						platform={shortcutPlatform}
						quickInput={quickInputSessions}
						reportActionOutcome={view.reportActionOutcome}
						returnToTerminal={view.returnToTerminal}
						rowMenu={fileBarContent.rowMenu}
						session={fileBarContent.session}
						settings={userSettings}
						shortcutLabelForAction={shortcutLabelForAction}
						storage={layoutStorage}
						terminals={fileBarTerminals}
						toggles={fileBarContent.toggles}
						viewFocusRequest={fileBarContent.viewFocusRequest}
					/>
				)}
				{fileBarContent.failure === null ? null : (
					<p className={styles['fileBarFailure']}>{fileBarContent.failure}</p>
				)}
			</aside>
		);
	const fileBarColumn = fileBar.column;
	const fileBarHandle =
		fileBarColumn === null ? null : (
			<ResizableHandle
				className={styles['resizeHandle']}
				onDoubleClick={fileBarColumn.onDividerDoubleClick}
				onKeyDown={fileBarColumn.onDividerKeyDown}
			/>
		);
	const fileBarPanel =
		fileBarColumn === null ? null : (
			<ResizablePanel
				id={fileBarColumn.id}
				defaultSize={fileBarColumn.defaultSize}
				minSize={fileBarColumn.minSize}
				maxSize={fileBarColumn.maxSize}
				groupResizeBehavior='preserve-pixel-size'
				onResize={fileBarColumn.onResize}
				panelRef={fileBarColumn.panelReference}
			>
				{fileBarLandmark}
			</ResizablePanel>
		);
	const fileBarOverlay = fileBar.overlay;

	const sequenceNotice = view.shortcutSequenceNotice;
	const statusNotice = (
		<div className={styles['statusNotice']} data-slot='workbench-status-notice' role='status'>
			{sequenceNotice === null ? null : (
				<span className={styles['statusNoticeText']}>
					{sequenceNotice.kind === 'waiting'
						? intl.formatMessage(keyboardShortcutMessages.sequenceWaiting, {
								leader: sequenceNotice.leader,
							})
						: intl.formatMessage(keyboardShortcutMessages.sequenceMissed, {
								key: sequenceNotice.step,
								leader: sequenceNotice.leader,
							})}
				</span>
			)}
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
			{/* An offer never covers what just happened: it shows once the line is free. */}
			{confirmationOffer === null ||
			preferencesUnreadable ||
			sequenceNotice !== null ||
			view.authorityNotice !== null ? null : (
				<WorkbenchAgentConfirmationOffer
					agentLabel={confirmationOffer.agentLabel}
					onAnswer={(answer) =>
						view.chooseAgentConfirmation(
							{ answer, presentationId: confirmationOffer.presentationId },
							'button',
						)
					}
				/>
			)}
		</div>
	);
	const workbench = (
		<div
			className={styles['root']}
			data-workbench-touch={view.touchSizes || undefined}
			onBlurCapture={view.onRootBlurCapture}
			onFocusCapture={view.onRootFocusCapture}
			ref={view.shortcutScopeReference}
		>
			{fileBarOffered ? (
				<div className={styles['noticeColumn']}>
					<WorkbenchNotificationCards queue={notificationQueue} />
					{statusNotice}
				</div>
			) : (
				statusNotice
			)}
			{view.sidebarPresentation === 'overlay' ? (
				<>
					{/* Nothing dims: the scrim only takes the click that closes the overlay. */}
					<div
						aria-hidden='true'
						className={styles['sidebarScrim']}
						data-slot='workbench-sidebar-scrim'
						onPointerDown={view.onSidebarScrimPointerDown}
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
			{fileBarOverlay === null ? null : (
				<>
					{/* Nothing dims: the scrim only takes the press that closes the File Bar. */}
					<div
						aria-hidden='true'
						className={styles['fileBarScrim']}
						data-slot='workbench-file-bar-scrim'
						onPointerDown={fileBarOverlay.onScrimPointerDown}
					/>
					<div
						className={styles['fileBarOverlay']}
						data-side={fileBar.position}
						data-slot='workbench-file-bar-overlay'
						onKeyDown={fileBarOverlay.onKeyDown}
						style={
							{
								'--ob-comp-workbench-file-bar-overlay-inline-size': `${fileBarOverlay.width}px`,
								'--ob-comp-workbench-file-bar-overlay-inline-start': `${fileBarOverlay.inlineStart}px`,
							} satisfies CSSProperties
						}
					>
						{fileBarLandmark}
					</div>
				</>
			)}
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
					focusRequest={view.tabStripFocusRequest}
					getTabCreationAvailability={view.getTabCreationAvailability}
					identityIcons={agentPresentation.tabIdentityIcons}
					newTabSections={view.newTabSections}
					newTabShortcutLabel={shortcutLabelForAction(WORKBENCH_ACTION_IDS.createTerminal)}
					onClose={view.dismissTabList}
					onCloseTabs={view.onCloseTabs}
					onKeyDown={view.onTabListKeyDown}
					onNewTab={view.newTab}
					onSelectTab={view.selectTab}
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
			<div
				className={styles['layout']}
				data-covered={layoutCovered || undefined}
				data-slot='workbench-layout'
			>
				<ResizablePanelGroup
					disableCursor
					onLayoutChanged={view.onLayoutChanged}
					orientation='horizontal'
					resizeTargetMinimumSize={WORKBENCH_RESIZE_TARGET_SIZE_PX}
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
					{/* The terminal area and the File Bar share their own group, so resizing the Sidebar or the
					    window leaves the File Bar's width as it was, whichever side it takes. */}
					<ResizablePanel id='workbench-main'>
						<ResizablePanelGroup
							disableCursor
							elementRef={fileBar.areaReference}
							onLayoutChanged={fileBar.onLayoutChanged}
							orientation='horizontal'
							resizeTargetMinimumSize={WORKBENCH_RESIZE_TARGET_SIZE_PX}
						>
							{fileBarPanel === null || fileBar.position !== 'left' ? null : (
								<>
									{fileBarPanel}
									{fileBarHandle}
								</>
							)}
							<ResizablePanel id='terminal-area' minSize={fileBarColumn?.terminalAreaMinSize}>
								<div className={styles['rightPanel']} data-slot='workbench-right-panel'>
									{tabChrome}
									<div
										className={styles['tabContent']}
										data-slot='workbench-tab-content'
										data-workbench-focus-region='terminal'
										ref={fileOverlay.terminalAreaReference}
									>
										{terminalContext !== null && view.hasVisibleTerminalTabs ? (
											<WorkbenchPanelWorkspace
												actionRuntime={actionRuntime}
												authority={view.authority}
												createPanelTerminal={view.createPanelTerminal}
												openTerminalFile={openTerminalFile}
												panelWorkspaceStore={terminalContext.panelWorkspaceStore}
												parentActionScope={view.actionScope}
												presentationAgentStatuses={agentPresentation.presentationAgentStatuses}
												reportActionOutcome={view.reportActionOutcome}
												shortcutLabelForAction={shortcutLabelForAction}
												terminalAutoFocusAllowed={view.terminalAutoFocusAllowed}
												terminalContextId={terminalContext.terminalContextId}
												terminalFocusRequest={view.terminalFocusRequest}
												terminalPathDrops={terminalPathDrops}
												terminalSurface={terminalSurface}
											/>
										) : (
											<div
												className={styles['emptyWindow']}
												data-slot='workbench-empty-window'
												onDoubleClick={
													terminalContext === null ? undefined : view.onEmptyContentDoubleClick
												}
											>
												{view.emptyWorkspace === null ? null : (
													<WorkbenchEmptyWorkspace
														fact={view.emptyWorkspace.fact}
														newTabLabel={view.emptyWorkspace.newTabLabel}
														newTabShortcutLabel={shortcutLabelForAction(
															WORKBENCH_ACTION_IDS.createTerminal,
														)}
														startAgentLabel={view.emptyWorkspace.startAgentLabel}
														startAgentShortcutLabel={shortcutLabelForAction(
															WORKBENCH_ACTION_IDS.openAgentStart,
														)}
													/>
												)}
											</div>
										)}
										{otherMachines === undefined ? null : (
											<WorkbenchMachineCard otherMachines={otherMachines} />
										)}
									</div>
								</div>
							</ResizablePanel>
							{fileBarPanel === null || fileBar.position !== 'right' ? null : (
								<>
									{fileBarHandle}
									{fileBarPanel}
								</>
							)}
						</ResizablePanelGroup>
					</ResizablePanel>
				</ResizablePanelGroup>
			</div>
			{/* The File Bar button floats over the top end of whichever chrome lies beneath it: the File
			    Bar's toggles row while it docks on the right or opens over the terminal area to the
			    window's end, else the Tab bar or the no-Workspace row. It follows the layout and the File
			    Bar over it, so its no-drag region comes after the toggles row's drag region, and it stays
			    over the scrim of the File Bar it closes. */}
			{fileBar.toggle === null ? null : (
				<div
					className={styles['fileBarToggle']}
					data-over-file-bar={fileBarOverlay === null ? undefined : ''}
					data-slot='workbench-file-bar-toggle-position'
				>
					<WorkbenchColumnToggle
						badge={
							fileBarBadge === null || FileBarToggleBadge === null ? undefined : (
								<FileBarToggleBadge descriptionId={fileBarBadgeId} session={fileBarBadge.session} />
							)
						}
						column='file-bar'
						descriptionId={fileBarBadge === null ? undefined : fileBarBadgeId}
						label={fileBar.toggle.label}
						onToggle={fileBar.toggle.onToggle}
						pressed={fileBar.toggle.shown}
						shortcutId={WORKBENCH_ACTION_IDS.toggleFileBar}
						shortcutLabel={shortcutLabelForAction(WORKBENCH_ACTION_IDS.toggleFileBar)}
						side={fileBar.position}
					/>
				</div>
			)}
			{/* The shown Workspace's file overlay lies over the layout, after the File Bar in the focus
			    order, and never takes part in the layout, so the terminal area keeps its size. */}
			{fileOverlay.host === null || FileOverlayHost === null ? null : (
				<div className={styles['fileOverlayRegion']} data-workbench-focus-region='file-overlay'>
					<FileOverlayHost
						commands={fileOverlay.host.commands}
						dispatch={fileOverlay.host.dispatch}
						focusTerminal={fileOverlay.host.focusTerminal}
						movePip={fileOverlay.host.movePip}
						pipPosition={fileOverlay.host.pipPosition}
						platform={fileOverlay.host.platform}
						ref={fileOverlay.host.reference}
						regions={fileOverlay.host.regions}
						returnFocus={fileOverlay.host.returnFocus}
						scrimDrop={fileOverlay.host.scrimDrop}
						shortcutLabels={fileOverlay.host.shortcutLabels}
						fileBarPosition={fileBar.position}
						sidebarDocked={view.sidebarPresentation === 'docked'}
						store={fileOverlay.host.store}
					/>
				</div>
			)}
			{view.searchState.kind === 'closed' ? null : (
				<WorkbenchSearchSurface
					actionRuntime={actionRuntime}
					actionScope={view.actionScope}
					argumentPickerForAction={view.searchArgumentPickerForAction}
					filters={view.searchFilters}
					providers={searchProviders}
					reportActionOutcome={view.reportActionOutcome}
					shortcutLabelForAction={shortcutLabelForAction}
					state={view.searchState}
					update={view.updateSearch}
				/>
			)}
			{fileBarOffered ? <WorkbenchQuickInputSurface sessions={quickInputSessions} /> : null}
			{view.agentStart === undefined ? null : (
				<WorkbenchAgentStartDialog
					catalog={view.agentStart.catalog}
					finalFocus={view.agentStart.finalFocus}
					onClose={view.closeAgentStart}
					onStart={view.startAgent}
					places={view.agentStart.places}
				/>
			)}
			{view.worktreeForm === undefined ? null : (
				<WorkbenchWorktreeFormDialog
					branch={view.worktreeForm.branch}
					finalFocus={view.worktreeForm.finalFocus}
					key={view.worktreeForm.key}
					onCancel={view.worktreeForm.onCancel}
					onSubmit={view.worktreeForm.onSubmit}
				/>
			)}
			{projectWorkspaceConfirmationSurface}
			{hostReplacementSurface}
			{nameEditorSurface}
			<WorkbenchProjectCreation
				messages={view.projectCreationMessages}
				view={view.projectCreation}
			/>
		</div>
	);
	return (
		<WorkbenchKeyboardScopeContext.Provider value={view.keyboardScopeRegistry}>
			{workbench}
		</WorkbenchKeyboardScopeContext.Provider>
	);
}
