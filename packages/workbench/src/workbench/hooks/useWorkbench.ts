import { useOrchbeeIntl, workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import type {
	ActionConfirmation,
	ActionExecution,
	ActionRuntime,
	ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	ScopedTabModel,
	type TabModel,
	type TabStripCloseCallback,
	type TabStripCloseConfirmation,
	type TabStripNewTabRequest,
	type TabStripOrientation,
} from '@orchbee/tab-strip';
import * as React from 'react';

import {
	readWorkbenchLeftPanelWidth,
	readWorkbenchSidebarVisibility,
	readWorkbenchTabStripOrientation,
	writeWorkbenchLeftPanelWidth,
	writeWorkbenchSidebarVisibility,
	writeWorkbenchTabStripOrientation,
	type WorkbenchLayoutStorage,
} from '../../workbench-layout-storage';
import type { ProjectWorkspaceState } from '../../project-workspace';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import type {
	WorkbenchHostActionCallback,
	WorkbenchNavigationPort,
	WorkbenchTerminalCreateCallback,
	WorkbenchTerminalFontSizeChangeCallback,
} from '../types/workbench-host-actions';
import type { WorkbenchSearchFilter } from '../types/workbench-search';
import type { WorkbenchTabScopeSource } from '../types/workbench-tab-scope';
import type { WorkbenchProjectNavigationMessages } from '../components/workbench-sidebar/types/workbench-sidebar-types';
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
import { WORKBENCH_ACTION_IDS } from '../actions/workbench-action-ids';
import type { WorkbenchTabTitleEditorRequest } from '../actions/registerWorkbenchTabTitleAction';
import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalContext,
} from '../model/WorkbenchProjectWorkspaceRuntime';
import { findActiveTerminalFocusTarget } from '../utils/findActiveTerminalFocusTarget';
import { useWorkbenchActions } from './useWorkbenchActions';
import { useWorkbenchShortcuts } from './useWorkbenchShortcuts';

interface ResizableLayoutChangedMeta {
	readonly isUserInteraction: boolean;
}

interface ResizablePanelSize {
	readonly inPixels: number;
}

interface WorkbenchView {
	readonly activateStandalone: () => void;
	readonly actionScope: ActionScope;
	readonly addProjectDirectory: (projectId: string, path: string) => void;
	readonly cancelProjectWorkspaceConfirmation: () => void;
	readonly confirmProjectWorkspaceAction: () => void;
	readonly createProject: (path: string) => void;
	readonly createWorkspace: (projectId: string, directoryId: string) => void;
	readonly deleteWorkspace: (projectId: string, workspaceId: string) => void;
	readonly duplicateWorkspace: (projectId: string, workspaceId: string) => void;
	readonly isSidebarVisible: boolean;
	readonly leftPanelSize: string;
	readonly newTerminal: (() => void) | undefined;
	readonly newTab: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly onCloseTabs: TabStripCloseCallback;
	readonly getCloseTabsConfirmation: TabStripCloseConfirmation;
	readonly newTerminalLabel: string;
	readonly newTerminalShortcutId: string;
	readonly newTerminalShortcutLabel: string;
	readonly onLayoutChanged: (
		layout: Readonly<Record<string, number>>,
		meta: ResizableLayoutChangedMeta,
	) => void;
	readonly onLeftPanelResize: (size: ResizablePanelSize) => void;
	readonly shortcutScopeReference: React.RefObject<HTMLDivElement | null>;
	readonly sidebarShortcutId: string;
	readonly sidebarShortcutLabel: string;
	readonly sidebarToggleLabel: string;
	readonly sidebarLabel: string;
	readonly projectLabel: string;
	readonly projectNavigationMessages: WorkbenchProjectNavigationMessages;
	readonly projectWorkspaceConfirmationConfirmButtonRef: React.RefObject<HTMLButtonElement | null>;
	readonly projectWorkspaceConfirmation: {
		readonly cancelLabel: string;
		readonly confirmLabel: string;
		readonly description: string;
		readonly title: string;
	} | null;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly removeProject: (projectId: string) => void;
	readonly removeProjectDirectory: (projectId: string, directoryId: string) => void;
	readonly reorderProject: (projectId: string, beforeProjectId: string | null) => void;
	readonly reorderWorkspace: (
		projectId: string,
		workspaceId: string,
		beforeWorkspaceId: string | null,
	) => void;
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
	readonly tabModel: TabModel;
	readonly hasVisibleTerminalTabs: boolean;
	readonly terminalLabel: string;
	readonly tabStripOrientation: TabStripOrientation;
	readonly onTabStripOrientationChange: (orientation: TabStripOrientation) => void;
	readonly verticalTabStripCollapsed: boolean;
	readonly onVerticalTabStripCollapsedChange: (collapsed: boolean) => void;
	readonly toggleSidebar: () => void;
	readonly toggleProjectExpansion: (projectId: string) => void;
	readonly activateWorkspace: (projectId: string, workspaceId: string) => void;
	readonly setProjectPrimaryDirectory: (projectId: string, directoryId: string) => void;
	readonly updateWorkspace: (
		projectId: string,
		workspaceId: string,
		name: string,
		primaryDirectoryId: string,
	) => void;
}

