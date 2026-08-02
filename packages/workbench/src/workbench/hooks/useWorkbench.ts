import { useOrchbeeIntl, workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import type {
	ActionConfirmation,
	ActionExecution,
	ActionRuntime,
	ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import type {
	TabStripCloseCallback,
	TabStripNewTabRequest,
	TabStripOrientation,
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
	WorkbenchTerminalCreateCallback,
} from '../types/workbench-host-actions';
import type { WorkbenchSearchFilter } from '../types/workbench-search';
import type { WorkbenchConfirmationAnchor } from '../types/workbench-confirmation';
import {
	createWorkbenchSearchState,
	transitionWorkbenchSearch,
	type WorkbenchSearchEvent,
	type WorkbenchSearchState,
} from '../components/workbench-search-surface/model/workbench-search-state';
import { formatWorkbenchShortcut, WORKBENCH_SHORTCUTS } from '../utils/workbench-shortcuts';
import { WORKBENCH_ACTION_IDS } from '../actions/workbench-action-ids';
import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalContext,
} from '../model/WorkbenchProjectWorkspaceRuntime';
import {
	readWorkbenchConfirmationAnchor,
	workbenchConfirmationCapabilities,
} from '../model/workbench-confirmation-invocation';
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
	readonly deleteWorkspace: (
		projectId: string,
		workspaceId: string,
		anchor: WorkbenchConfirmationAnchor,
	) => void;
	readonly duplicateWorkspace: (projectId: string, workspaceId: string) => void;
	readonly isSidebarVisible: boolean;
	readonly leftPanelSize: string;
	readonly newTerminal: (() => void) | undefined;
	readonly newTab: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly onCloseTabs: TabStripCloseCallback;
	readonly newTerminalLabel: string;
	readonly newTerminalShortcutId: string;
	readonly newTerminalShortcutLabel: string;
	readonly onLayoutChanged: (
		layout: Readonly<Record<string, number>>,
		meta: ResizableLayoutChangedMeta,
	) => void;
	readonly onLeftPanelResize: (size: ResizablePanelSize) => void;
	readonly sidebarShortcutId: string;
	readonly sidebarShortcutLabel: string;
	readonly sidebarToggleLabel: string;
	readonly sidebarLabel: string;
	readonly projectLabel: string;
	readonly projectNavigationMessages: {
		readonly add: string;
		readonly cancel: string;
		readonly close: string;
		readonly collapseProject: string;
		readonly create: string;
		readonly createWorkspace: string;
		readonly deleteWorkspace: string;
		readonly duplicateWorkspace: string;
		readonly expandProject: string;
		readonly newFolder: string;
		readonly projectFolders: string;
		readonly projectCreate: string;
		readonly projectCreateDescription: string;
		readonly projectCreatePath: string;
		readonly projectCreatePathDescription: string;
		readonly projectCreatePathPlaceholder: string;
		readonly projectCreateTitle: string;
		readonly projectManage: string;
		readonly projectManageAddFolder: string;
		readonly projectManageDescription: string;
		readonly projectManageFolderAlreadyAdded: string;
		readonly projectManageFolders: string;
		readonly projectManageName: string;
		readonly projectManageNamePlaceholder: string;
		readonly projectManagePrimaryFolder: string;
		readonly projectManageTitle: string;
		readonly projectOptions: string;
		readonly projectWorktrees: string;
		readonly removeProject: string;
		readonly removeProjectFolder: string;
		readonly save: string;
		readonly selectFolder: string;
		readonly setProjectPrimaryFolder: string;
		readonly workspaceManage: string;
		readonly workspaceManageDescription: string;
		readonly workspaceManageName: string;
		readonly workspaceManageNamePlaceholder: string;
		readonly workspaceManagePrimaryFolder: string;
		readonly workspaceManageTitle: string;
		readonly workspaceOptions: string;
	};
	readonly projectWorkspaceConfirmation: {
		readonly anchor: WorkbenchConfirmationAnchor | null;
		readonly cancelLabel: string;
		readonly confirmLabel: string;
		readonly description: string;
		readonly title: string;
	} | null;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly removeProject: (projectId: string, anchor: WorkbenchConfirmationAnchor) => void;
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
	readonly terminalContext: WorkbenchTerminalContext;
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
	readonly onSettings: WorkbenchHostActionCallback | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly storage: WorkbenchLayoutStorage | undefined;
}

