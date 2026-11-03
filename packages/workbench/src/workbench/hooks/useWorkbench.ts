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
import {
	type TabModel,
	type TabStripChangeCallback,
	type TabStripCloseCallback,
	type TabStripCloseConfirmation,
	type TabStripNewTabChoice,
	type TabStripNewTabRequest,
	type TabStripOrientation,
	type TabStripTabCreationIntent,
} from '@orchbee/tab-strip';
import * as React from 'react';
import {
	workbenchSharedDocumentRevision,
	type UserLaunchProfile,
} from '@orchbee/workbench-client-protocol';

import {
	readWorkbenchLeftPanelWidth,
	readWorkbenchSidebarVisibility,
	readWorkbenchTabStripOrientation,
	writeWorkbenchLeftPanelWidth,
	writeWorkbenchSidebarVisibility,
	writeWorkbenchTabStripOrientation,
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
import type { WorkbenchSearchFilter } from '../types/workbench-search';
import type {
	WorkbenchProjectNavigationMessages,
	WorkbenchWorktreeCommands,
} from '../components/workbench-sidebar/types/workbench-sidebar-types';
import type { WorkbenchRemoteTerminalCreationCapability } from '../types/workbench-remote-terminal-creation';
import type { WorkbenchProjectCreationReceipt } from '../types/workbench-project-folder-acquisition';
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
	type WorkbenchTerminalCreateInput,
} from '../actions/workbench-action-ids';
import type { WorkbenchTabTitleEditorRequest } from '../actions/registerWorkbenchTabTitleAction';
import type { NameDialogBlankRule } from '../components/name-dialog/NameDialog';
import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalContext,
} from '../model/WorkbenchProjectWorkspaceRuntime';
import { findActiveTerminalFocusTarget } from '../utils/findActiveTerminalFocusTarget';
import { resolveActiveTerminalPresentationId } from '../utils/resolveActiveTerminalPresentationId';
import {
	executeWorkbenchTerminalCreationAction,
	getWorkbenchTabCreationAvailability,
	getWorkbenchTerminalCreationAvailability,
} from '../actions/workbench-terminal-creation-action';
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
import type { WorkbenchAuthority } from '../types/workbench-authority';
import { useWorkbenchActions } from './useWorkbenchActions';
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
import { terminalCloseWarningValues } from '../model/terminal-close-impact';
import {
	workbenchActiveLocationsEqual,
	type WorkbenchTerminalLocationActivationOrigin,
} from '../model/workbench-terminal-location-activation';

/** The TUI names this many Tabs in a close question and counts the rest. */
const SHARED_TAB_CLOSE_NAMED_TABS = 3;

interface ResizableLayoutChangedMeta {
	readonly isUserInteraction: boolean;
}

interface ResizablePanelSize {
	readonly inPixels: number;
}

