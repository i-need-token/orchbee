import { useOrchbeeIntl, workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import type {
	ActionConfirmation,
	ActionAvailability,
	ActionExecution,
	ActionResult,
	ActionRuntime,
	ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import type { ResizablePanelHandle } from '@orchbee/ui/components';
import {
	type TabStripChangeCallback,
	type TabStripCloseCallback,
	type TabStripNewTabSection,
	type TabStripNewTabRequest,
	TabStripOrientation,
	type TabStripTabCreationIntent,
} from '@orchbee/tab-strip';
import * as React from 'react';
import type {
	UserLaunchProfile,
	WorkbenchAgentStartProvider,
} from '@orchbee/workbench-client-protocol';

import {
	DEFAULT_LEFT_PANEL_WIDTH,
	MAXIMUM_LEFT_PANEL_WIDTH,
	MINIMUM_LEFT_PANEL_WIDTH,
	readWorkbenchLeftPanelWidth,
	writeWorkbenchLeftPanelWidth,
	type WorkbenchLayoutStorage,
} from '../../workbench-layout-storage';
import type { ProjectWorkspaceState, WorkbenchActiveLocation } from '../../project-workspace';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import type { WorkbenchAgentSource } from '../types/workbench-agent-observation';
import type {
	WorkbenchHostActionCallback,
	WorkbenchNavigationPort,
	WorkbenchTerminalCreateCallback,
	WorkbenchTerminalFontSizeChangeCallback,
} from '../types/workbench-host-actions';
import { WORKBENCH_SEARCH_FILTERS, type WorkbenchSearchFilter } from '../types/workbench-search';
import type { WorkbenchSearchProviderArgumentPicker } from '../types/workbench-search-provider';
import type {
	WorkbenchProjectNavigationMessages,
	WorkbenchSidebarWorkspaceCreation,
	WorkbenchWorktreeCommands,
} from '../components/workbench-sidebar/types/workbench-sidebar-types';
import type {
	WorkbenchProjectCreationReceipt,
	WorkbenchProjectFolderAcquisition,
} from '../types/workbench-project-folder-acquisition';
import {
	useWorkbenchProjectCreation,
	type WorkbenchProjectCreationView,
} from '../components/workbench-project-creation/hooks/useWorkbenchProjectCreation';
import type { WorkbenchProjectCreationMessages } from '../components/workbench-project-creation/types/workbench-project-creation-messages';
import { createFocusedPanelWorktreePicker } from '../model/createFocusedPanelWorktreePicker';
import type { WorkbenchSidebarPresentation } from '../model/workbench-sidebar-presentation';
import { matchesWorkbenchProjectCreation } from '../model/matchesWorkbenchProjectCreation';
import {
	projectWorkbenchEmptyWorkspace,
	type WorkbenchEmptyWorkspace,
} from '../model/projectWorkbenchEmptyWorkspace';
import {
	createWorkbenchSearchState,
	transitionWorkbenchSearch,
	type WorkbenchSearchEvent,
	type WorkbenchSearchState,
} from '../components/workbench-search-surface/model/workbench-search-state';
import {
	formatWorkbenchShortcut,
	WORKBENCH_SHORTCUTS,
	type WorkbenchShortcutBindings,
} from '../utils/workbench-shortcuts';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchPanelTargetInput,
	type WorkbenchTerminalCreateInput,
} from '../actions/workbench-action-ids';
import type { WorkbenchTabTitleEditorRequest } from '../actions/registerWorkbenchTabTitleAction';
import type { WorkbenchPanelNameEditorRequest } from '../actions/registerWorkbenchPanelNameAction';
import { findPanel } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { NameDialogBlankRule } from '../components/name-dialog/NameDialog';
import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalContext,
} from '../model/WorkbenchProjectWorkspaceRuntime';
import { findActiveTerminalFocusTarget } from '../utils/findActiveTerminalFocusTarget';
import type { WorkbenchAgentStartInput } from '../types/workbench-agent-start';
import {
	useWorkbenchAgentDialogs,
	type WorkbenchAgentStartDialogModel,
} from './useWorkbenchAgentDialogs';
import { resolveActiveTerminalPresentationId } from '../utils/resolveActiveTerminalPresentationId';
import { subscribeToNothing } from '../utils/subscribeToNothing';
import { useWorkbenchMobileFocusRouter } from './useWorkbenchMobileFocusRouter';
import { useWorkbenchSidebarPresentation } from './useWorkbenchSidebarPresentation';
import {
	createSharedTab,
	createSharedTerminal,
	duplicateSharedTabs,
	getSharedTabCreationAvailability,
	getSharedTerminalCreationAvailability,
} from '../actions/workbench-shared-tab-creation';
import {
	createSharedPanelTerminal,
	type WorkbenchSharedPanelTerminalCreate,
} from '../actions/createSharedPanelTerminal';
import { closeReturningToTerminal } from '../actions/closeReturningToTerminal';
import type { WorkbenchAuthority } from '../types/workbench-authority';
import type { WorkbenchConfiguredCommand } from '../types/workbench-configured-commands';
import { workbenchConfiguredCommandActionId } from '../actions/workbench-configured-command-action-id';
import {
	findWorkbenchConfiguredCommandForKey,
	type WorkbenchConfiguredCommandKeys,
} from '../model/workbench-configured-command-keys';
import { useWorkbenchActions } from './useWorkbenchActions';
import {
	useWorkbenchWorktreeForm,
	type WorkbenchWorktreeFormModel,
} from './useWorkbenchWorktreeForm';
import {
	useWorkbenchAuthorityNotice,
	type WorkbenchAuthorityNoticeView,
} from './useWorkbenchAuthorityNotice';
import { useWorkbenchFocusRouting } from './useWorkbenchFocusRouting';
import { useWorkbenchNewTabChoices } from './useWorkbenchNewTabChoices';
import type { WorkbenchTerminalFocusRequest } from '../model/resolveWorkbenchFocusTarget';
import { useWorkbenchShortcuts } from './useWorkbenchShortcuts';
import { useWorkbenchAgentNavigation } from './useWorkbenchAgentNavigation';
import { useWorkbenchTerminalLocationActivation } from './useWorkbenchTerminalLocationActivation';
import type { WorkbenchAgentNavigationInput } from '../model/project-workbench-agent-overview';
import {
	terminalCloseWarningValues,
	type TerminalCloseImpact,
} from '../model/terminal-close-impact';
import {
	workbenchActiveLocationsEqual,
	type WorkbenchTerminalLocationActivationOrigin,
} from '../model/workbench-terminal-location-activation';

/** A window that withholds Search keeps the Actions filter, the TUI's action menu counterpart. */
const ACTION_SEARCH_FILTERS: readonly WorkbenchSearchFilter[] = ['actions'];

const ACTIVE_TAB_UNAVAILABLE = { code: 'active-tab-unavailable', kind: 'blocked' } as const;

/**
 * The close of those requested Tabs the context still holds, in its strip's order, which asks the
 * authority first.
 */
function sharedTabsClosure(activeContext: WorkbenchTerminalContext, tabIds: readonly string[]) {
	const requested = new Set(tabIds);
	return {
		tabIds: activeContext.tabModel
			.getSnapshot()
			.tabs.filter((tab) => requested.has(tab.id))
			.map((tab) => tab.id),
		terminalContextId: activeContext.terminalContextId,
		type: 'tabs-close-if-idle' as const,
	};
}

interface ResizableLayoutChangedMeta {
	readonly isUserInteraction: boolean;
}

interface ResizablePanelSize {
	readonly inPixels: number;
}