interface UseWorkbenchOptions {
	readonly actionRuntime: ActionRuntime;
	readonly isShortcutScopeActive: boolean;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly onSettings: WorkbenchHostActionCallback | undefined;
	readonly onTerminalFontSizeChange?: WorkbenchTerminalFontSizeChangeCallback | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly shortcutBindings: WorkbenchShortcutBindings;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly storage: WorkbenchLayoutStorage | undefined;
	readonly tabScope?: WorkbenchTabScopeSource | undefined;
}

export function useWorkbench({
	actionRuntime,
	isShortcutScopeActive,
	navigation,
	onSettings,
	onTerminalFontSizeChange,
	projectWorkspaceRuntime,
	shortcutBindings,
	shortcutPlatform,
	storage,
	tabScope,
}: UseWorkbenchOptions): WorkbenchView {
	const intl = useOrchbeeIntl();
	const pendingProjectWorkspaceConfirmation = React.useRef<((accepted: boolean) => void) | null>(
		null,
	);
	const projectWorkspaceConfirmationConfirmButtonRef = React.useRef<HTMLButtonElement>(null);
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
	const scopedTabModel = React.useMemo(
		() =>
			tabScope === undefined
				? null
				: new ScopedTabModel(terminalContext.tabModel, {
						visibleTabIds:
							tabScope
								.getSnapshot()
								.terminalContexts.find(
									(context) => context.terminalContextId === terminalContext.terminalContextId,
								)?.tabIds ?? [],
					}),
		[tabScope, terminalContext.tabModel, terminalContext.terminalContextId],
	);
	React.useEffect(() => {
		if (tabScope === undefined || scopedTabModel === null) return undefined;
		const synchronize = () => {
			scopedTabModel.setVisibleTabIds(
				tabScope
					.getSnapshot()
					.terminalContexts.find(
						(context) => context.terminalContextId === terminalContext.terminalContextId,
					)?.tabIds ?? [],
			);
		};
		synchronize();
		return tabScope.subscribe(synchronize);
	}, [scopedTabModel, tabScope, terminalContext.terminalContextId]);
	React.useEffect(() => () => scopedTabModel?.dispose(), [scopedTabModel]);
	const tabModel = scopedTabModel ?? terminalContext.tabModel;
	const tabModelSnapshot = React.useSyncExternalStore(
		tabModel.subscribe,
		tabModel.getSnapshot,
		tabModel.getSnapshot,
	);
	const [isSidebarVisible, setIsSidebarVisible] = React.useState(() =>
		readWorkbenchSidebarVisibility(storage),
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
	const createTerminal = useMemoizedFn<
		void,
		Parameters<WorkbenchTerminalCreateCallback>,
		ReturnType<WorkbenchTerminalCreateCallback>
	>((context, input) => {
		if (context.signal.aborted) return { kind: 'cancelled' };
		const cwdResult =
			input.currentDirectoryTarget === undefined
				? projectWorkspaceRuntime.resolveActiveTerminalCwd(input.directoryId)
				: projectWorkspaceRuntime.resolveTerminalCurrentDirectoryTarget(
						input.currentDirectoryTarget,
					);
		if (cwdResult.kind === 'rejected' || cwdResult.kind === 'unavailable') {
			return { code: cwdResult.reason, kind: 'blocked' };
		}
		context.markCommitted();
		const commonInput = {
			groupId: input.groupId,
			...(input.index === undefined ? {} : { index: input.index }),
			source: context.source,
			title: intl.formatMessage(workbenchTabMessages.terminalTitle),
		};
		const creation =
			input.currentDirectoryTarget === undefined
				? projectWorkspaceRuntime.createTerminal({
						...commonInput,
						...(input.directoryId === undefined ? {} : { directoryId: input.directoryId }),
					})
				: projectWorkspaceRuntime.createTerminal({
						...commonInput,
						currentDirectoryTarget: input.currentDirectoryTarget,
					});
		if (creation.kind === 'rejected') {
			throw new Error('A resolved terminal Directory became invalid during synchronous creation.');
		}
		scopedTabModel?.includeTabOptimistically(creation.tabId, true);
		return { kind: 'completed' };
	});
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
		isSidebarVisible,
		navigation,
		onNewTerminal: createTerminal,
		onSettings,
		onTerminalFontSizeChange,
		openSearch,
		openTabTitleEditor,
		primaryWorkspaceName: intl.formatMessage(workbenchMessages.workspacePrimaryName),
		projectWorkspaceRuntime,
		projectWorkspaceState,
		runtime: actionRuntime,
		setTabStripOrientation,
		tabStripOrientation,
		tabModel,
		toggleSidebar,
		workspaceCopySuffix: intl.formatMessage(workbenchMessages.workspaceCopySuffix),
		workspaceTerminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle),
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
		activateStandalone: () => {
			actions.invoke(WORKBENCH_ACTION_IDS.activateStandalone, undefined, 'button');
		},
		actionScope: actions.scope,
		addProjectDirectory: (projectId, path) => {
			actions.invoke(WORKBENCH_ACTION_IDS.addProjectFolder, { path, projectId }, 'button');
		},
		activateWorkspace: (projectId, workspaceId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.activateWorkspace, { projectId, workspaceId }, 'button');
		},
		cancelProjectWorkspaceConfirmation: () => pendingProjectWorkspaceConfirmation.current?.(false),
		confirmProjectWorkspaceAction: () => pendingProjectWorkspaceConfirmation.current?.(true),
		createProject: (path) => {
			actions.invoke(WORKBENCH_ACTION_IDS.createProject, { path }, 'button');
		},
		createWorkspace: (projectId, directoryId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.createWorkspace, { directoryId, projectId }, 'menu');
		},
		deleteWorkspace: (projectId, workspaceId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.deleteWorkspace, { projectId, workspaceId }, 'menu');
		},
		duplicateWorkspace: (projectId, workspaceId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.duplicateWorkspace, { projectId, workspaceId }, 'menu');
		},
		isSidebarVisible,
		leftPanelSize: `${leftPanelWidth.current}px`,
		newTerminal: () => {
			actions.invoke(WORKBENCH_ACTION_IDS.createTerminal, { groupId: null }, 'button');
		},
		newTab: (request) =>
			actions.invoke(
				WORKBENCH_ACTION_IDS.createTerminal,
				{
					groupId: request.groupId,
					...(request.index === undefined ? {} : { index: request.index }),
				},
				request.source,
			),
		newTerminalLabel: intl.formatMessage(workbenchMessages.newTerminal),
		newTerminalShortcutId: WORKBENCH_SHORTCUTS.newTerminal.id,
		newTerminalShortcutLabel: formatWorkbenchShortcut(
			'newTerminal',
			shortcutPlatform,
			shortcutBindings,
		),
		onCloseTabs: closeTabs,
		getCloseTabsConfirmation,
		onLayoutChanged: (_layout, meta) => {
			if (meta.isUserInteraction) {
				writeWorkbenchLeftPanelWidth(storage, leftPanelWidth.current);
			}
		},
		onLeftPanelResize: (size) => {
			leftPanelWidth.current = size.inPixels;
		},
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
		projectNavigationMessages: {
			add: intl.formatMessage(workbenchMessages.resourceAdd),
			cancel: intl.formatMessage(workbenchMessages.resourceCancel),
			close: intl.formatMessage(workbenchMessages.resourceClose),
			collapseProject: intl.formatMessage(workbenchMessages.projectCollapse),
			create: intl.formatMessage(workbenchMessages.resourceCreate),
			createWorkspace: intl.formatMessage(workbenchMessages.projectCreateWorkspace),
			deleteWorkspace: intl.formatMessage(workbenchMessages.workspaceDelete),
			duplicateWorkspace: intl.formatMessage(workbenchMessages.workspaceDuplicate),
			expandProject: intl.formatMessage(workbenchMessages.projectExpand),
			folderAcquisitionFailureDismiss: intl.formatMessage(
				workbenchMessages.projectFolderAcquisitionDismiss,
			),
			folderAcquisitionFailureMessages: {
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
			projectManage: intl.formatMessage(workbenchMessages.projectManage),
			projectManageAddFolder: intl.formatMessage(workbenchMessages.projectAddFolder),
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
		projectWorkspaceConfirmationConfirmButtonRef,
		projectWorkspaceConfirmation,
		projectWorkspaceState,
		removeProject: (projectId) => {
			actions.invoke(WORKBENCH_ACTION_IDS.removeProject, { projectId }, 'menu');
		},
		removeProjectDirectory: (projectId, directoryId) => {
			actions.invoke(
				WORKBENCH_ACTION_IDS.removeProjectDirectory,
				{ directoryId, projectId },
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
		reorderWorkspace: (projectId, workspaceId, beforeWorkspaceId) => {
			actions.invoke(
				WORKBENCH_ACTION_IDS.reorderWorkspace,
				{ beforeWorkspaceId, projectId, workspaceId },
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
		tabModel,
		hasVisibleTerminalTabs: tabModelSnapshot.tabs.length > 0,
		terminalLabel: intl.formatMessage(workbenchMessages.terminal),
		tabStripOrientation,
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
		setProjectPrimaryDirectory: (projectId, directoryId) => {
			actions.invoke(
				WORKBENCH_ACTION_IDS.setProjectPrimaryDirectory,
				{ directoryId, projectId },
				'button',
			);
		},
		updateWorkspace: (projectId, workspaceId, name, primaryDirectoryId) => {
			actions.invoke(
				WORKBENCH_ACTION_IDS.updateWorkspace,
				{ name, primaryDirectoryId, projectId, workspaceId },
				'button',
			);
		},
		updateSearch,
	};
}