interface WorkbenchView {
	readonly agentNavigationAnnouncement: {
		readonly message: string;
		readonly sequence: number;
	} | null;
	readonly authority: WorkbenchAuthorityNoticeView['authority'];
	readonly authorityNotice: WorkbenchAuthorityNoticeView['notice'];
	readonly activateStandalone: (origin: WorkbenchTerminalLocationActivationOrigin) => void;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly actionScope: ActionScope;
	readonly cancelProjectWorkspaceConfirmation: () => void;
	readonly confirmProjectWorkspaceAction: () => void;
	/** Present when an authority decides the Panels, whose new terminals then show as it answers. */
	readonly createPanelTerminal: WorkbenchSharedPanelTerminalCreate | undefined;
	readonly createProject: (path: string) => ActionExecution<WorkbenchProjectCreationReceipt>;
	readonly activateCreatedProject: (
		receipt: WorkbenchProjectCreationReceipt,
	) => ActionExecution<void>;
	readonly isCreatedProjectActive: (receipt: WorkbenchProjectCreationReceipt) => boolean;
	readonly createWorkspace: (
		input?:
			| { readonly projectId: string }
			| {
					readonly resourceScopeId: string;
					readonly projectId: string | null;
					readonly directoryId: string;
			  },
	) => void;
	readonly setWorkspaceProject: (workspaceId: string, projectId: string | null) => void;
	readonly createWorkspaceProject: (workspaceId: string, name: string) => void;
	readonly deleteWorkspace: (workspaceId: string) => void;
	readonly isSidebarVisible: boolean;
	readonly leftPanelSize: string;
	readonly newTab: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly newTabChoices: readonly TabStripNewTabChoice[] | undefined;
	readonly onCloseTabs: TabStripCloseCallback;
	/** Present when an authority decides the Tabs, which then show only as it answers. */
	readonly onTabsChange: TabStripChangeCallback | undefined;
	readonly getCloseTabsConfirmation: TabStripCloseConfirmation;
	readonly getTabCreationAvailability: (intent: TabStripTabCreationIntent) => ActionAvailability;
	readonly onLayoutChanged: (
		layout: Readonly<Record<string, number>>,
		meta: ResizableLayoutChangedMeta,
	) => void;
	readonly onLeftPanelResize: (size: ResizablePanelSize) => void;
	readonly onRootBlurCapture: (event: React.FocusEvent<HTMLDivElement>) => void;
	readonly onRootFocusCapture: (event: React.FocusEvent<HTMLDivElement>) => void;
	readonly shortcutScopeReference: React.RefObject<HTMLDivElement | null>;
	readonly sidebarShortcutId: string;
	readonly sidebarShortcutLabel: string;
	readonly sidebarToggleLabel: string;
	readonly sidebarLabel: string;
	readonly projectLabel: string;
	readonly newWorkspaceLabel: string;
	readonly projectNavigationMessages: WorkbenchProjectNavigationMessages;
	readonly projectWorkspaceConfirmationCancelButtonRef: React.RefObject<HTMLButtonElement | null>;
	readonly projectWorkspaceConfirmation: {
		readonly cancelLabel: string;
		readonly confirmLabel: string;
		readonly description: string;
		readonly title: string;
	} | null;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly removeProject: (projectId: string) => void;
	readonly reorderProject: (projectId: string, beforeProjectId: string | null) => Promise<unknown>;
	readonly reorderWorkspace: (
		workspaceId: string,
		beforeWorkspaceId: string | null,
	) => Promise<unknown>;
	readonly renameProject: (projectId: string, name: string) => void;
	readonly searchLabel: string;
	readonly search: (returnFocus: HTMLElement) => void;
	readonly searchState: WorkbenchSearchState;
	readonly updateSearch: React.Dispatch<WorkbenchSearchEvent>;
	readonly searchShortcutId: string;
	readonly searchShortcutLabel: string;
	readonly settingsLabel: string;
	readonly settings: (() => void) | undefined;
	readonly settingsShortcutId: string;
	readonly settingsShortcutLabel: string;
	readonly sidebarFocusRequest: number;
	readonly setTabTitle: (tabId: string) => void;
	readonly tabTitleEditor:
		| (WorkbenchTabTitleEditorRequest & { readonly draftTitle: string })
		| null;
	readonly tabTitleEditorFinalFocus: () => HTMLElement | null;
	readonly tabTitleEditorMessages: {
		readonly blank: NameDialogBlankRule;
		readonly cancel: string;
		readonly close: string;
		readonly nameTooLong: string;
		/** The automatic name an unnamed shared Tab keeps until one is typed. */
		readonly placeholder: string | undefined;
		readonly submit: string;
		readonly title: string;
	};
	readonly cancelTabTitleEditor: () => void;
	readonly submitTabTitleEditor: () => void;
	readonly updateTabTitleEditor: (title: string) => void;
	readonly terminalContext: WorkbenchTerminalContext;
	readonly terminalAutoFocusAllowed: boolean;
	readonly terminalFocusRequest: WorkbenchTerminalFocusRequest | null;
	readonly tabModel: TabModel;
	readonly hasVisibleTerminalTabs: boolean;
	/** Only a shared Workbench says anything in place of a Workspace's missing Tabs. */
	readonly emptyWorkspace: WorkbenchEmptyWorkspace | null;
	readonly terminalLabel: string;
	readonly tabStripOrientation: TabStripOrientation;
	readonly tabStripFocusRequest: number;
	readonly onTabStripOrientationChange: (orientation: TabStripOrientation) => void;
	readonly verticalTabStripCollapsed: boolean;
	readonly onVerticalTabStripCollapsedChange: (collapsed: boolean) => void;
	readonly toggleSidebar: () => void;
	readonly toggleProjectExpansion: (projectId: string) => void;
	readonly toggleRecentsExpansion: () => void;
	readonly activateWorkspace: (
		workspaceId: string,
		origin: WorkbenchTerminalLocationActivationOrigin,
	) => void;
	readonly updateWorkspace: (workspaceId: string, name: string, primaryDirectoryId: string) => void;
	/** Present when the authority reaches Git for its Workspaces. */
	readonly worktreeCommands: WorkbenchWorktreeCommands | undefined;
}

interface UseWorkbenchOptions {
	readonly actionRuntime: ActionRuntime;
	readonly agentObservationSource?: WorkbenchAgentSource | undefined;
	readonly authority?: WorkbenchAuthority | undefined;
	readonly isShortcutScopeActive: boolean;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly onSettings: WorkbenchHostActionCallback | undefined;
	readonly onTerminalFontSizeChange?: WorkbenchTerminalFontSizeChangeCallback | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly remoteTerminalCreation?: WorkbenchRemoteTerminalCreationCapability | undefined;
	readonly shortcutBindings: WorkbenchShortcutBindings;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly storage: WorkbenchLayoutStorage | undefined;
	readonly terminalSurfaceAvailable: boolean;
}