export function useWorkbench({
	actionRuntime,
	onSettings,
	projectWorkspaceRuntime,
	shortcutPlatform,
	storage,
}: UseWorkbenchOptions): WorkbenchView {
	const intl = useOrchbeeIntl();
	const pendingProjectWorkspaceConfirmation = React.useRef<((accepted: boolean) => void) | null>(
		null,
	);
	const [projectWorkspaceConfirmation, setProjectWorkspaceConfirmation] = React.useState<{
		readonly anchor: WorkbenchConfirmationAnchor | null;
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
		const terminalSequence = projectWorkspaceRuntime.getActiveNextTerminalSequence();
		const cwdResult = projectWorkspaceRuntime.resolveActiveTerminalCwd(input.directoryId);
		if (cwdResult.kind === 'rejected') {
			return { code: cwdResult.reason, kind: 'blocked' };
		}
		context.markCommitted();
		const creation = projectWorkspaceRuntime.createTerminal({
			...(input.directoryId === undefined ? {} : { directoryId: input.directoryId }),
			groupId: input.groupId,
			...(input.index === undefined ? {} : { index: input.index }),
			source: context.source,
			title: intl.formatMessage(workbenchTabMessages.terminalTitle, {
				number: terminalSequence,
			}),
		});
		if (creation.kind === 'rejected') {
			throw new Error('A resolved terminal Directory became invalid during synchronous creation.');
		}
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
	const actions = useWorkbenchActions({
		isSidebarVisible,
		onNewTerminal: createTerminal,
		onSettings,
		openSearch,
		primaryWorkspaceName: intl.formatMessage(workbenchMessages.workspacePrimaryName),
		projectWorkspaceRuntime,
		projectWorkspaceState,
		runtime: actionRuntime,
		setTabStripOrientation,
		tabStripOrientation,
		toggleSidebar,
		workspaceCopySuffix: intl.formatMessage(workbenchMessages.workspaceCopySuffix),
		workspaceTerminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle, { number: 1 }),
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
			(confirmation, invocation, signal) =>
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
						anchor: readWorkbenchConfirmationAnchor(invocation.capabilities),
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

	useWorkbenchShortcuts(shortcutPlatform, actions.invokeShortcut);

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
		deleteWorkspace: (projectId, workspaceId, anchor) => {
			actions.invoke(WORKBENCH_ACTION_IDS.deleteWorkspace, { projectId, workspaceId }, 'menu', {
				capabilities: workbenchConfirmationCapabilities(anchor),
			});
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
		newTerminalShortcutLabel: formatWorkbenchShortcut('newTerminal', shortcutPlatform),
		onCloseTabs: closeTabs,
		onLayoutChanged: (_layout, meta) => {
			if (meta.isUserInteraction) {
				writeWorkbenchLeftPanelWidth(storage, leftPanelWidth.current);
			}
		},
		onLeftPanelResize: (size) => {
			leftPanelWidth.current = size.inPixels;
		},
		sidebarShortcutId: WORKBENCH_SHORTCUTS.toggleSidebar.id,
		sidebarShortcutLabel: formatWorkbenchShortcut('toggleSidebar', shortcutPlatform),
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
			newFolder: intl.formatMessage(workbenchMessages.projectNewFolder),
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
		projectWorkspaceConfirmation,
		projectWorkspaceState,
		removeProject: (projectId, anchor) => {
			actions.invoke(WORKBENCH_ACTION_IDS.removeProject, { projectId }, 'menu', {
				capabilities: workbenchConfirmationCapabilities(anchor),
			});
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
		searchShortcutLabel: formatWorkbenchShortcut('search', shortcutPlatform),
		settings:
			onSettings === undefined
				? undefined
				: () => {
						actions.invoke(WORKBENCH_ACTION_IDS.openSettings, undefined, 'button');
					},
		settingsLabel: intl.formatMessage(workbenchMessages.settings),
		settingsShortcutId: WORKBENCH_SHORTCUTS.settings.id,
		settingsShortcutLabel: formatWorkbenchShortcut('settings', shortcutPlatform),
		terminalContext: projectWorkspaceRuntime.getActiveTerminalContext(),
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
