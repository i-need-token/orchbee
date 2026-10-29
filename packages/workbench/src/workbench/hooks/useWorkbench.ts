import { useOrchbeeIntl, workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import type {
	ActionConfirmation,
	ActionAvailability,
	ActionExecution,
	ActionRuntime,
	ActionScope,
} from '@orchbee/actions';
import type { AgentSnapshotSource } from '@orchbee/agent';
import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	type TabModel,
	type TabStripCloseCallback,
	type TabStripCloseConfirmation,
	type TabStripNewTabRequest,
	type TabStripOrientation,
	type TabStripTabCreationIntent,
} from '@orchbee/tab-strip';
import * as React from 'react';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench-client-protocol';

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
import type {
	WorkbenchHostActionCallback,
	WorkbenchNavigationPort,
	WorkbenchTerminalCreateCallback,
	WorkbenchTerminalFontSizeChangeCallback,
} from '../types/workbench-host-actions';
import type { WorkbenchSearchFilter } from '../types/workbench-search';
import type { WorkbenchProjectNavigationMessages } from '../components/workbench-sidebar/types/workbench-sidebar-types';
import type { WorkbenchRemoteTerminalCreationCapability } from '../types/workbench-remote-terminal-creation';
import type { WorkbenchProjectCreationReceipt } from '../types/workbench-project-folder-acquisition';
import { matchesWorkbenchProjectCreation } from '../model/matchesWorkbenchProjectCreation';
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
import { useWorkbenchActions } from './useWorkbenchActions';
import { useWorkbenchFocusRouting } from './useWorkbenchFocusRouting';
import type { WorkbenchTerminalFocusRequest } from '../model/resolveWorkbenchFocusTarget';
import { useWorkbenchShortcuts } from './useWorkbenchShortcuts';
import { useWorkbenchAgentNavigation } from './useWorkbenchAgentNavigation';
import { useWorkbenchTerminalLocationActivation } from './useWorkbenchTerminalLocationActivation';
import type { WorkbenchAgentNavigationInput } from '../model/project-workbench-agent-overview';
import {
	workbenchActiveLocationsEqual,
	type WorkbenchTerminalLocationActivationOrigin,
} from '../model/workbench-terminal-location-activation';

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
	readonly activateStandalone: (origin: WorkbenchTerminalLocationActivationOrigin) => void;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly actionScope: ActionScope;
	readonly addResourceDirectory: (resourceScopeId: string, path: string) => void;
	readonly cancelProjectWorkspaceConfirmation: () => void;
	readonly confirmProjectWorkspaceAction: () => void;
	readonly createProject: (path: string) => ActionExecution<WorkbenchProjectCreationReceipt>;
	readonly activateCreatedProject: (
		receipt: WorkbenchProjectCreationReceipt,
	) => ActionExecution<void>;
	readonly isCreatedProjectActive: (receipt: WorkbenchProjectCreationReceipt) => boolean;
	readonly createWorkspace: (input?: {
		readonly resourceScopeId: string;
		readonly projectId: string | null;
		readonly directoryId: string;
	}) => void;
	readonly setWorkspaceProject: (workspaceId: string, projectId: string | null) => void;
	readonly createWorkspaceProject: (workspaceId: string, name: string) => void;
	readonly closeProjectWorkspaces: (projectId: string) => void;
	readonly deleteWorkspace: (workspaceId: string) => void;
	readonly duplicateWorkspace: (workspaceId: string) => void;
	readonly isSidebarVisible: boolean;
	readonly leftPanelSize: string;
	readonly newTab: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly onCloseTabs: TabStripCloseCallback;
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
	readonly removeResourceDirectory: (resourceScopeId: string, directoryId: string) => void;
	readonly reorderProject: (projectId: string, beforeProjectId: string | null) => void;
	readonly reorderWorkspace: (workspaceId: string, beforeWorkspaceId: string | null) => void;
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
		readonly cancel: string;
		readonly close: string;
		readonly description: string;
		readonly field: string;
		readonly placeholder: string;
		readonly restoreDescription: string;
		readonly save: string;
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
	readonly terminalLabel: string;
	readonly tabStripOrientation: TabStripOrientation;
	readonly tabStripFocusRequest: number;
	readonly onTabStripOrientationChange: (orientation: TabStripOrientation) => void;
	readonly verticalTabStripCollapsed: boolean;
	readonly onVerticalTabStripCollapsedChange: (collapsed: boolean) => void;
	readonly toggleSidebar: () => void;
	readonly toggleProjectExpansion: (projectId: string) => void;
	readonly activateWorkspace: (
		workspaceId: string,
		origin: WorkbenchTerminalLocationActivationOrigin,
	) => void;
	readonly setResourcePrimaryDirectory: (resourceScopeId: string, directoryId: string) => void;
	readonly updateWorkspace: (workspaceId: string, name: string, primaryDirectoryId: string) => void;
}