interface WorkbenchView {
	/** Present while Start Agent is open. */
	readonly agentStart: WorkbenchAgentStartDialogModel | undefined;
	/** Esc, Cancel or the backdrop: closes Start Agent. */
	readonly closeAgentStart: () => void;
	/** Closes Start Agent and runs `workbench.agent.start-provider`. */
	readonly startAgent: (input: WorkbenchAgentStartInput) => void;
	/** Present while the New Worktree form is open. */
	readonly worktreeForm: WorkbenchWorktreeFormModel | undefined;
	readonly authority: WorkbenchAuthorityNoticeView['authority'];
	readonly authorityNotice: WorkbenchAuthorityNoticeView['notice'];
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly closeAgentPanel: (input: WorkbenchPanelTargetInput) => void;
	readonly renameAgentPanel: (input: WorkbenchPanelTargetInput) => void;
	readonly actionScope: ActionScope;
	readonly cancelProjectWorkspaceConfirmation: () => void;
	readonly confirmProjectWorkspaceAction: () => void;
	/** Present when an authority decides the Panels, whose new terminals then show as it answers. */
	readonly createPanelTerminal: WorkbenchSharedPanelTerminalCreate;
	readonly createWorkspace: (input?: WorkbenchSidebarWorkspaceCreation) => void;
	readonly setWorkspaceProject: (workspaceId: string, projectId: string | null) => void;
	readonly createWorkspaceProject: (workspaceId: string, name: string) => void;
	readonly deleteWorkspace: (workspaceId: string) => void;
	readonly isSidebarVisible: boolean;
	/**
	 * Docked beside the content, hidden, open over the content in a narrow window, or filling the
	 * window in the mobile layout.
	 */
	readonly sidebarPresentation: WorkbenchSidebarPresentation;
	/** Below 481 px wide: the bar replaces the Tab strip and the lists fill the window. */
	readonly mobileLayout: boolean;
	/** Touch sizes, which the mobile layout takes from 408 px high. */
	readonly touchSizes: boolean;
	/** The mobile layout's Tab list, filling the window. */
	readonly tabListOpen: boolean;
	readonly toggleTabList: () => void;
	/** Closes the Tab list and gives the keyboard back to the terminal. */
	readonly dismissTabList: () => void;
	/** Esc inside the Tab list closes it; a menu's own Esc stays in its portal, a drag's in the drag. */
	readonly onTabListKeyDown: (event: React.KeyboardEvent<HTMLElement>) => void;
	/** The current Workspace's name, which heads the Tab list. */
	readonly activeWorkspaceName: string | null;
	/** Closes the sidebar opened over the content and gives the keyboard back to the terminal. */
	readonly dismissSidebarOverlay: () => void;
	/** Esc inside the overlay closes it; a menu's own Esc stays in its portal, a drag's in the drag. */
	readonly onSidebarOverlayKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
	/** The sidebar's width in pixels, which the overlay takes too. */
	readonly sidebarWidth: number;
	/** Shift-free ← and → on the focused divider change the width in 10 px steps. */
	readonly onSidebarDividerKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
	readonly leftPanelReference: React.RefObject<ResizablePanelHandle | null>;
	readonly leftPanelSize: string;
	readonly newTab: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly newTabSections: readonly TabStripNewTabSection[] | undefined;
	readonly onCloseTabs: TabStripCloseCallback;
	/** Present when an authority decides the Tabs, which then show only as it answers. */
	readonly onTabsChange: TabStripChangeCallback;
	readonly getTabCreationAvailability: (intent: TabStripTabCreationIntent) => ActionAvailability;
	readonly onLayoutChanged: (
		layout: Readonly<Record<string, number>>,
		meta: ResizableLayoutChangedMeta,
	) => void;
	readonly onLeftPanelResize: (size: ResizablePanelSize) => void;
	/** Returns the sidebar to its default width. */
	readonly resetSidebarWidth: () => void;
	/** Gives keyboard focus back to the current terminal. */
	readonly returnToTerminal: () => void;
	readonly onRootBlurCapture: (event: React.FocusEvent<HTMLDivElement>) => void;
	readonly onRootFocusCapture: (event: React.FocusEvent<HTMLDivElement>) => void;
	readonly shortcutScopeReference: React.RefObject<HTMLDivElement | null>;
	readonly sidebarShortcutId: string;
	/** Absent while a configured command holds the chord. */
	readonly sidebarShortcutLabel: string | undefined;
	readonly sidebarToggleLabel: string;
	readonly sidebarLabel: string;
	readonly projectLabel: string;
	readonly newWorkspaceLabel: string;
	readonly newWorkspaceShortcutId: string;
	readonly newWorkspaceShortcutLabel: string | undefined;
	readonly projectNavigationMessages: WorkbenchProjectNavigationMessages;
	readonly projectCreation: WorkbenchProjectCreationView;
	readonly projectCreationMessages: WorkbenchProjectCreationMessages;
	/** The button that holds the initial focus: Close in a close confirmation, Cancel otherwise. */
	readonly projectWorkspaceConfirmationFocusRef: React.RefObject<HTMLButtonElement | null>;
	readonly projectWorkspaceConfirmation: {
		readonly cancelLabel: string;
		readonly confirmLabel: string;
		readonly defaultAnswer: 'cancel' | 'confirm';
		readonly description: string;
		readonly title: string;
	} | null;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly removeProject: (projectId: string) => void;
	readonly reorderProject: (projectId: string, beforeProjectId: string | null) => Promise<unknown>;
	readonly moveWorkspace: (
		workspaceId: string,
		projectId: string | null,
		beforeWorkspaceId: string | null,
	) => Promise<unknown>;
	readonly renameProject: (projectId: string, name: string) => void;
	readonly searchLabel: string;
	/** Absent when the window withholds Search. */
	readonly search: ((returnFocus: HTMLElement) => void) | undefined;
	/** The filters Search offers; a window that withholds Search keeps only Actions. */
	readonly searchFilters: readonly WorkbenchSearchFilter[];
	readonly searchState: WorkbenchSearchState;
	/** Pickers for Search's Actions whose choices the Workbench reads when the picker opens. */
	readonly searchArgumentPickerForAction: (
		actionId: string,
	) => WorkbenchSearchProviderArgumentPicker | undefined;
	readonly updateSearch: React.Dispatch<WorkbenchSearchEvent>;
	readonly searchShortcutId: string;
	readonly searchShortcutLabel: string | undefined;
	readonly settingsLabel: string;
	readonly settings: (() => void) | undefined;
	readonly shortcuts: (() => void) | undefined;
	readonly shortcutsLabel: string;
	readonly shortcutsShortcutId: string;
	readonly shortcutsShortcutLabel: string | undefined;
	readonly settingsShortcutId: string;
	readonly settingsShortcutLabel: string | undefined;
	readonly sidebarFocusRequest: number;
	readonly setTabTitle: (tabId: string) => void;
	readonly nameEditor:
		| ((WorkbenchTabTitleEditorRequest | WorkbenchPanelNameEditorRequest) & {
				readonly draftTitle: string;
		  })
		| null;
	readonly nameEditorFinalFocus: () => HTMLElement | null;
	readonly nameEditorMessages: {
		readonly blank: NameDialogBlankRule;
		readonly cancel: string;
		readonly close: string;
		readonly nameTooLong: string;
		/** The automatic name an unnamed shared Tab keeps until one is typed. */
		readonly placeholder: string | undefined;
		readonly submit: string;
		readonly title: string;
	};
	readonly cancelNameEditor: () => void;
	readonly submitNameEditor: () => void;
	readonly updateNameEditor: (title: string) => void;
	/** The active Workspace's context; null while no Workspace is open. */
	readonly terminalContext: WorkbenchTerminalContext | null;
	readonly terminalAutoFocusAllowed: boolean;
	readonly terminalFocusRequest: WorkbenchTerminalFocusRequest | null;
	readonly hasVisibleTerminalTabs: boolean;
	/** Only a shared Workbench says anything in place of a Workspace's missing Tabs. */
	readonly emptyWorkspace: WorkbenchEmptyWorkspace | null;
	/** Double-clicking the content of a Workspace with no Tab starts one, as the New Tab control does. */
	readonly onEmptyContentDoubleClick: (event: React.MouseEvent<HTMLElement>) => void;
	readonly tabStripFocusRequest: number;
	readonly toggleSidebar: () => void;
	readonly toggleProjectExpansion: (projectId: string) => void;
	readonly toggleRecentsExpansion: () => void;
	readonly activateWorkspace: (
		workspaceId: string,
		origin: WorkbenchTerminalLocationActivationOrigin,
	) => void;
	readonly updateWorkspace: (workspaceId: string, name: string) => void;
	/** Present when the authority reaches Git for its Workspaces. */
	readonly worktreeCommands: WorkbenchWorktreeCommands | undefined;
}