export function useWorkbench({
	actionRuntime,
	agentObservationSource,
	authority,
	isShortcutScopeActive,
	navigation,
	onSettings,
	onTerminalFontSizeChange,
	projectWorkspaceRuntime,
	remoteTerminalCreation,
	shortcutBindings,
	shortcutPlatform,
	storage,
	terminalSurfaceAvailable,
}: UseWorkbenchOptions): WorkbenchView {
	const intl = useOrchbeeIntl();
	const authorityNotice = useWorkbenchAuthorityNotice(authority, projectWorkspaceRuntime);
	const [agentNavigationAnnouncementSequence, setAgentNavigationAnnouncementSequence] =
		React.useState(0);
	const pendingProjectWorkspaceConfirmation = React.useRef<((accepted: boolean) => void) | null>(
		null,
	);
	const projectWorkspaceConfirmationCancelButtonRef = React.useRef<HTMLButtonElement>(null);
	const [projectWorkspaceConfirmation, setProjectWorkspaceConfirmation] = React.useState<{
		readonly cancelLabel: string;
		readonly confirmLabel: string;
		readonly description: string;
		readonly title: string;
	} | null>(null);
	const projectWorkspaceState = React.useSyncExternalStore(
		projectWorkspaceRuntime.subscribe,
		projectWorkspaceRuntime.getSnapshot,
		projectWorkspaceRuntime.getSnapshot,
	);
	const terminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
	const tabModel = terminalContext.tabModel;
	const tabModelSnapshot = React.useSyncExternalStore(
		tabModel.subscribe,
		tabModel.getSnapshot,
		tabModel.getSnapshot,
	);
	const [isSidebarVisible, setIsSidebarVisible] = React.useState(() =>
		readWorkbenchSidebarVisibility(storage),
	);
	const resolveTerminalFocusTarget = useMemoizedFn(() =>
		terminalSurfaceAvailable ? resolveActiveTerminalPresentationId(projectWorkspaceRuntime) : null,
	);
	const focusRouting = useWorkbenchFocusRouting({
		isSidebarVisible,
		resolveTerminalTarget: resolveTerminalFocusTarget,
	});
	const focusRouter = React.useMemo(
		() => ({
			beginTerminalHandover: focusRouting.beginTerminalHandover,
			focus: focusRouting.focus,
			getResolution: focusRouting.getResolution,
		}),
		[focusRouting.beginTerminalHandover, focusRouting.focus, focusRouting.getResolution],
	);
	const [tabStripOrientation, setTabStripOrientationState] = React.useState<TabStripOrientation>(
		() => readWorkbenchTabStripOrientation(storage),
	);
	const [verticalTabStripCollapsed, setVerticalTabStripCollapsed] = React.useState(false);
	const [defaultLeftPanelWidth] = React.useState(() => readWorkbenchLeftPanelWidth(storage));
	const leftPanelWidth = React.useRef(defaultLeftPanelWidth);
	const [searchState, updateSearch] = React.useReducer(
		transitionWorkbenchSearch,
		undefined,
		createWorkbenchSearchState,
	);
	const searchHasOpened = React.useRef(false);
	const searchReturnFocus = React.useRef<HTMLElement | null>(null);
	const tabTitleReturnFocus = React.useRef<HTMLElement | null>(null);
	const [tabTitleEditor, setTabTitleEditor] = React.useState<
		(WorkbenchTabTitleEditorRequest & { readonly draftTitle: string }) | null
	>(null);
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
	const openTabTitleEditor = useMemoizedFn((request: WorkbenchTabTitleEditorRequest) => {
		tabTitleReturnFocus.current = findActiveTerminalFocusTarget(projectWorkspaceRuntime);
		setTabTitleEditor({ ...request, draftTitle: request.unnamed ? '' : request.title });
	});
	const tabTitleEditorFinalFocus = useMemoizedFn(() => {
		const capturedTarget = tabTitleReturnFocus.current;
		return capturedTarget?.isConnected === true
			? capturedTarget
			: findActiveTerminalFocusTarget(projectWorkspaceRuntime);
	});
	const toggleSidebar = useMemoizedFn(() => {
		setIsSidebarVisible((currentVisibility) => {
			const nextVisibility = !currentVisibility;
			writeWorkbenchSidebarVisibility(storage, nextVisibility);
			return nextVisibility;
		});
	});
	const setTabStripOrientation = useMemoizedFn((orientation: TabStripOrientation) => {
		setTabStripOrientationState(orientation);
		writeWorkbenchTabStripOrientation(storage, orientation);
	});
	const worktreeLabel = useMemoizedFn((ordinal: number) =>
		intl.formatMessage(workbenchMessages.worktreeDefaultLabel, { ordinal }),
	);
	const getNewTerminalAvailability = useMemoizedFn(
		(input: WorkbenchTerminalCreateInput): ActionAvailability => {
			if (authorityNotice.authority !== undefined) {
				return getSharedTerminalCreationAvailability(projectWorkspaceRuntime, input);
			}
			// Only an authority knows the saved launch profiles.
			return input.profile === undefined
				? getWorkbenchTerminalCreationAvailability(
						projectWorkspaceRuntime,
						remoteTerminalCreation,
						input,
					)
				: { kind: 'disabled', reason: workbenchMessages.actionUnavailable };
		},
	);
	const getTabCreationAvailability = useMemoizedFn((intent: TabStripTabCreationIntent) =>
		authorityNotice.authority === undefined
			? getWorkbenchTabCreationAvailability(projectWorkspaceRuntime, remoteTerminalCreation, intent)
			: getSharedTabCreationAvailability(projectWorkspaceRuntime, intent),
	);
	const createTerminal = useMemoizedFn<
		void,
		Parameters<WorkbenchTerminalCreateCallback>,
		ReturnType<WorkbenchTerminalCreateCallback>
	>((context, input) => {
		if (authorityNotice.authority !== undefined) {
			return createSharedTerminal(context, input, {
				authority: authorityNotice.authority,
				beginTerminalHandover: focusRouting.beginTerminalHandover,
				runtime: projectWorkspaceRuntime,
			});
		}
		// A chosen profile never falls back to the default program.
		if (input.profile !== undefined) return { code: 'launch-profile-unavailable', kind: 'blocked' };
		return executeWorkbenchTerminalCreationAction(context, input, {
			fallbackTitle: intl.formatMessage(workbenchTabMessages.terminalTitle),
			onLocalTerminalCreated: () => {
				focusRouting.requestTerminalFocus();
			},
			onRemoteTerminalCreated: () => {
				focusRouting.requestTerminalFocus();
			},
			remoteTerminalCreation,
			runtime: projectWorkspaceRuntime,
		});
	});
	const createPanelTerminal = useMemoizedFn<
		void,
		Parameters<WorkbenchSharedPanelTerminalCreate>,
		ReturnType<WorkbenchSharedPanelTerminalCreate>
	>((context, creation) => {
		const decider = authorityNotice.authority;
		return decider === undefined
			? Promise.resolve({ kind: 'unsupported' })
			: createSharedPanelTerminal(context, creation, {
					authority: decider,
					beginTerminalHandover: focusRouting.beginTerminalHandover,
					runtime: projectWorkspaceRuntime,
				});
	});
	const changeSharedTabs = useMemoizedFn<
		void,
		Parameters<TabStripChangeCallback>,
		ReturnType<TabStripChangeCallback>
	>((context, change) => {
		const decider = authorityNotice.authority;
		if (decider === undefined) return { kind: 'unsupported' };
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
			default:
				return decider.change(context, {
					change,
					terminalContextId: projectWorkspaceRuntime.getActiveTerminalContext().terminalContextId,
					type: 'tabs-changed',
				});
		}
	});
	/** In the order the strip shows them, which is the order a set of Tabs closes in. */
	const sharedTabsClosure = (tabIds: readonly string[]) => {
		const { tabModel: activeTabModel, terminalContextId } =
			projectWorkspaceRuntime.getActiveTerminalContext();
		const requested = new Set(tabIds);
		return {
			tabIds: activeTabModel
				.getSnapshot()
				.tabs.filter((tab) => requested.has(tab.id))
				.map((tab) => tab.id),
			terminalContextId,
			type: 'tabs-close' as const,
		};
	};
	const closeSharedTabs = async (
		context: Parameters<TabStripCloseCallback>[0],
		decider: WorkbenchAuthority,
		tabIds: readonly string[],
	): Promise<ActionResult<void>> => {
		const handover = focusRouting.beginTerminalHandover();
		const result = await decider.close(context, sharedTabsClosure(tabIds));
		if (result.kind === 'completed') handover.complete();
		return result;
	};
	const closeTabs = useMemoizedFn<
		void,
		Parameters<TabStripCloseCallback>,
		ReturnType<TabStripCloseCallback>
	>((context, request) => {
		if (context.signal.aborted) return { kind: 'cancelled' };
		if (authorityNotice.authority !== undefined) {
			return closeSharedTabs(context, authorityNotice.authority, request.tabIds);
		}
		const result = projectWorkspaceRuntime.requestActiveTerminalTabsClose(request.tabIds);
		if (result.kind === 'rejected') return { code: result.reason, kind: 'blocked' };
		if (result.kind === 'unchanged') return { kind: 'unchanged' };
		context.markCommitted();
		return { kind: 'completed' };
	});
	const getCloseTabsConfirmation = useMemoizedFn<
		void,
		Parameters<TabStripCloseConfirmation>,
		ReturnType<TabStripCloseConfirmation>
	>((request) => {
		const decider = authorityNotice.authority;
		const asks =
			projectWorkspaceRuntime.getActiveTerminalTabsCloseConfirmationCount(request.tabIds) > 0 ||
			// A shared set always asks, because one press ends more than the Tab under the pointer.
			(decider !== undefined && request.tabIds.length > 1);
		if (!asks) return null;
		const closure = sharedTabsClosure(request.tabIds);
		const { tabModel: activeTabModel } = projectWorkspaceRuntime.getActiveTerminalContext();
		const shown = closure.tabIds.slice(0, SHARED_TAB_CLOSE_NAMED_TABS);
		const titles = new Map(activeTabModel.getSnapshot().tabs.map((tab) => [tab.id, tab.title]));
		return {
			cancelLabel: workbenchMessages.panelCancel,
			confirmLabel: workbenchMessages.panelClose,
			description: {
				...workbenchMessages.tabCloseDescription,
				values: {
					more: closure.tabIds.length - shown.length,
					names: shown.map((tabId) => titles.get(tabId) ?? '').join(', '),
					warning: intl.formatMessage(
						workbenchMessages.terminalCloseWarning,
						terminalCloseWarningValues(
							projectWorkspaceRuntime.getActiveTerminalTabsCloseImpact(closure.tabIds),
						),
					),
				},
			},
			risk: 'destructive',
			title: { ...workbenchMessages.tabCloseTitle, values: { count: closure.tabIds.length } },
			watch:
				decider === undefined ? undefined : (withdraw) => decider.watchClose(closure, withdraw),
		};
	});
	const actions = useWorkbenchActions({
		agentObservationSource,
		authority: authorityNotice.authority,
		focusRouting: focusRouter,
		getNewTerminalAvailability,
		isSidebarVisible,
		navigation,
		onNewTerminal: createTerminal,
		onSettings,
		onTerminalFontSizeChange,
		openSearch,
		openTabTitleEditor,
		primaryWorkspaceName: intl.formatMessage(workbenchMessages.workspacePrimaryName),
		defaultWorkspaceName: intl.formatMessage(workbenchMessages.workspaceDefaultName),
		projectWorkspaceRuntime,
		projectWorkspaceState,
		runtime: actionRuntime,
		setTabStripOrientation,
		tabStripOrientation,
		toggleSidebar,
		workspaceCopySuffix: intl.formatMessage(workbenchMessages.workspaceCopySuffix),
		workspaceTerminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle),
		worktreeLabel,
	});
	const worktrees = authorityNotice.authority?.worktrees;
	const createTabWithProfile = useMemoizedFn((profile: UserLaunchProfile) => {
		actions.invoke(WORKBENCH_ACTION_IDS.createTerminal, { groupId: null, profile }, 'context-menu');
	});
	const newTabChoices = useWorkbenchNewTabChoices(
		authorityNotice.authority?.launchProfiles,
		projectWorkspaceState.activeLocation.kind === 'workspace',
		createTabWithProfile,
	);
	const invokeLocationActivation = useMemoizedFn(
		(targetLocation: WorkbenchActiveLocation, source: 'button' | 'pointer') =>
			targetLocation.kind === 'standalone'
				? actions.invoke(WORKBENCH_ACTION_IDS.activateStandalone, undefined, source)
				: actions.invoke(
						WORKBENCH_ACTION_IDS.activateWorkspace,
						{
							workspaceId: targetLocation.workspaceId,
						},
						source,
					),
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
	const activateStandalone = useMemoizedFn((origin: WorkbenchTerminalLocationActivationOrigin) =>
		activateLocation({ kind: 'standalone' }, origin),
	);
	const activateWorkspace = useMemoizedFn(
		(workspaceId: string, origin: WorkbenchTerminalLocationActivationOrigin) =>
			activateLocation({ kind: 'workspace', workspaceId }, origin),
	);
	const invokeAgentNavigation = useMemoizedFn((input: WorkbenchAgentNavigationInput) =>
		actions.invoke(WORKBENCH_ACTION_IDS.navigateAgent, input, 'button'),
	);
	const announceAgentNavigationUnavailable = useMemoizedFn(() => {
		setAgentNavigationAnnouncementSequence((sequence) => sequence + 1);
	});
	const navigateAgent = useWorkbenchAgentNavigation({
		activeLocation: projectWorkspaceState.activeLocation,
		announceUnavailable: announceAgentNavigationUnavailable,
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

	const shortcutScopeReference = useWorkbenchShortcuts(
		isShortcutScopeActive,
		shortcutPlatform,
		shortcutBindings,
		actions.invokeShortcut,
	);

	return {
		agentNavigationAnnouncement:
			agentNavigationAnnouncementSequence === 0
				? null
				: {
						message: intl.formatMessage(workbenchMessages.workspaceAgentNavigationUnavailable),
						sequence: agentNavigationAnnouncementSequence,
					},
		authority: authorityNotice.authority,
		authorityNotice: authorityNotice.notice,
		activateStandalone,
		navigateAgent,
		actionScope: actions.scope,
		activateWorkspace,
		cancelProjectWorkspaceConfirmation: () => pendingProjectWorkspaceConfirmation.current?.(false),
		confirmProjectWorkspaceAction: () => pendingProjectWorkspaceConfirmation.current?.(true),
		createProject: (path) => actions.invoke(WORKBENCH_ACTION_IDS.createProject, { path }, 'button'),
		activateCreatedProject: (receipt) =>
			actions.invoke(
				WORKBENCH_ACTION_IDS.activateWorkspace,
				{
					creationReceipt: receipt,

					workspaceId: receipt.workspaceId,
				},
				'button',
			),
		isCreatedProjectActive: (receipt) => {
			const location = projectWorkspaceRuntime.getSnapshot().activeLocation;
			return (
				location.kind === 'workspace' &&
				location.workspaceId === receipt.workspaceId &&
				(navigation === undefined || navigation.isActive?.(location) === true) &&
				matchesWorkbenchProjectCreation(
					projectWorkspaceRuntime.getSharedDocument(workbenchSharedDocumentRevision('0')),
					receipt,
				)
			);
		},
		setWorkspaceProject: (workspaceId, projectId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.setWorkspaceProject, { workspaceId, projectId }, 'menu');
		},
		createWorkspaceProject: (workspaceId, name) => {
			actions.invoke(WORKBENCH_ACTION_IDS.createWorkspaceProject, { workspaceId, name }, 'button');
		},
		createWorkspace: (input) => {
			actions.invoke(WORKBENCH_ACTION_IDS.createWorkspace, input, 'menu');
		},
		deleteWorkspace: (workspaceId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.deleteWorkspace, { workspaceId }, 'menu');
		},
		isSidebarVisible,
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
		newTabChoices,
		onCloseTabs: closeTabs,
		createPanelTerminal: authorityNotice.authority === undefined ? undefined : createPanelTerminal,
		onTabsChange: authorityNotice.authority === undefined ? undefined : changeSharedTabs,
		getCloseTabsConfirmation,
		getTabCreationAvailability,
		onLayoutChanged: (_layout, meta) => {
			if (meta.isUserInteraction) {
				writeWorkbenchLeftPanelWidth(storage, leftPanelWidth.current);
			}
		},
		onLeftPanelResize: (size) => {
			leftPanelWidth.current = size.inPixels;
		},
		onRootBlurCapture: focusRouting.onBlurCapture,
		onRootFocusCapture: focusRouting.onFocusCapture,
		shortcutScopeReference,
		sidebarShortcutId: WORKBENCH_SHORTCUTS.toggleSidebar.id,
		sidebarShortcutLabel: formatWorkbenchShortcut(
			'toggleSidebar',
			shortcutPlatform,
			shortcutBindings,
		),
		sidebarToggleLabel: intl.formatMessage(
			isSidebarVisible ? workbenchMessages.hideSidebar : workbenchMessages.showSidebar,
		),
		sidebarLabel: intl.formatMessage(workbenchMessages.sidebarLabel),
		projectLabel: intl.formatMessage(workbenchMessages.project),
		newWorkspaceLabel: intl.formatMessage(workbenchMessages.sidebarNewWorkspace),
		projectNavigationMessages: {
			workspaceMoveToProject: intl.formatMessage(workbenchMessages.workspaceMoveToProject),
			workspaceCreateProject: intl.formatMessage(workbenchMessages.workspaceCreateProject),
			workspaceUngroup: intl.formatMessage(workbenchMessages.workspaceUngroup),
			workspaceNew: intl.formatMessage(workbenchMessages.workspaceNew),

			cancel: intl.formatMessage(workbenchMessages.resourceCancel),
			close: intl.formatMessage(workbenchMessages.resourceClose),
			collapseProject: intl.formatMessage(workbenchMessages.projectCollapse),
			collapseRecents: intl.formatMessage(workbenchMessages.recentsCollapse),
			connectProject: intl.formatMessage(workbenchMessages.projectConnect),
			connectViaSsh: intl.formatMessage(workbenchMessages.projectConnectViaSsh),
			create: intl.formatMessage(workbenchMessages.resourceCreate),
			createWorkspace: intl.formatMessage(workbenchMessages.projectCreateWorkspace),
			disconnectProject: intl.formatMessage(workbenchMessages.projectDisconnect),
			expandProject: intl.formatMessage(workbenchMessages.projectExpand),
			expandRecents: intl.formatMessage(workbenchMessages.recentsExpand),
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
				'not-directory': intl.formatMessage(workbenchMessages.projectFolderAcquisitionNotDirectory),
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
			newFolder: intl.formatMessage(workbenchMessages.projectNewFolder),
			newFolderTitle: intl.formatMessage(workbenchMessages.projectNewFolderTitle),
			newFolderDescription: intl.formatMessage(workbenchMessages.projectNewFolderDescription),
			newFolderParent: intl.formatMessage(workbenchMessages.projectNewFolderParent),
			newFolderName: intl.formatMessage(workbenchMessages.projectNewFolderName),
			newFolderNameHint: intl.formatMessage(workbenchMessages.projectNewFolderNameHint),
			newFolderCreating: intl.formatMessage(workbenchMessages.projectNewFolderCreating),
			newFolderOpening: intl.formatMessage(workbenchMessages.projectNewFolderOpening),
			newFolderRetained: intl.formatMessage(workbenchMessages.projectNewFolderRetained),
			newFolderUnknown: intl.formatMessage(workbenchMessages.projectNewFolderUnknown),
			nameRequired: intl.formatMessage(workbenchMessages.nameRequired),
			nameTooLong: intl.formatMessage(workbenchMessages.nameTooLong),
			renameSubmit: intl.formatMessage(workbenchMessages.renameSubmit),
			projectFolders: intl.formatMessage(workbenchMessages.projectFolders),
			projectCreate: intl.formatMessage(workbenchMessages.projectCreate),
			projectCreateDescription: intl.formatMessage(workbenchMessages.projectCreateDescription),
			projectCreatePath: intl.formatMessage(workbenchMessages.projectCreatePath),
			projectCreatePathDescription: intl.formatMessage(
				workbenchMessages.projectCreatePathDescription,
			),
			projectCreatePathPlaceholder: intl.formatMessage(
				workbenchMessages.projectCreatePathPlaceholder,
			),
			projectCreateTitle: intl.formatMessage(workbenchMessages.projectCreateTitle),
			projectConnectionStatus: {
				blocked: intl.formatMessage(workbenchMessages.projectConnectionBlocked),
				connected: intl.formatMessage(workbenchMessages.projectConnectionConnected),
				connecting: intl.formatMessage(workbenchMessages.projectConnectionConnecting),
				disconnected: intl.formatMessage(workbenchMessages.projectConnectionDisconnected),
				'interaction-required': intl.formatMessage(
					workbenchMessages.projectConnectionInteractionRequired,
				),
				retrying: intl.formatMessage(workbenchMessages.projectConnectionRetrying),
				unavailable: intl.formatMessage(workbenchMessages.projectConnectionUnavailable),
			},
			projectMenuDissolve: intl.formatMessage(workbenchMessages.projectMenuDissolve),
			projectMenuRename: intl.formatMessage(workbenchMessages.projectMenuRename),
			projectOptions: intl.formatMessage(workbenchMessages.projectOptions),
			projectRenameTitle: intl.formatMessage(workbenchMessages.projectRenameTitle),
			projectWorktrees: intl.formatMessage(workbenchMessages.projectWorktrees),
			recents: intl.formatMessage(workbenchMessages.recents),
			retryProjectConnection: intl.formatMessage(workbenchMessages.projectRetryConnection),
			selectFolder: intl.formatMessage(workbenchMessages.projectSelectFolder),
			workspaceMenuClose: intl.formatMessage(workbenchMessages.workspaceMenuClose),
			workspaceMenuNewProject: intl.formatMessage(workbenchMessages.workspaceMenuNewProject),
			workspaceMenuRename: intl.formatMessage(workbenchMessages.workspaceMenuRename),
			workspaceOptions: intl.formatMessage(workbenchMessages.workspaceOptions),
			workspaceRenameTitle: intl.formatMessage(workbenchMessages.workspaceRenameTitle),
		},
		projectWorkspaceConfirmationCancelButtonRef,
		projectWorkspaceConfirmation,
		projectWorkspaceState,
		removeProject: (projectId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.removeProject, { projectId }, 'menu');
		},
		reorderProject: (projectId, beforeProjectId) =>
			actions.invoke(WORKBENCH_ACTION_IDS.reorderProject, { beforeProjectId, projectId }, 'pointer')
				.completion,
		reorderWorkspace: (workspaceId, beforeWorkspaceId) =>
			actions.invoke(
				WORKBENCH_ACTION_IDS.reorderWorkspace,
				{ beforeWorkspaceId, workspaceId },
				'pointer',
			).completion,
		renameProject: (projectId, name) => {
			actions.invoke(WORKBENCH_ACTION_IDS.renameProject, { name, projectId }, 'button');
		},
		search: (returnFocus) => {
			searchReturnFocus.current = returnFocus;
			actions.invoke(WORKBENCH_ACTION_IDS.openSearch, { filter: 'all' }, 'button');
		},
		searchState,
		searchLabel: intl.formatMessage(workbenchMessages.search),
		searchShortcutId: WORKBENCH_SHORTCUTS.search.id,
		searchShortcutLabel: formatWorkbenchShortcut('search', shortcutPlatform, shortcutBindings),
		settings:
			onSettings === undefined
				? undefined
				: () => {
						actions.invoke(WORKBENCH_ACTION_IDS.openSettings, undefined, 'button');
					},
		settingsLabel: intl.formatMessage(workbenchMessages.settings),
		settingsShortcutId: WORKBENCH_SHORTCUTS.settings.id,
		settingsShortcutLabel: formatWorkbenchShortcut('settings', shortcutPlatform, shortcutBindings),
		sidebarFocusRequest: focusRouting.sidebarFocusRequest,
		setTabTitle: (tabId) => {
			const activeTerminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
			actions.invoke(
				WORKBENCH_ACTION_IDS.setTabTitle,
				{ kind: 'open', tabId, terminalContextId: activeTerminalContext.terminalContextId },
				'context-menu',
			);
		},
		tabTitleEditor,
		tabTitleEditorFinalFocus,
		tabTitleEditorMessages: {
			// A shared name has no way back to following the terminal, so a blank one is refused.
			blank:
				authorityNotice.authority === undefined
					? {
							help: intl.formatMessage(workbenchTabMessages.tabTitleRestoreDescription),
							kind: 'accept',
						}
					: { kind: 'refuse', reason: intl.formatMessage(workbenchMessages.nameRequired) },
			cancel: intl.formatMessage(workbenchMessages.resourceCancel),
			close: intl.formatMessage(workbenchMessages.resourceClose),
			nameTooLong: intl.formatMessage(workbenchMessages.nameTooLong),
			placeholder: tabTitleEditor?.unnamed === true ? tabTitleEditor.title : undefined,
			submit: intl.formatMessage(workbenchMessages.renameSubmit),
			title: intl.formatMessage(workbenchTabMessages.renameTab),
		},
		cancelTabTitleEditor: () => setTabTitleEditor(null),
		submitTabTitleEditor: () => {
			if (tabTitleEditor === null) return;
			actions.invoke(
				WORKBENCH_ACTION_IDS.setTabTitle,
				{
					kind: 'commit',
					tabId: tabTitleEditor.tabId,
					terminalContextId: tabTitleEditor.terminalContextId,
					title: tabTitleEditor.draftTitle,
				},
				'button',
			);
			setTabTitleEditor(null);
		},
		updateTabTitleEditor: (title) => {
			setTabTitleEditor((current) => (current === null ? null : { ...current, draftTitle: title }));
		},
		terminalContext,
		terminalAutoFocusAllowed: focusRouting.terminalAutoFocusAllowed,
		terminalFocusRequest: focusRouting.terminalFocusRequest,
		tabModel,
		hasVisibleTerminalTabs: tabModelSnapshot.tabs.length > 0,
		emptyWorkspace:
			authorityNotice.authority === undefined || tabModelSnapshot.tabs.length > 0
				? null
				: projectWorkbenchEmptyWorkspace(projectWorkspaceState, intl),
		terminalLabel: intl.formatMessage(workbenchMessages.terminal),
		tabStripOrientation,
		tabStripFocusRequest: focusRouting.tabStripFocusRequest,
		onTabStripOrientationChange: (orientation) => {
			actions.invoke(WORKBENCH_ACTION_IDS.setTabStripOrientation, { orientation }, 'context-menu');
		},
		verticalTabStripCollapsed,
		onVerticalTabStripCollapsedChange: setVerticalTabStripCollapsed,
		toggleSidebar: () => {
			actions.invoke(WORKBENCH_ACTION_IDS.toggleSidebar, undefined, 'button');
		},
		toggleProjectExpansion: (projectId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.toggleProjectExpansion, { projectId }, 'button');
		},
		toggleRecentsExpansion: () => {
			actions.invoke(WORKBENCH_ACTION_IDS.toggleRecentsExpansion, undefined, 'button');
		},
		updateWorkspace: (workspaceId, name, primaryDirectoryId) => {
			actions.invoke(
				WORKBENCH_ACTION_IDS.updateWorkspace,
				{ name, primaryDirectoryId, workspaceId },
				'button',
			);
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