interface UseWorkbenchOptions {
	readonly actionRuntime: ActionRuntime;
	readonly agentObservationSource?: AgentSnapshotSource | undefined;
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
		() => ({ focus: focusRouting.focus, getResolution: focusRouting.getResolution }),
		[focusRouting.focus, focusRouting.getResolution],
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
		setTabTitleEditor({ ...request, draftTitle: request.title });
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
	const getNewTerminalAvailability = useMemoizedFn((input: WorkbenchTerminalCreateInput) =>
		getWorkbenchTerminalCreationAvailability(
			projectWorkspaceRuntime,
			remoteTerminalCreation,
			input,
		),
	);
	const getTabCreationAvailability = useMemoizedFn((intent: TabStripTabCreationIntent) =>
		getWorkbenchTabCreationAvailability(projectWorkspaceRuntime, remoteTerminalCreation, intent),
	);
	const createTerminal = useMemoizedFn<
		void,
		Parameters<WorkbenchTerminalCreateCallback>,
		ReturnType<WorkbenchTerminalCreateCallback>
	>((context, input) =>
		executeWorkbenchTerminalCreationAction(context, input, {
			fallbackTitle: intl.formatMessage(workbenchTabMessages.terminalTitle),
			onLocalTerminalCreated: () => {
				focusRouting.requestTerminalFocus();
			},
			onRemoteTerminalCreated: () => {
				focusRouting.requestTerminalFocus();
			},
			remoteTerminalCreation,
			runtime: projectWorkspaceRuntime,
		}),
	);
	const closeTabs = useMemoizedFn<
		void,
		Parameters<TabStripCloseCallback>,
		ReturnType<TabStripCloseCallback>
	>((context, request) => {
		if (context.signal.aborted) return { kind: 'cancelled' };
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
		const count = projectWorkspaceRuntime.getActiveTerminalTabsCloseConfirmationCount(
			request.tabIds,
		);
		return count === 0
			? null
			: {
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.panelCloseAnyway,
					description: {
						...workbenchMessages.tabCloseRunningDescription,
						values: { count },
					},
					risk: 'destructive',
					title: workbenchMessages.panelCloseRunningTitle,
				};
	});
	const actions = useWorkbenchActions({
		agentObservationSource,
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
	});
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
		activateStandalone,
		navigateAgent,
		actionScope: actions.scope,
		addResourceDirectory: (resourceScopeId, path) => {
			actions.invoke(WORKBENCH_ACTION_IDS.addResourceFolder, { path, resourceScopeId }, 'button');
		},
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
		closeProjectWorkspaces: (projectId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.closeProjectWorkspaces, { projectId }, 'menu');
		},
		createWorkspace: (input) => {
			actions.invoke(WORKBENCH_ACTION_IDS.createWorkspace, input, 'menu');
		},
		deleteWorkspace: (workspaceId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.deleteWorkspace, { workspaceId }, 'menu');
		},
		duplicateWorkspace: (workspaceId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.duplicateWorkspace, { workspaceId }, 'menu');
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
		onCloseTabs: closeTabs,
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
			projectCloseAll: intl.formatMessage(workbenchMessages.projectCloseAll),
			workspaceNew: intl.formatMessage(workbenchMessages.workspaceNew),

			add: intl.formatMessage(workbenchMessages.resourceAdd),
			cancel: intl.formatMessage(workbenchMessages.resourceCancel),
			close: intl.formatMessage(workbenchMessages.resourceClose),
			collapseProject: intl.formatMessage(workbenchMessages.projectCollapse),
			connectProject: intl.formatMessage(workbenchMessages.projectConnect),
			connectViaSsh: intl.formatMessage(workbenchMessages.projectConnectViaSsh),
			create: intl.formatMessage(workbenchMessages.resourceCreate),
			createWorkspace: intl.formatMessage(workbenchMessages.projectCreateWorkspace),
			deleteWorkspace: intl.formatMessage(workbenchMessages.workspaceDelete),
			disconnectProject: intl.formatMessage(workbenchMessages.projectDisconnect),
			duplicateWorkspace: intl.formatMessage(workbenchMessages.workspaceDuplicate),
			expandProject: intl.formatMessage(workbenchMessages.projectExpand),
			folderAcquisitionFailureDismiss: intl.formatMessage(
				workbenchMessages.projectFolderAcquisitionDismiss,
			),
			folderAcquisitionFailureMessages: {
				'already-exists': intl.formatMessage(
					workbenchMessages.projectFolderAcquisitionAlreadyExists,
				),
				'invalid-name': intl.formatMessage(workbenchMessages.projectFolderAcquisitionInvalidName),
				'stale-parent': intl.formatMessage(workbenchMessages.projectFolderAcquisitionStaleParent),
				'already-added': intl.formatMessage(workbenchMessages.projectFolderAcquisitionAlreadyAdded),
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
				'project-unavailable': intl.formatMessage(
					workbenchMessages.projectFolderAcquisitionProjectUnavailable,
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
			newWorktree: intl.formatMessage(workbenchMessages.projectNewWorktree),
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
			projectManage: intl.formatMessage(workbenchMessages.projectManage),
			projectManageAddFolder: intl.formatMessage(workbenchMessages.projectAddFolder),
			projectManageAddLocalFolder: intl.formatMessage(
				workbenchMessages.projectManageAddLocalFolder,
			),
			projectManageAddSshFolder: intl.formatMessage(workbenchMessages.projectManageAddSshFolder),
			projectManageDescription: intl.formatMessage(workbenchMessages.projectManageDescription),
			projectManageFolderAlreadyAdded: intl.formatMessage(
				workbenchMessages.projectManageFolderAlreadyAdded,
			),
			projectManageFolders: intl.formatMessage(workbenchMessages.projectManageFolders),
			projectManageName: intl.formatMessage(workbenchMessages.projectManageName),
			projectManageNamePlaceholder: intl.formatMessage(
				workbenchMessages.projectManageNamePlaceholder,
			),
			projectManagePrimaryFolder: intl.formatMessage(workbenchMessages.projectManagePrimaryFolder),
			projectManageSshFolderBadge: intl.formatMessage(
				workbenchMessages.projectManageSshFolderBadge,
			),
			projectManageSshFolderDescription: intl.formatMessage(
				workbenchMessages.projectManageSshFolderDescription,
			),
			projectManageTitle: intl.formatMessage(workbenchMessages.projectManageTitle),
			projectOptions: intl.formatMessage(workbenchMessages.projectOptions),
			projectWorktrees: intl.formatMessage(workbenchMessages.projectWorktrees),
			projectWorktreeBase: intl.formatMessage(workbenchMessages.projectWorktreeBase),
			projectWorktreeBaseNoMatches: intl.formatMessage(
				workbenchMessages.projectWorktreeBaseNoMatches,
			),
			projectWorktreeBranch: intl.formatMessage(workbenchMessages.projectWorktreeBranch),
			projectWorktreeCreateWorkspace: intl.formatMessage(
				workbenchMessages.projectWorktreeCreateWorkspace,
			),
			projectWorktreeDescription: intl.formatMessage(workbenchMessages.projectWorktreeDescription),
			projectWorktreeFailureMessages: {
				'base-not-found': intl.formatMessage(workbenchMessages.projectWorktreeFailureBaseNotFound),
				'branch-already-exists': intl.formatMessage(
					workbenchMessages.projectWorktreeFailureBranchAlreadyExists,
				),
				busy: intl.formatMessage(workbenchMessages.projectWorktreeFailureGeneric),
				'compensation-failed': intl.formatMessage(
					workbenchMessages.projectWorktreeFailureCompensationFailed,
				),
				'control-unavailable': intl.formatMessage(workbenchMessages.projectWorktreeFailureGeneric),
				'creation-failed': intl.formatMessage(workbenchMessages.projectWorktreeFailureGeneric),
				'discovery-failed': intl.formatMessage(workbenchMessages.projectWorktreeFailureGeneric),
				'document-conflict': intl.formatMessage(workbenchMessages.projectWorktreeFailureGeneric),
				'git-unavailable': intl.formatMessage(workbenchMessages.projectWorktreeFailureGeneric),
				'invalid-branch': intl.formatMessage(workbenchMessages.projectWorktreeFailureInvalidBranch),
				'project-not-found': intl.formatMessage(workbenchMessages.projectWorktreeFailureGeneric),
				'recovery-required': intl.formatMessage(
					workbenchMessages.projectWorktreeFailureRecoveryRequired,
				),
				'repository-not-found': intl.formatMessage(workbenchMessages.projectWorktreeFailureGeneric),
				'source-unavailable': intl.formatMessage(workbenchMessages.projectWorktreeFailureGeneric),
				'target-conflict': intl.formatMessage(workbenchMessages.projectWorktreeFailureGeneric),
				'worktree-retained': intl.formatMessage(workbenchMessages.projectWorktreeFailureRetained),
				'worktree-not-found': intl.formatMessage(workbenchMessages.projectWorktreeFailureGeneric),
			},
			projectWorktreeFailureTitle: intl.formatMessage(
				workbenchMessages.projectWorktreeFailureTitle,
			),
			projectWorktreeInventoryUnavailable: intl.formatMessage(
				workbenchMessages.projectWorktreeInventoryUnavailable,
			),
			projectWorktreeLoading: intl.formatMessage(workbenchMessages.projectWorktreeLoading),
			projectWorktreeLocation: intl.formatMessage(workbenchMessages.projectWorktreeLocation),
			projectWorktreeLocationAutomatic: (root) =>
				intl.formatMessage(workbenchMessages.projectWorktreeLocationAutomatic, { root }),
			projectWorktreeNoRepositories: intl.formatMessage(
				workbenchMessages.projectWorktreeNoRepositories,
			),
			projectWorktreeRepository: intl.formatMessage(workbenchMessages.projectWorktreeRepository),
			projectWorktreeTitle: intl.formatMessage(workbenchMessages.projectWorktreeTitle),
			removeProject: intl.formatMessage(workbenchMessages.projectRemove),
			retryProjectConnection: intl.formatMessage(workbenchMessages.projectRetryConnection),
			removeProjectFolder: intl.formatMessage(workbenchMessages.projectRemoveFolder),
			save: intl.formatMessage(workbenchMessages.resourceSave),
			selectFolder: intl.formatMessage(workbenchMessages.projectSelectFolder),
			setProjectPrimaryFolder: intl.formatMessage(workbenchMessages.projectSetPrimaryFolder),
			workspaceManage: intl.formatMessage(workbenchMessages.workspaceManage),
			workspaceManageDescription: intl.formatMessage(workbenchMessages.workspaceManageDescription),
			workspaceManageName: intl.formatMessage(workbenchMessages.workspaceManageName),
			workspaceManageNamePlaceholder: intl.formatMessage(
				workbenchMessages.workspaceManageNamePlaceholder,
			),
			workspaceManagePrimaryFolder: intl.formatMessage(
				workbenchMessages.workspaceManagePrimaryFolder,
			),
			workspaceManageTitle: intl.formatMessage(workbenchMessages.workspaceManageTitle),
			workspaceOptions: intl.formatMessage(workbenchMessages.workspaceOptions),
		},
		projectWorkspaceConfirmationCancelButtonRef,
		projectWorkspaceConfirmation,
		projectWorkspaceState,
		removeProject: (projectId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.removeProject, { projectId }, 'menu');
		},
		removeResourceDirectory: (resourceScopeId, directoryId) => {
			actions.invoke(
				WORKBENCH_ACTION_IDS.removeResourceDirectory,
				{ directoryId, resourceScopeId },
				'button',
			);
		},
		reorderProject: (projectId, beforeProjectId) => {
			actions.invoke(
				WORKBENCH_ACTION_IDS.reorderProject,
				{ beforeProjectId, projectId },
				'pointer',
			);
		},
		reorderWorkspace: (workspaceId, beforeWorkspaceId) => {
			actions.invoke(
				WORKBENCH_ACTION_IDS.reorderWorkspace,
				{ beforeWorkspaceId, workspaceId },
				'pointer',
			);
		},
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
			cancel: intl.formatMessage(workbenchMessages.resourceCancel),
			close: intl.formatMessage(workbenchMessages.resourceClose),
			description: intl.formatMessage(workbenchTabMessages.tabTitleDescription),
			field: intl.formatMessage(workbenchTabMessages.tabTitleField),
			placeholder: intl.formatMessage(workbenchTabMessages.tabTitlePlaceholder),
			restoreDescription: intl.formatMessage(workbenchTabMessages.tabTitleRestoreDescription),
			save: intl.formatMessage(workbenchMessages.resourceSave),
			title: intl.formatMessage(workbenchTabMessages.setTabTitle),
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
		setResourcePrimaryDirectory: (resourceScopeId, directoryId) => {
			actions.invoke(
				WORKBENCH_ACTION_IDS.setResourcePrimaryDirectory,
				{ directoryId, resourceScopeId },
				'button',
			);
		},
		updateWorkspace: (workspaceId, name, primaryDirectoryId) => {
			actions.invoke(
				WORKBENCH_ACTION_IDS.updateWorkspace,
				{ name, primaryDirectoryId, workspaceId },
				'button',
			);
		},
		updateSearch,
	};
}