interface UseWorkbenchOptions {
	readonly actionRuntime: ActionRuntime;
	readonly agentObservationSource?: WorkbenchAgentSource | undefined;
	readonly authority: WorkbenchAuthority;
	readonly configuredCommandCatalog: readonly WorkbenchConfiguredCommand[];
	readonly configuredCommandKeys: WorkbenchConfiguredCommandKeys;
	readonly isShortcutScopeActive: boolean;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly onSettings: WorkbenchHostActionCallback | undefined;
	readonly onShortcuts?: WorkbenchHostActionCallback | undefined;
	readonly onTerminalFontSizeChange?: WorkbenchTerminalFontSizeChangeCallback | undefined;
	readonly projectFolderAcquisition?: WorkbenchProjectFolderAcquisition | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	/**
	 * Whether the window offers Search across Agents, Actions and Settings. A release withholds it
	 * and keeps only the Actions filter, the TUI's action menu counterpart.
	 */
	readonly searchOffered: boolean;
	readonly shortcutBindings: WorkbenchShortcutBindings;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly storage: WorkbenchLayoutStorage | undefined;
	readonly terminalSurfaceAvailable: boolean;
}

export function useWorkbench({
	actionRuntime,
	agentObservationSource,
	authority,
	configuredCommandCatalog,
	configuredCommandKeys,
	isShortcutScopeActive,
	navigation,
	onSettings,
	onShortcuts,
	onTerminalFontSizeChange,
	projectFolderAcquisition,
	projectWorkspaceRuntime,
	searchOffered,
	shortcutBindings,
	shortcutPlatform,
	storage,
	terminalSurfaceAvailable,
}: UseWorkbenchOptions): WorkbenchView {
	const intl = useOrchbeeIntl();
	const visibleShortcutLabel = (label: string): string | undefined =>
		configuredCommandKeys.heldLabels.has(label) ? undefined : label;
	const startAgentShortcutLabel = visibleShortcutLabel(
		formatWorkbenchShortcut('startAgent', shortcutPlatform, shortcutBindings),
	);
	const authorityNotice = useWorkbenchAuthorityNotice(
		authority,
		projectWorkspaceRuntime,
		startAgentShortcutLabel,
	);
	const pendingProjectWorkspaceConfirmation = React.useRef<((accepted: boolean) => void) | null>(
		null,
	);
	const projectWorkspaceConfirmationFocusRef = React.useRef<HTMLButtonElement>(null);
	const [projectWorkspaceConfirmation, setProjectWorkspaceConfirmation] = React.useState<{
		readonly cancelLabel: string;
		readonly confirmLabel: string;
		readonly defaultAnswer: 'cancel' | 'confirm';
		readonly description: string;
		readonly title: string;
	} | null>(null);
	const projectWorkspaceState = React.useSyncExternalStore(
		projectWorkspaceRuntime.subscribe,
		projectWorkspaceRuntime.getSnapshot,
		projectWorkspaceRuntime.getSnapshot,
	);
	const terminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
	const tabModel = terminalContext?.tabModel;
	const readTabCount = () => tabModel?.getSnapshot().tabs.length ?? 0;
	const tabCount = React.useSyncExternalStore(
		tabModel?.subscribe ?? subscribeToNothing,
		readTabCount,
		readTabCount,
	);
	const sidebar = useWorkbenchSidebarPresentation(storage);
	const sidebarPresentation = sidebar.presentation;
	const isSidebarVisible = sidebarPresentation !== 'hidden';
	const resolveTerminalFocusTarget = useMemoizedFn(() =>
		terminalSurfaceAvailable ? resolveActiveTerminalPresentationId(projectWorkspaceRuntime) : null,
	);
	const focusRouting = useWorkbenchFocusRouting({
		isSidebarVisible,
		isTabStripVisible: !sidebar.mobile || sidebar.tabListOpen,
		resolveTerminalTarget: resolveTerminalFocusTarget,
	});
	const layoutFocusRouter = useWorkbenchMobileFocusRouter({
		focusRouting,
		hasTabs: tabModel !== undefined,
		sidebar,
	});
	const focusRouter = React.useMemo(
		() => ({
			beginTerminalHandover: focusRouting.beginTerminalHandover,
			focus: layoutFocusRouter.focus,
			getResolution: layoutFocusRouter.getResolution,
		}),
		[focusRouting.beginTerminalHandover, layoutFocusRouter.focus, layoutFocusRouter.getResolution],
	);
	// Wide windows show the horizontal strip; the mobile layout's Tab list is a vertical one.
	const tabStripOrientation = sidebar.mobile
		? TabStripOrientation.Vertical
		: TabStripOrientation.Horizontal;
	const [defaultLeftPanelWidth] = React.useState(() => readWorkbenchLeftPanelWidth(storage));
	const leftPanelWidth = React.useRef(defaultLeftPanelWidth);
	const leftPanelReference = React.useRef<ResizablePanelHandle>(null);
	const setLeftPanelWidth = useMemoizedFn((width: number) => {
		const next = Math.min(MAXIMUM_LEFT_PANEL_WIDTH, Math.max(MINIMUM_LEFT_PANEL_WIDTH, width));
		const panel = leftPanelReference.current;
		if (panel === null) return;
		panel.resize(next);
		leftPanelWidth.current = next;
		writeWorkbenchLeftPanelWidth(storage, next);
	});
	const [searchState, updateSearch] = React.useReducer(
		transitionWorkbenchSearch,
		undefined,
		createWorkbenchSearchState,
	);
	const searchHasOpened = React.useRef(false);
	const searchReturnFocus = React.useRef<HTMLElement | null>(null);
	const nameReturnFocus = React.useRef<HTMLElement | null>(null);
	const [nameEditor, setNameEditor] = React.useState<
		| ((WorkbenchTabTitleEditorRequest | WorkbenchPanelNameEditorRequest) & {
				readonly draftTitle: string;
		  })
		| null
	>(null);
	const automaticNamePlaceholder = nameEditor?.unnamed === true ? nameEditor.title : undefined;
	const searchFilters = searchOffered ? WORKBENCH_SEARCH_FILTERS : ACTION_SEARCH_FILTERS;
	const openSearch = useMemoizedFn((input: { readonly filter: WorkbenchSearchFilter }) => {
		searchHasOpened.current = true;
		if (
			searchReturnFocus.current === null &&
			typeof document !== 'undefined' &&
			typeof HTMLElement !== 'undefined' &&
			document.activeElement instanceof HTMLElement &&
			document.activeElement !== document.body &&
			document.activeElement !== document.documentElement
		) {
			searchReturnFocus.current = document.activeElement;
		}
		updateSearch({ filter: input.filter, type: 'open' });
	});
	React.useEffect(() => {
		if (
			searchState.kind !== 'closed' ||
			searchHasOpened.current === false ||
			typeof document === 'undefined'
		) {
			return;
		}
		searchHasOpened.current = false;
		const returnFocus =
			searchReturnFocus.current?.isConnected === true
				? searchReturnFocus.current
				: (document.querySelector<HTMLElement>('[data-slot="workbench-sidebar-search"]') ??
					document.querySelector<HTMLElement>('[data-slot="workbench-sidebar-toggle"]'));
		searchReturnFocus.current = null;
		returnFocus?.focus();
	}, [searchState.kind]);
	const openNameEditor = useMemoizedFn(
		(request: WorkbenchTabTitleEditorRequest | WorkbenchPanelNameEditorRequest) => {
			nameReturnFocus.current = findActiveTerminalFocusTarget(projectWorkspaceRuntime);
			setNameEditor({ ...request, draftTitle: request.unnamed ? '' : request.title });
		},
	);
	const nameEditorFinalFocus = useMemoizedFn(() => {
		const capturedTarget = nameReturnFocus.current;
		return capturedTarget?.isConnected === true
			? capturedTarget
			: findActiveTerminalFocusTarget(projectWorkspaceRuntime);
	});
	React.useEffect(() => {
		if (nameEditor === null) return;
		const captured = nameEditor;
		const reconcile = (): void => {
			const context = projectWorkspaceRuntime.getTerminalContext(captured.terminalContextId);
			const tab = context?.panelWorkspaceStore
				.getSnapshot()
				.tabs.find((candidate) => candidate.tabId === captured.tabId);
			const valid =
				tab !== undefined &&
				(!('panelId' in captured) ||
					findPanel(tab.root, captured.panelId)?.primaryPresentation.presentationId ===
						captured.presentationId);
			if (!valid) setNameEditor((current) => (current === captured ? null : current));
		};
		reconcile();
		return projectWorkspaceRuntime.subscribeTerminalTopology(reconcile);
	}, [nameEditor, projectWorkspaceRuntime]);
	const agentDialogs = useWorkbenchAgentDialogs({
		agentCatalog: authorityNotice.authority.agentCatalog,
		projectWorkspaceRuntime,
	});
	const toggleSidebar = sidebar.toggle;
	const closeSidebarOverlay = sidebar.dismissOverlay;
	// The overlay and the full-window lists take the keyboard as they open, as the TUI's do.
	const sidebarOverlayOpen = sidebarPresentation === 'overlay' || sidebarPresentation === 'fill';
	const tabListOpen = sidebar.tabListOpen;
	const focusRegion = focusRouting.focus;
	React.useEffect(() => {
		if (sidebarOverlayOpen) focusRegion({ kind: 'direct', region: 'sidebar' });
	}, [focusRegion, sidebarOverlayOpen]);
	React.useEffect(() => {
		if (tabListOpen) focusRegion({ kind: 'direct', region: 'tab-strip' });
	}, [focusRegion, tabListOpen]);
	const worktreeForm = useWorkbenchWorktreeForm({ projectWorkspaceRuntime });
	const worktreeLabel = useMemoizedFn((ordinal: number) =>
		intl.formatMessage(workbenchMessages.worktreeDefaultLabel, { ordinal }),
	);
	const formatCloseWarning = useMemoizedFn((impact: TerminalCloseImpact) =>
		intl.formatMessage(workbenchMessages.terminalCloseWarning, terminalCloseWarningValues(impact)),
	);
	const getNewTerminalAvailability = useMemoizedFn(
		(input: WorkbenchTerminalCreateInput): ActionAvailability =>
			getSharedTerminalCreationAvailability(projectWorkspaceRuntime, input),
	);
	const getTabCreationAvailability = useMemoizedFn((intent: TabStripTabCreationIntent) =>
		getSharedTabCreationAvailability(projectWorkspaceRuntime, intent),
	);
	const createTerminal = useMemoizedFn<
		void,
		Parameters<WorkbenchTerminalCreateCallback>,
		ReturnType<WorkbenchTerminalCreateCallback>
	>((context, input) =>
		createSharedTerminal(context, input, {
			authority: authorityNotice.authority,
			beginTerminalHandover: focusRouting.beginTerminalHandover,
			runtime: projectWorkspaceRuntime,
		}),
	);
	const createPanelTerminal = useMemoizedFn<
		void,
		Parameters<WorkbenchSharedPanelTerminalCreate>,
		ReturnType<WorkbenchSharedPanelTerminalCreate>
	>((context, creation) =>
		createSharedPanelTerminal(context, creation, {
			authority: authorityNotice.authority,
			beginTerminalHandover: focusRouting.beginTerminalHandover,
			runtime: projectWorkspaceRuntime,
		}),
	);
	const changeSharedTabs = useMemoizedFn<
		void,
		Parameters<TabStripChangeCallback>,
		ReturnType<TabStripChangeCallback>
	>((context, change) => {
		const decider = authorityNotice.authority;
		switch (change.kind) {
			case 'tab-created-in-new-group':
				return createSharedTab(
					context,
					{ placement: { kind: 'new-group' } },
					{
						authority: decider,
						beginTerminalHandover: focusRouting.beginTerminalHandover,
						runtime: projectWorkspaceRuntime,
					},
				);
			case 'tabs-duplicated':
				return duplicateSharedTabs(context, change.tabIds, {
					authority: decider,
					onStopped: authorityNotice.reportDuplicationStopped,
					runtime: projectWorkspaceRuntime,
				});
			default: {
				const activeContext = projectWorkspaceRuntime.getActiveTerminalContext();
				return activeContext === null
					? ACTIVE_TAB_UNAVAILABLE
					: decider.change(context, {
							change,
							terminalContextId: activeContext.terminalContextId,
							type: 'tabs-changed',
						});
			}
		}
	});
	/**
	 * In the order the strip shows them, which is the order a set of Tabs closes in. As in the TUI,
	 * one Tab or a set closes unasked when nothing runs in any of them, which only the authority
	 * knows, and otherwise asks once for all of them.
	 */
	const closeSharedTabs = async (
		context: Parameters<TabStripCloseCallback>[0],
		decider: WorkbenchAuthority,
		tabIds: readonly string[],
	): Promise<ActionResult<void>> => {
		const activeContext = projectWorkspaceRuntime.getActiveTerminalContext();
		if (activeContext === null) return ACTIVE_TAB_UNAVAILABLE;
		const closure = sharedTabsClosure(activeContext, tabIds);
		const result = await closeReturningToTerminal(
			decider,
			context,
			closure,
			focusRouting.beginTerminalHandover(),
			actions.focusAfterTabClose,
		);
		if (result.kind !== 'blocked' || result.code !== 'confirmation-required') return result;
		// The question is its own Action, because this one has already asked the authority.
		actions.invoke(
			WORKBENCH_ACTION_IDS.closeTabConfirmed,
			{ tabIds: closure.tabIds, terminalContextId: closure.terminalContextId },
			context.source,
		);
		return { kind: 'unchanged' };
	};
	const closeTabs = useMemoizedFn<
		void,
		Parameters<TabStripCloseCallback>,
		ReturnType<TabStripCloseCallback>
	>((context, request) =>
		context.signal.aborted
			? { kind: 'cancelled' }
			: closeSharedTabs(context, authorityNotice.authority, request.tabIds),
	);
	const actions = useWorkbenchActions({
		agentObservationSource,
		authority: authorityNotice.authority,
		configuredCommandCatalog,
		configuredCommandKeys,
		focusRouting: focusRouter,
		formatCloseWarning,
		getNewTerminalAvailability,
		isSidebarVisible,
		navigation,
		onNewTerminal: createTerminal,
		onSettings,
		onShortcuts,
		onTerminalFontSizeChange,
		openAgentStart: agentDialogs.openStart,
		openSearch,
		openTabTitleEditor: openNameEditor,
		openPanelNameEditor: openNameEditor,
		projectWorkspaceRuntime,
		projectWorkspaceState,
		requestWorktreeForm: worktreeForm.request,
		runtime: actionRuntime,
		searchFilters,
		tabStripOrientation,
		toggleSidebar,
		worktreeLabel,
	});
	const startAgent = useMemoizedFn((input: WorkbenchAgentStartInput) => {
		// No stacked dialogs: New Worktree… closes Start Agent and its form opens in its place.
		agentDialogs.endStart(input.place === 'new-worktree');
		actions.invoke(WORKBENCH_ACTION_IDS.startAgent, input, 'button');
	});
	const createProject = useMemoizedFn((path: string) =>
		actions.invoke(WORKBENCH_ACTION_IDS.createProject, { path }, 'button'),
	);
	const activateCreatedProject = useMemoizedFn((receipt: WorkbenchProjectCreationReceipt) =>
		actions.invoke(
			WORKBENCH_ACTION_IDS.activateWorkspace,
			{ creationReceipt: receipt, workspaceId: receipt.workspaceId },
			'button',
		),
	);
	const isCreatedProjectActive = useMemoizedFn((receipt: WorkbenchProjectCreationReceipt) => {
		const location = projectWorkspaceRuntime.getSnapshot().activeLocation;
		return (
			location.kind === 'workspace' &&
			location.workspaceId === receipt.workspaceId &&
			(navigation === undefined || navigation.isActive?.(location) === true) &&
			matchesWorkbenchProjectCreation(projectWorkspaceRuntime, receipt)
		);
	});
	const projectCreation = useWorkbenchProjectCreation({
		acquisition: projectFolderAcquisition,
		activateCreatedProject,
		createProject,
		createWorkspaceAtDirectory: (projectId, path) =>
			actions.invoke(WORKBENCH_ACTION_IDS.createWorkspace, { projectId, path }, 'menu'),
		isCreatedProjectActive,
		onDirectoryRefusalShown: authorityNotice.withdrawDirectoryRefusal,
		runtime: actionRuntime,
		scope: actions.scope,
	});
	const worktrees = authorityNotice.authority.worktrees;
	const newFolderAvailable = projectFolderAcquisition?.newFolder !== undefined;
	const searchArgumentPickerForAction = React.useMemo(() => {
		const worktreePicker =
			worktrees === undefined
				? undefined
				: createFocusedPanelWorktreePicker({ intl, projectWorkspaceRuntime, worktrees });
		// The folder choices the sidebar's New Project menu offered before Search took it over.
		const newProjectPicker: WorkbenchSearchProviderArgumentPicker = {
			kind: 'options',
			options: [
				{
					id: 'existing',
					input: { folder: 'existing' },
					keywords: [],
					label: intl.formatMessage(workbenchMessages.projectSelectFolder),
				},
				...(newFolderAvailable
					? [
							{
								id: 'new',
								input: { folder: 'new' },
								keywords: [],
								label: intl.formatMessage(workbenchMessages.projectNewFolder),
							},
						]
					: []),
			],
			placeholder: intl.formatMessage(workbenchMessages.searchArgumentPlaceholder),
			prompt: intl.formatMessage(workbenchMessages.searchArgumentPrompt, {
				action: intl.formatMessage(workbenchMessages.projectNew),
			}),
		};
		return (actionId: string) => {
			switch (actionId) {
				case WORKBENCH_ACTION_IDS.openFocusedPanelWorktree:
					return worktreePicker;
				case WORKBENCH_ACTION_IDS.newProject:
					return newProjectPicker;
				default:
					return undefined;
			}
		};
	}, [intl, newFolderAvailable, projectWorkspaceRuntime, worktrees]);
	const createTabWithProfile = useMemoizedFn((profile: UserLaunchProfile) => {
		actions.invoke(WORKBENCH_ACTION_IDS.createTerminal, { groupId: null, profile }, 'context-menu');
	});
	const openAgentStartForNewTab = useMemoizedFn(() => {
		actions.invoke(WORKBENCH_ACTION_IDS.openAgentStart, undefined, 'context-menu');
	});
	const startAgentInNewTab = useMemoizedFn((providerId: WorkbenchAgentStartProvider) => {
		actions.invoke(
			WORKBENCH_ACTION_IDS.startAgent,
			{ place: 'new-tab', providerId },
			'context-menu',
		);
	});
	const newTabSections = useWorkbenchNewTabChoices({
		agentCatalog: authorityNotice.authority.agentCatalog,
		createTab: createTabWithProfile,
		inWorkspace: projectWorkspaceState.activeLocation.kind === 'workspace',
		launchProfiles: authorityNotice.authority.launchProfiles,
		openAgentStart: openAgentStartForNewTab,
		startAgentInNewTab,
		startAgentShortcutLabel,
	});
	const invokeLocationActivation = useMemoizedFn(
		(targetLocation: WorkbenchActiveLocation, source: 'button' | 'keyboard' | 'pointer') => {
			// No Workspace opens by itself once none is left, so nothing activates it.
			if (targetLocation.kind !== 'workspace') throw new Error('Only a Workspace activates.');
			return actions.invoke(
				WORKBENCH_ACTION_IDS.activateWorkspace,
				{ workspaceId: targetLocation.workspaceId },
				source,
			);
		},
	);
	const requestLocationTerminalFocus = useMemoizedFn(
		(targetLocation: WorkbenchActiveLocation): boolean => {
			if (
				!workbenchActiveLocationsEqual(
					projectWorkspaceRuntime.getSnapshot().activeLocation,
					targetLocation,
				)
			) {
				return false;
			}
			return focusRouting.requestTerminalFocus();
		},
	);
	const activateLocation = useWorkbenchTerminalLocationActivation({
		activeLocation: projectWorkspaceState.activeLocation,
		invokeLocationActivation,
		requestTerminalFocus: requestLocationTerminalFocus,
		terminalTargetAvailable: resolveTerminalFocusTarget() !== null,
	});
	const activateWorkspace = useMemoizedFn(
		(workspaceId: string, origin: WorkbenchTerminalLocationActivationOrigin) =>
			activateLocation({ kind: 'workspace', workspaceId }, origin),
	);
	const invokeAgentNavigation = useMemoizedFn((input: WorkbenchAgentNavigationInput) =>
		actions.invoke(WORKBENCH_ACTION_IDS.navigateAgent, input, 'button'),
	);
	const navigateAgent = useWorkbenchAgentNavigation({
		activeLocation: projectWorkspaceState.activeLocation,
		announceUnavailable: authorityNotice.reportAgentNavigationUnavailable,
		invokeAgentNavigation,
		projectWorkspaceRuntime,
		requestTerminalFocus: focusRouting.requestTerminalFocus,
		source: agentObservationSource,
	});
	React.useEffect(() => {
		const formatActionMessage = (message: ActionConfirmation['title']) => {
			if (message.id === undefined) {
				throw new Error('A Workbench confirmation message must have a stable ID.');
			}
			return intl.formatMessage(
				{
					id: message.id,
					...(message.defaultMessage === undefined
						? {}
						: { defaultMessage: message.defaultMessage }),
				},
				message.values,
			);
		};
		const registration = actionRuntime.registerConfirmationPresenter(
			actions.scope,
			(confirmation, _invocation, signal) =>
				new Promise<boolean>((resolve) => {
					pendingProjectWorkspaceConfirmation.current?.(false);
					let settled = false;
					const finish = (accepted: boolean) => {
						if (settled) return;
						settled = true;
						signal.removeEventListener('abort', onAbort);
						if (pendingProjectWorkspaceConfirmation.current === finish) {
							pendingProjectWorkspaceConfirmation.current = null;
							setProjectWorkspaceConfirmation(null);
						}
						resolve(accepted);
					};
					const onAbort = () => finish(false);
					pendingProjectWorkspaceConfirmation.current = finish;
					setProjectWorkspaceConfirmation({
						cancelLabel:
							confirmation.cancelLabel === undefined
								? intl.formatMessage(workbenchMessages.panelCancel)
								: formatActionMessage(confirmation.cancelLabel),
						confirmLabel: formatActionMessage(confirmation.confirmLabel),
						defaultAnswer: confirmation.defaultAnswer ?? 'cancel',
						description: formatActionMessage(confirmation.description),
						title: formatActionMessage(confirmation.title),
					});
					signal.addEventListener('abort', onAbort, { once: true });
				}),
		);
		return () => {
			registration.dispose();
			pendingProjectWorkspaceConfirmation.current?.(false);
		};
	}, [actionRuntime, actions.scope, intl]);

	// A configured command's chord runs it before any window shortcut sees the key, as in the TUI.
	const invokeConfiguredCommandKey = useMemoizedFn((event: KeyboardEvent): boolean => {
		const commandId = findWorkbenchConfiguredCommandForKey(
			configuredCommandKeys,
			event,
			shortcutPlatform,
		);
		if (commandId === null) return false;
		const actionId = workbenchConfiguredCommandActionId(commandId);
		const invocation = { scope: actions.scope, source: 'keyboard' } as const;
		if (
			actionRuntime.resolveAction(actionId, undefined, invocation)?.availability.kind !== 'enabled'
		) {
			return false;
		}
		actionRuntime.execute(actionId, undefined, invocation);
		return true;
	});
	const shortcutScopeReference = useWorkbenchShortcuts(
		isShortcutScopeActive,
		shortcutPlatform,
		shortcutBindings,
		actions.invokeShortcut,
		invokeConfiguredCommandKey,
	);

	return {
		agentStart: agentDialogs.start,
		closeAgentStart: agentDialogs.closeStart,
		startAgent,
		worktreeForm: worktreeForm.form,
		authority: authorityNotice.authority,
		authorityNotice: authorityNotice.notice,
		navigateAgent: (input) => {
			closeSidebarOverlay();
			navigateAgent(input);
		},
		actionScope: actions.scope,
		// Choosing a row closes the overlay; a drag making its Workspace current does not.
		activateWorkspace: (workspaceId, origin) => {
			if (origin !== 'other') closeSidebarOverlay();
			activateWorkspace(workspaceId, origin);
		},
		cancelProjectWorkspaceConfirmation: () => pendingProjectWorkspaceConfirmation.current?.(false),
		confirmProjectWorkspaceAction: () => pendingProjectWorkspaceConfirmation.current?.(true),
		closeAgentPanel: (input) => {
			actions.invoke(WORKBENCH_ACTION_IDS.closePanelTarget, input, 'menu');
		},
		renameAgentPanel: (input) => {
			actions.invoke(WORKBENCH_ACTION_IDS.renamePanel, { kind: 'open', ...input }, 'menu');
		},
		setWorkspaceProject: (workspaceId, projectId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.setWorkspaceProject, { workspaceId, projectId }, 'menu');
		},
		createWorkspaceProject: (workspaceId, name) => {
			actions.invoke(WORKBENCH_ACTION_IDS.createWorkspaceProject, { workspaceId, name }, 'button');
		},
		createWorkspace: (input) => {
			closeSidebarOverlay();
			if (input !== undefined && 'selectDirectory' in input) {
				projectCreation.requestWorkspaceDirectory(
					input.projectId,
					intl.formatMessage(workbenchMessages.projectCreateWorkspaceAtDirectory),
				);
			} else actions.invoke(WORKBENCH_ACTION_IDS.createWorkspace, input, 'menu');
		},
		deleteWorkspace: (workspaceId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.deleteWorkspace, { workspaceId }, 'menu');
		},
		isSidebarVisible,
		sidebarPresentation,
		mobileLayout: sidebar.mobile,
		touchSizes: sidebar.touch,
		tabListOpen,
		toggleTabList: sidebar.toggleTabList,
		dismissTabList: () => {
			sidebar.dismissTabList();
			focusRouting.requestTerminalFocus();
		},
		onTabListKeyDown: (event) => {
			if (!isUnclaimedEscape(event)) return;
			event.preventDefault();
			sidebar.dismissTabList();
			focusRouting.requestTerminalFocus();
		},
		activeWorkspaceName: findActiveWorkspaceName(projectWorkspaceState),
		dismissSidebarOverlay: () => {
			closeSidebarOverlay();
			focusRouting.requestTerminalFocus();
		},
		onSidebarOverlayKeyDown: (event) => {
			if (!isUnclaimedEscape(event)) return;
			event.preventDefault();
			closeSidebarOverlay();
			focusRouting.requestTerminalFocus();
		},
		sidebarWidth: leftPanelWidth.current,
		onSidebarDividerKeyDown: (event) => {
			if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
			if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
			event.preventDefault();
			setLeftPanelWidth(leftPanelWidth.current + (event.key === 'ArrowLeft' ? -10 : 10));
		},
		leftPanelReference,
		leftPanelSize: `${leftPanelWidth.current}px`,
		newTab: (request) =>
			actions.invoke(
				WORKBENCH_ACTION_IDS.createTerminal,
				{
					groupId: request.groupId,
					...(request.index === undefined ? {} : { index: request.index }),
				},
				request.source,
			),
		newTabSections,
		onCloseTabs: closeTabs,
		createPanelTerminal,
		onTabsChange: changeSharedTabs,
		getTabCreationAvailability,
		onLayoutChanged: (_layout, meta) => {
			if (meta.isUserInteraction) {
				writeWorkbenchLeftPanelWidth(storage, leftPanelWidth.current);
			}
		},
		onLeftPanelResize: (size) => {
			leftPanelWidth.current = size.inPixels;
		},
		resetSidebarWidth: () => setLeftPanelWidth(DEFAULT_LEFT_PANEL_WIDTH),
		returnToTerminal: () => {
			closeSidebarOverlay();
			sidebar.dismissTabList();
			focusRouting.requestTerminalFocus();
		},
		onRootBlurCapture: focusRouting.onBlurCapture,
		onRootFocusCapture: focusRouting.onFocusCapture,
		shortcutScopeReference,
		sidebarShortcutId: WORKBENCH_SHORTCUTS.toggleSidebar.id,
		sidebarShortcutLabel: visibleShortcutLabel(
			formatWorkbenchShortcut('toggleSidebar', shortcutPlatform, shortcutBindings),
		),
		sidebarToggleLabel: intl.formatMessage(
			isSidebarVisible ? workbenchMessages.hideSidebar : workbenchMessages.showSidebar,
		),
		sidebarLabel: intl.formatMessage(workbenchMessages.sidebarLabel),
		projectLabel: intl.formatMessage(workbenchMessages.project),
		newWorkspaceLabel: intl.formatMessage(workbenchMessages.sidebarNewWorkspace),
		newWorkspaceShortcutId: WORKBENCH_SHORTCUTS.newWorkspace.id,
		newWorkspaceShortcutLabel: visibleShortcutLabel(
			formatWorkbenchShortcut('newWorkspace', shortcutPlatform, shortcutBindings),
		),
		projectNavigationMessages: {
			agentMenuClose: intl.formatMessage(workbenchMessages.panelClose),
			agentMenuRename: intl.formatMessage(workbenchMessages.panelRenameMenu),
			workspaceMoveToProject: intl.formatMessage(workbenchMessages.workspaceMoveToProject),
			workspaceCreateProject: intl.formatMessage(workbenchMessages.workspaceCreateProject),
			workspaceUngroup: intl.formatMessage(workbenchMessages.workspaceUngroup),
			cancel: intl.formatMessage(workbenchMessages.resourceCancel),
			close: intl.formatMessage(workbenchMessages.resourceClose),
			closeWorkspace: intl.formatMessage(workbenchMessages.workspaceDelete),
			collapseProject: intl.formatMessage(workbenchMessages.projectCollapse),
			collapseRecents: intl.formatMessage(workbenchMessages.recentsCollapse),
			create: intl.formatMessage(workbenchMessages.resourceCreate),
			createWorkspace: intl.formatMessage(workbenchMessages.projectCreateWorkspace),
			createWorkspaceAtDirectory: intl.formatMessage(
				workbenchMessages.projectCreateWorkspaceAtDirectory,
			),
			dropEndOfProjects: intl.formatMessage(workbenchMessages.sidebarDropEndOfProjects),
			expandProject: intl.formatMessage(workbenchMessages.projectExpand),
			expandRecents: intl.formatMessage(workbenchMessages.recentsExpand),
			nameRequired: intl.formatMessage(workbenchMessages.nameRequired),
			nameTooLong: intl.formatMessage(workbenchMessages.nameTooLong),
			renameSubmit: intl.formatMessage(workbenchMessages.renameSubmit),
			projectFolders: intl.formatMessage(workbenchMessages.projectFolders),
			projectMenuDissolve: intl.formatMessage(workbenchMessages.projectMenuDissolve),
			projectMenuRename: intl.formatMessage(workbenchMessages.projectMenuRename),
			projectOptions: intl.formatMessage(workbenchMessages.projectOptions),
			projectRenameTitle: intl.formatMessage(workbenchMessages.projectRenameTitle),
			projectWorktrees: intl.formatMessage(workbenchMessages.projectWorktrees),
			recents: intl.formatMessage(workbenchMessages.recents),
			workspaceMenuClose: intl.formatMessage(workbenchMessages.workspaceMenuClose),
			workspaceMenuNewProject: intl.formatMessage(workbenchMessages.workspaceMenuNewProject),
			workspaceMenuRename: intl.formatMessage(workbenchMessages.workspaceMenuRename),
			workspaceOptions: intl.formatMessage(workbenchMessages.workspaceOptions),
			workspaceRenameTitle: intl.formatMessage(workbenchMessages.workspaceRenameTitle),
		},
		projectCreationMessages: {
			cancel: intl.formatMessage(workbenchMessages.resourceCancel),
			close: intl.formatMessage(workbenchMessages.resourceClose),
			create: intl.formatMessage(workbenchMessages.resourceCreate),
			folderAcquisitionFailureDismiss: intl.formatMessage(
				workbenchMessages.projectFolderAcquisitionDismiss,
			),
			folderAcquisitionFailureMessages: {
				'already-exists': intl.formatMessage(
					workbenchMessages.projectFolderAcquisitionAlreadyExists,
				),
				'invalid-name': intl.formatMessage(workbenchMessages.projectFolderAcquisitionInvalidName),
				'stale-parent': intl.formatMessage(workbenchMessages.projectFolderAcquisitionStaleParent),
				busy: intl.formatMessage(workbenchMessages.projectFolderAcquisitionBusy),
				'control-unavailable': intl.formatMessage(
					workbenchMessages.projectFolderAcquisitionControlUnavailable,
				),
				'dialog-failed': intl.formatMessage(workbenchMessages.projectFolderAcquisitionDialogFailed),
				'not-found': intl.formatMessage(workbenchMessages.projectFolderAcquisitionNotFound),
				'permission-denied': intl.formatMessage(
					workbenchMessages.projectFolderAcquisitionPermissionDenied,
				),
				'validation-failed': intl.formatMessage(
					workbenchMessages.projectFolderAcquisitionValidationFailed,
				),
			},
			folderAcquisitionFailureTitle: intl.formatMessage(
				workbenchMessages.projectFolderAcquisitionTitle,
			),
			folderRefusalMessages: {
				'directory-inaccessible': intl.formatMessage(
					workbenchMessages.workspaceSetupDirectoryRefusedInaccessible,
				),
				'directory-missing': intl.formatMessage(
					workbenchMessages.workspaceSetupDirectoryRefusedMissing,
				),
				'directory-unavailable': intl.formatMessage(
					workbenchMessages.workspaceSetupDirectoryRefusedUnavailable,
				),
				'not-a-directory': intl.formatMessage(
					workbenchMessages.workspaceSetupDirectoryRefusedNotADirectory,
				),
			},
			newFolderCreating: intl.formatMessage(workbenchMessages.projectNewFolderCreating),
			newFolderDescription: intl.formatMessage(workbenchMessages.projectNewFolderDescription),
			newFolderName: intl.formatMessage(workbenchMessages.projectNewFolderName),
			newFolderNameHint: intl.formatMessage(workbenchMessages.projectNewFolderNameHint),
			newFolderOpening: intl.formatMessage(workbenchMessages.projectNewFolderOpening),
			newFolderParent: intl.formatMessage(workbenchMessages.projectNewFolderParent),
			newFolderRetained: intl.formatMessage(workbenchMessages.projectNewFolderRetained),
			newFolderTitle: intl.formatMessage(workbenchMessages.projectNewFolderTitle),
			newFolderUnknown: intl.formatMessage(workbenchMessages.projectNewFolderUnknown),
			projectCreateDescription: intl.formatMessage(workbenchMessages.projectCreateDescription),
			projectCreatePath: intl.formatMessage(workbenchMessages.projectCreatePath),
			projectCreatePathDescription: intl.formatMessage(
				workbenchMessages.projectCreatePathDescription,
			),
			projectCreatePathPlaceholder: intl.formatMessage(
				workbenchMessages.projectCreatePathPlaceholder,
			),
			projectCreateTitle: intl.formatMessage(workbenchMessages.projectCreateTitle),
			selectFolder: intl.formatMessage(workbenchMessages.projectSelectFolder),
		},
		projectCreation,
		projectWorkspaceConfirmationFocusRef,
		projectWorkspaceConfirmation,
		projectWorkspaceState,
		removeProject: (projectId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.removeProject, { projectId }, 'menu');
		},
		reorderProject: (projectId, beforeProjectId) =>
			actions.invoke(WORKBENCH_ACTION_IDS.reorderProject, { beforeProjectId, projectId }, 'pointer')
				.completion,
		moveWorkspace: (workspaceId, projectId, beforeWorkspaceId) =>
			actions.invoke(
				WORKBENCH_ACTION_IDS.reorderWorkspace,
				{ beforeWorkspaceId, projectId, workspaceId },
				'pointer',
			).completion,
		renameProject: (projectId, name) => {
			actions.invoke(WORKBENCH_ACTION_IDS.renameProject, { name, projectId }, 'button');
		},
		search: searchOffered
			? (returnFocus) => {
					closeSidebarOverlay();
					searchReturnFocus.current = returnFocus;
					actions.invoke(WORKBENCH_ACTION_IDS.openSearch, { filter: 'all' }, 'button');
				}
			: undefined,
		searchArgumentPickerForAction,
		searchFilters,
		searchState,
		searchLabel: intl.formatMessage(workbenchMessages.search),
		searchShortcutId: WORKBENCH_SHORTCUTS.search.id,
		searchShortcutLabel: visibleShortcutLabel(
			formatWorkbenchShortcut('search', shortcutPlatform, shortcutBindings),
		),
		settings:
			onSettings === undefined
				? undefined
				: () => {
						closeSidebarOverlay();
						actions.invoke(WORKBENCH_ACTION_IDS.openSettings, undefined, 'button');
					},
		settingsLabel: intl.formatMessage(workbenchMessages.settings),
		shortcuts:
			onShortcuts === undefined
				? undefined
				: () => {
						closeSidebarOverlay();
						actions.invoke(WORKBENCH_ACTION_IDS.openShortcuts, undefined, 'button');
					},
		shortcutsLabel: intl.formatMessage(workbenchMessages.shortcuts),
		shortcutsShortcutId: WORKBENCH_SHORTCUTS.shortcuts.id,
		shortcutsShortcutLabel: visibleShortcutLabel(
			formatWorkbenchShortcut('shortcuts', shortcutPlatform, shortcutBindings),
		),
		settingsShortcutId: WORKBENCH_SHORTCUTS.settings.id,
		settingsShortcutLabel: visibleShortcutLabel(
			formatWorkbenchShortcut('settings', shortcutPlatform, shortcutBindings),
		),
		sidebarFocusRequest: focusRouting.sidebarFocusRequest,
		setTabTitle: (tabId) => {
			const activeTerminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
			if (activeTerminalContext === null) return;
			actions.invoke(
				WORKBENCH_ACTION_IDS.setTabTitle,
				{ kind: 'open', tabId, terminalContextId: activeTerminalContext.terminalContextId },
				'context-menu',
			);
		},
		nameEditor,
		nameEditorFinalFocus,
		nameEditorMessages: {
			// Tab names remain fixed; a blank Panel name restores its creation fallback.
			blank:
				nameEditor !== null && 'panelId' in nameEditor
					? { kind: 'accept', help: intl.formatMessage(workbenchMessages.panelRenameHelp) }
					: { kind: 'refuse', reason: intl.formatMessage(workbenchMessages.nameRequired) },
			cancel: intl.formatMessage(workbenchMessages.resourceCancel),
			close: intl.formatMessage(workbenchMessages.resourceClose),
			nameTooLong: intl.formatMessage(workbenchMessages.nameTooLong),
			placeholder:
				nameEditor !== null && 'panelId' in nameEditor
					? nameEditor.fallbackTitle
					: automaticNamePlaceholder,
			submit: intl.formatMessage(workbenchMessages.renameSubmit),
			title: intl.formatMessage(
				nameEditor !== null && 'panelId' in nameEditor
					? workbenchMessages.panelRenameTitle
					: workbenchTabMessages.renameTab,
			),
		},
		cancelNameEditor: () => setNameEditor(null),
		submitNameEditor: () => {
			if (nameEditor === null) return;
			if ('panelId' in nameEditor) {
				actions.invoke(
					WORKBENCH_ACTION_IDS.renamePanel,
					{
						kind: 'commit',
						panelId: nameEditor.panelId,
						presentationId: nameEditor.presentationId,
						tabId: nameEditor.tabId,
						terminalContextId: nameEditor.terminalContextId,
						title: nameEditor.draftTitle,
					},
					'button',
				);
				setNameEditor(null);
				return;
			}
			actions.invoke(
				WORKBENCH_ACTION_IDS.setTabTitle,
				{
					kind: 'commit',
					tabId: nameEditor.tabId,
					terminalContextId: nameEditor.terminalContextId,
					title: nameEditor.draftTitle,
				},
				'button',
			);
			setNameEditor(null);
		},
		updateNameEditor: (title) => {
			setNameEditor((current) => (current === null ? null : { ...current, draftTitle: title }));
		},
		terminalContext,
		terminalAutoFocusAllowed: focusRouting.terminalAutoFocusAllowed,
		terminalFocusRequest: focusRouting.terminalFocusRequest,
		hasVisibleTerminalTabs: tabCount > 0,
		emptyWorkspace:
			tabCount > 0
				? null
				: projectWorkbenchEmptyWorkspace(projectWorkspaceState, intl, agentDialogs.startOffered),
		onEmptyContentDoubleClick: (event) => {
			if (!isPlainPrimaryPress(event) || tabCount > 0) return;
			actions.invoke(WORKBENCH_ACTION_IDS.createTerminal, { groupId: null }, 'pointer');
		},
		tabStripFocusRequest: focusRouting.tabStripFocusRequest,
		toggleSidebar: () => {
			actions.invoke(WORKBENCH_ACTION_IDS.toggleSidebar, undefined, 'button');
		},
		toggleProjectExpansion: (projectId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.toggleProjectExpansion, { projectId }, 'button');
		},
		toggleRecentsExpansion: () => {
			actions.invoke(WORKBENCH_ACTION_IDS.toggleRecentsExpansion, undefined, 'button');
		},
		updateWorkspace: (workspaceId, name) => {
			actions.invoke(WORKBENCH_ACTION_IDS.updateWorkspace, { name, workspaceId }, 'button');
		},
		updateSearch,
		worktreeCommands:
			worktrees === undefined
				? undefined
				: {
						create: (workspaceId) =>
							actions.invoke(WORKBENCH_ACTION_IDS.createWorktree, { workspaceId }, 'menu'),
						discover: (workspaceId) => worktrees.discover(workspaceId),
						open: (input) => actions.invoke(WORKBENCH_ACTION_IDS.openWorktree, input, 'menu'),
						remove: (input) => actions.invoke(WORKBENCH_ACTION_IDS.removeWorktree, input, 'menu'),
						resume: (input) =>
							actions.invoke(WORKBENCH_ACTION_IDS.resumeWorktreeRemoval, input, 'menu'),
					},
	};
}

/** A plain primary-button press, as the TUI's double-click shortcuts take. */
function isPlainPrimaryPress(event: React.MouseEvent<HTMLElement>): boolean {
	return event.button === 0 && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
}

/** Esc that no menu, field or drag inside the surface has already taken. */
function isUnclaimedEscape(event: React.KeyboardEvent<HTMLElement>): boolean {
	return (
		event.key === 'Escape' &&
		!event.defaultPrevented &&
		event.target instanceof Node &&
		event.currentTarget.contains(event.target) &&
		event.currentTarget.querySelector('[data-dragging]') === null
	);
}

function findActiveWorkspaceName(state: ProjectWorkspaceState): string | null {
	const location = state.activeLocation;
	if (location.kind !== 'workspace') return null;
	return state.workspaces.find((workspace) => workspace.id === location.workspaceId)?.name ?? null;
}
