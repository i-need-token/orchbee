import {
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionCatalogId,
	type ActionAvailability,
	type ActionExecution,
	type ActionInputFor,
	type ActionInvocation,
	type ActionInvocationSource,
	type ActionOutputFor,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import type { TabStripOrientation } from '@orchbee/tab-strip';
import * as React from 'react';

import type { ProjectWorkspaceState } from '../../project-workspace';
import { registerWorkbenchProjectWorkspaceActions } from '../actions/registerWorkbenchProjectWorkspaceActions';
import { registerWorkbenchAgentNavigationAction } from '../actions/registerWorkbenchAgentNavigationAction';
import { registerWorkbenchAgentStartActions } from '../actions/registerWorkbenchAgentStartActions';
import { registerWorkbenchConfiguredCommandActions } from '../actions/registerWorkbenchConfiguredCommandActions';
import { registerWorkbenchActiveTerminalActions } from '../actions/registerWorkbenchActiveTerminalActions';
import { registerWorkbenchActiveTabActions } from '../actions/registerWorkbenchActiveTabActions';
import { registerWorkbenchTabCloseConfirmedAction } from '../actions/registerWorkbenchTabCloseConfirmedAction';
import { registerWorkbenchTabCloseResumeAction } from '../actions/registerWorkbenchTabCloseResumeAction';
import {
	registerWorkbenchTabTitleAction,
	type WorkbenchTabTitleEditorRequest,
} from '../actions/registerWorkbenchTabTitleAction';
import {
	registerWorkbenchPanelNameAction,
	type WorkbenchPanelNameEditorRequest,
} from '../actions/registerWorkbenchPanelNameAction';
import { registerWorkbenchPanelTargetCloseAction } from '../actions/registerWorkbenchPanelTargetCloseAction';
import { registerWorkbenchTabSelectionActions } from '../actions/registerWorkbenchTabSelectionActions';
import { registerWorkbenchFocusActions } from '../actions/registerWorkbenchFocusActions';
import { registerWorkbenchEarlierLayoutAction } from '../actions/registerWorkbenchEarlierLayoutAction';
import { registerWorkbenchLastPanelAction } from '../actions/registerWorkbenchLastPanelAction';
import type { WorkbenchWorktreeFormRequest } from '../types/workbench-worktree-form';
import { registerWorkbenchWorktreeActions } from '../actions/registerWorkbenchWorktreeActions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchSearchOpenInput,
	workbenchSearchOpenInput,
	type WorkbenchTerminalCreateInput,
	workbenchTerminalCreateInput,
} from '../actions/workbench-action-ids';
import {
	createWorkbenchFocusAfterTabClose,
	type WorkbenchFocusAfterTabClose,
} from '../actions/workbench-tab-close-focus';
import type { TerminalCloseImpact } from '../model/terminal-close-impact';
import { WorkbenchPanelFocusTrail } from '../model/WorkbenchPanelFocusTrail';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAgentSource } from '../types/workbench-agent-observation';
import type { WorkbenchAgentStartPlace } from '../types/workbench-agent-start';
import type {
	WorkbenchHostActionCallback,
	WorkbenchNavigationPort,
	WorkbenchTerminalFontSizeChange,
	WorkbenchTerminalFontSizeChangeCallback,
	WorkbenchTerminalCreateCallback,
} from '../types/workbench-host-actions';
import type { WorkbenchConfiguredCommandKeys } from '../model/workbench-configured-command-keys';
import type { WorkbenchAuthority } from '../types/workbench-authority';
import type { WorkbenchConfiguredCommand } from '../types/workbench-configured-commands';
import type { WorkbenchFocusRouting } from '../types/workbench-focus-routing';
import type { WorkbenchSearchFilter } from '../types/workbench-search';
import { WORKBENCH_SHORTCUTS, type WorkbenchShortcutBinding } from '../utils/workbench-shortcuts';

interface UseWorkbenchActionsOptions {
	readonly agentObservationSource?: WorkbenchAgentSource | undefined;
	readonly authority: WorkbenchAuthority;
	/** The configured commands as the window last read them, and what its keys do with them. */
	readonly configuredCommandCatalog: readonly WorkbenchConfiguredCommand[];
	readonly configuredCommandKeys: WorkbenchConfiguredCommandKeys;
	readonly focusRouting: Pick<
		WorkbenchFocusRouting,
		'beginTerminalHandover' | 'focus' | 'getResolution'
	>;
	/** The localized sentence naming what a close ends; keep it stable across renders. */
	readonly formatCloseWarning: (impact: TerminalCloseImpact) => string;
	readonly isSidebarVisible: boolean;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly onNewTerminal: WorkbenchTerminalCreateCallback | undefined;
	readonly getNewTerminalAvailability?:
		| ((input: WorkbenchTerminalCreateInput) => ActionAvailability)
		| undefined;
	readonly openAgentBoard: () => void;
	readonly openAgentStart: (places: readonly WorkbenchAgentStartPlace[]) => void;
	readonly openSearch: (input: WorkbenchSearchOpenInput) => void;
	/** The Search filters the window offers; a release offers only Actions. */
	readonly searchFilters: readonly WorkbenchSearchFilter[];
	readonly openTabTitleEditor: (request: WorkbenchTabTitleEditorRequest) => void;
	readonly openPanelNameEditor: (request: WorkbenchPanelNameEditorRequest) => void;
	readonly onSettings: WorkbenchHostActionCallback | undefined;
	readonly onShortcuts?: WorkbenchHostActionCallback | undefined;
	readonly onTerminalFontSizeChange?: WorkbenchTerminalFontSizeChangeCallback | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly runtime: ActionRuntime;
	/** How the strip lays out Tabs now, which names the place after the active Tab. */
	readonly tabStripOrientation: TabStripOrientation;
	readonly toggleSidebar: () => void;
	/** Opens the New Worktree form; keep it stable across renders. */
	readonly requestWorktreeForm: WorkbenchWorktreeFormRequest;
	/** The localized default Worktree Workspace label with this ordinal; keep it stable across renders. */
	readonly worktreeLabel: (ordinal: number) => string;
}

interface WorkbenchActions {
	/** Where focus goes after a Tab close, read from the same Panel focus trail Last Panel uses. */
	readonly focusAfterTabClose: WorkbenchFocusAfterTabClose;
	readonly invoke: <Id extends ActionCatalogId<WorkbenchActionCatalog>>(
		actionId: Id,
		input: ActionInputFor<WorkbenchActionCatalog, Id>,
		source: ActionInvocationSource,
		details?: Pick<ActionInvocation, 'capabilities' | 'focus' | 'target'>,
	) => ActionExecution<ActionOutputFor<WorkbenchActionCatalog, Id>>;
	readonly invokeShortcut: (
		binding: WorkbenchShortcutBinding,
		source: ActionInvocationSource,
	) => boolean;
	readonly scope: ActionScope;
}

export function useWorkbenchActions({
	agentObservationSource,
	authority,
	configuredCommandCatalog,
	configuredCommandKeys,
	focusRouting,
	formatCloseWarning,
	isSidebarVisible,
	navigation,
	onNewTerminal,
	getNewTerminalAvailability,
	openAgentBoard,
	openAgentStart,
	openSearch,
	openTabTitleEditor,
	openPanelNameEditor,
	onSettings,
	onShortcuts,
	onTerminalFontSizeChange,
	projectWorkspaceRuntime,
	projectWorkspaceState,
	requestWorktreeForm,
	runtime,
	searchFilters,
	tabStripOrientation,
	toggleSidebar,
	worktreeLabel,
}: UseWorkbenchActionsOptions): WorkbenchActions {
	const scope = React.useMemo(() => runtime.createScope('workbench'), [runtime]);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
		[runtime],
	);
	// Outlives the registrations below, which re-run as the Workbench changes.
	const panelFocusTrail = React.useMemo(
		() => new WorkbenchPanelFocusTrail(projectWorkspaceRuntime),
		[projectWorkspaceRuntime],
	);
	React.useEffect(() => panelFocusTrail.observe(), [panelFocusTrail]);
	const focusAfterTabClose = React.useMemo(
		() =>
			createWorkbenchFocusAfterTabClose({
				navigation,
				projectWorkspaceRuntime,
				trail: panelFocusTrail,
			}),
		[navigation, panelFocusTrail, projectWorkspaceRuntime],
	);

	React.useEffect(() => {
		const resolveCurrentDirectoryTarget = (context: Pick<ActionInvocation, 'target'>) =>
			context.target !== undefined && context.target.type !== 'panel'
				? ({ kind: 'unavailable', reason: 'panel-unavailable' } as const)
				: projectWorkspaceRuntime.resolveActiveTerminalCurrentDirectoryTarget(context.target?.id);
		const captureCurrentDirectoryTarget = (context: Pick<ActionInvocation, 'target'>) => {
			const resolution = resolveCurrentDirectoryTarget(context);
			return resolution.kind === 'ready' ? resolution.target : null;
		};
		const categoryNavigation = {
			id: 'navigation',
			title: workbenchMessages.actionCategoryNavigation,
		};
		const categoryTerminal = {
			id: 'terminal',
			title: workbenchMessages.actionCategoryTerminal,
		};
		const categoryView = {
			id: 'view',
			title: workbenchMessages.actionCategoryView,
		};
		const execution = {
			concurrency: 'parallel' as const,
			concurrencyKey: () => 'workbench',
			mode: 'foreground' as const,
		};
		// A host that wires no callback lacks the Action, so no surface offers it.
		const availabilityFor = (callback: unknown) => () =>
			callback === undefined ? { kind: 'hidden' as const } : { kind: 'enabled' as const };
		const handlerFor = (callback: WorkbenchHostActionCallback | undefined) => ({
			execute: (context: Parameters<WorkbenchHostActionCallback>[0]) => {
				if (callback === undefined) {
					return { kind: 'blocked' as const, code: 'host-capability-unavailable' };
				}
				return callback(context);
			},
		});
		const terminalFontSizeHandlerFor = (change: WorkbenchTerminalFontSizeChange) => ({
			execute: (context: Parameters<WorkbenchTerminalFontSizeChangeCallback>[0]) => {
				if (onTerminalFontSizeChange === undefined) {
					return { kind: 'blocked' as const, code: 'host-capability-unavailable' };
				}
				return onTerminalFontSizeChange(context, change);
			},
		});
		const registrations = [
			registerWorkbenchAgentNavigationAction({
				navigation,
				projectWorkspaceRuntime,
				scope,
				source: agentObservationSource,
				typedRuntime,
			}),
			...registerWorkbenchFocusActions({
				focusRouter: focusRouting,
				scope,
				typedRuntime,
			}),
			registerWorkbenchLastPanelAction({
				navigation,
				projectWorkspaceRuntime,
				scope,
				trail: panelFocusTrail,
				typedRuntime,
			}),
			registerWorkbenchPanelNameAction({
				authority,
				openEditor: openPanelNameEditor,
				projectWorkspaceRuntime,
				scope,
				typedRuntime,
			}),
			...registerWorkbenchPanelTargetCloseAction({
				authority,
				projectWorkspaceRuntime,
				scope,
				typedRuntime,
			}),
			registerWorkbenchTabTitleAction({
				authority,
				openEditor: openTabTitleEditor,
				projectWorkspaceRuntime,
				scope,
				typedRuntime,
			}),
			...registerWorkbenchActiveTabActions({
				projectWorkspaceRuntime,
				runtime,
				scope,
				tabStripOrientation,
				typedRuntime,
			}),
			...registerWorkbenchActiveTerminalActions({
				projectWorkspaceRuntime,
				runtime,
				scope,
				stoppedTabClose: authority.stoppedTabClose,
				typedRuntime,
			}),
			registerWorkbenchTabCloseConfirmedAction({
				authority,
				beginTerminalHandover: focusRouting.beginTerminalHandover,
				focusAfterTabClose,
				formatCloseWarning,
				projectWorkspaceRuntime,
				scope,
				typedRuntime,
			}),
			registerWorkbenchTabCloseResumeAction({
				authority,
				beginTerminalHandover: focusRouting.beginTerminalHandover,
				focusAfterTabClose,
				formatCloseWarning,
				projectWorkspaceRuntime,
				scope,
				typedRuntime,
			}),
			...registerWorkbenchProjectWorkspaceActions({
				authority,
				beginTerminalHandover: focusRouting.beginTerminalHandover,
				navigation,
				projectWorkspaceRuntime,
				scope,
				typedRuntime,
			}),
			...(authority.earlierLayouts === undefined
				? []
				: [
						registerWorkbenchEarlierLayoutAction({
							earlierLayouts: authority.earlierLayouts,
							navigation,
							projectWorkspaceRuntime,
							scope,
							typedRuntime,
						}),
					]),
			...(authority.agentCatalog === undefined
				? []
				: registerWorkbenchAgentStartActions({
						agentCatalog: authority.agentCatalog,
						authority,
						beginTerminalHandover: focusRouting.beginTerminalHandover,
						openAgentStart,
						projectWorkspaceRuntime,
						scope,
						typedRuntime,
					})),
			...(authority.worktrees === undefined
				? []
				: registerWorkbenchWorktreeActions({
						projectWorkspaceRuntime,
						requestWorktreeForm,
						scope,
						typedRuntime,
						worktreeLabel,
						worktrees: authority.worktrees,
					})),
			...registerWorkbenchTabSelectionActions({
				projectWorkspaceRuntime,
				runtime,
				scope,
				typedRuntime,
			}),
			typedRuntime.register(
				scope,
				defineAction({
					category: categoryTerminal,
					execution: {
						concurrency: 'reject-duplicate',
						concurrencyKey: (input) =>
							`${input.groupId ?? 'ungrouped'}:${input.index ?? 'end'}:${input.profile?.id ?? 'default'}`,
						mode: 'foreground',
					},
					getAvailability: (_context, input) =>
						getNewTerminalAvailability?.(input) ?? availabilityFor(onNewTerminal)(),
					handler: {
						execute: (context, input) => {
							if (onNewTerminal === undefined) {
								return {
									code: 'host-capability-unavailable',
									kind: 'blocked',
								};
							}
							if (projectWorkspaceRuntime.getSnapshot().activeLocation.kind !== 'workspace') {
								// A Tab needs a Workspace, and creating one explicitly has its own owner.
								typedRuntime.execute(WORKBENCH_ACTION_IDS.createWorkspace, undefined, {
									scope,
									source: context.source,
								});
								return { kind: 'unchanged' };
							}
							return onNewTerminal(context, input);
						},
					},
					id: WORKBENCH_ACTION_IDS.createTerminal,
					input: workbenchTerminalCreateInput,
					title: workbenchMessages.newTerminal,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: categoryTerminal,
					execution: {
						concurrency: 'serialize',
						concurrencyKey: () => 'current-directory-terminal-creation',
						mode: 'foreground',
					},
					getAvailability: (context) => {
						if (onNewTerminal === undefined) return { kind: 'hidden' };
						const resolution = resolveCurrentDirectoryTarget(context);
						if (resolution.kind === 'ready') return { kind: 'enabled' };
						return {
							kind: 'disabled',
							reason:
								resolution.reason === 'terminal-current-directory-unavailable'
									? workbenchMessages.panelUnavailableDirectoryUnreported
									: workbenchMessages.panelUnavailableFocusedPanelRequired,
						};
					},
					handler: {
						execute: (context) => {
							if (onNewTerminal === undefined) {
								return { code: 'host-capability-unavailable', kind: 'blocked' };
							}
							const target = captureCurrentDirectoryTarget(context);
							if (target === null) {
								return { code: 'terminal-current-directory-unavailable', kind: 'blocked' };
							}
							return onNewTerminal(context, {
								currentDirectoryTarget: target,
								groupId: null,
							});
						},
					},
					id: WORKBENCH_ACTION_IDS.createTerminalInCurrentDirectory,
					input: noActionInput,
					title: workbenchTabMessages.newTabCurrentDirectory,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: categoryTerminal,
					execution,
					getAvailability: availabilityFor(onTerminalFontSizeChange),
					handler: terminalFontSizeHandlerFor('increase'),
					id: WORKBENCH_ACTION_IDS.increaseTerminalFontSize,
					input: noActionInput,
					title: workbenchMessages.terminalFontSizeIncrease,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: categoryTerminal,
					execution,
					getAvailability: availabilityFor(onTerminalFontSizeChange),
					handler: terminalFontSizeHandlerFor('decrease'),
					id: WORKBENCH_ACTION_IDS.decreaseTerminalFontSize,
					input: noActionInput,
					title: workbenchMessages.terminalFontSizeDecrease,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: categoryTerminal,
					execution,
					getAvailability: availabilityFor(onTerminalFontSizeChange),
					handler: terminalFontSizeHandlerFor('reset'),
					id: WORKBENCH_ACTION_IDS.resetTerminalFontSize,
					input: noActionInput,
					title: workbenchMessages.terminalFontSizeReset,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: categoryNavigation,
					execution,
					// A filter the window withholds is not offered, so no key, list or row opens it.
					getAvailability: (_context, input) =>
						searchFilters.includes(input.filter) ? { kind: 'enabled' } : { kind: 'hidden' },
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							openSearch(input);
							return { kind: 'completed' };
						},
					},
					id: WORKBENCH_ACTION_IDS.openSearch,
					input: workbenchSearchOpenInput,
					title: workbenchMessages.search,
				}),
			),
			// Only an authority that reads the Server's Agent sessions has a board to open.
			...(authority.agentBoard === undefined
				? []
				: [
						typedRuntime.register(
							scope,
							defineAction({
								category: categoryNavigation,
								execution,
								getAvailability: () => ({ kind: 'enabled' }),
								handler: {
									execute: (context) => {
										context.markCommitted();
										openAgentBoard();
										return { kind: 'completed' };
									},
								},
								id: WORKBENCH_ACTION_IDS.openAgentBoard,
								input: noActionInput,
								title: workbenchMessages.agentBoardTitle,
							}),
						),
					]),
			typedRuntime.register(
				scope,
				defineAction({
					category: categoryNavigation,
					execution,
					getAvailability: availabilityFor(onSettings),
					handler: handlerFor(onSettings),
					id: WORKBENCH_ACTION_IDS.openSettings,
					input: noActionInput,
					title: workbenchMessages.settings,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: categoryNavigation,
					execution,
					getAvailability: availabilityFor(onShortcuts),
					handler: handlerFor(onShortcuts),
					id: WORKBENCH_ACTION_IDS.openShortcuts,
					input: noActionInput,
					title: workbenchMessages.shortcuts,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: categoryView,
					execution,
					getAvailability: () => ({ checked: isSidebarVisible, kind: 'enabled' }),
					handler: {
						execute: (context) => {
							context.markCommitted();
							toggleSidebar();
							return { kind: 'completed' };
						},
					},
					id: WORKBENCH_ACTION_IDS.toggleSidebar,
					input: noActionInput,
					title: workbenchMessages.toggleSidebar,
				}),
			),
		];
		// Re-registration notifies registry observers to resolve fresh availability; handlers read the
		// Runtime snapshot directly and never treat this render snapshot as execution authority.
		return () => {
			for (const registration of registrations) {
				registration.dispose();
			}
		};
	}, [
		agentObservationSource,
		authority,
		focusAfterTabClose,
		focusRouting,
		formatCloseWarning,
		isSidebarVisible,
		navigation,
		getNewTerminalAvailability,
		onNewTerminal,
		onSettings,
		onShortcuts,
		onTerminalFontSizeChange,
		openAgentBoard,
		openAgentStart,
		openSearch,
		openTabTitleEditor,
		openPanelNameEditor,
		panelFocusTrail,
		projectWorkspaceRuntime,
		projectWorkspaceState,
		requestWorktreeForm,
		runtime,
		searchFilters,
		scope,
		tabStripOrientation,
		toggleSidebar,
		typedRuntime,
		worktreeLabel,
	]);

	// Its own registration, so a changed list or key note registers the configured commands alone.
	const configuredCommands = authority.configuredCommands;
	React.useEffect(() => {
		if (configuredCommands === undefined) return;
		const registration = registerWorkbenchConfiguredCommandActions({
			catalog: configuredCommandCatalog,
			commands: configuredCommands,
			notes: configuredCommandKeys.notes,
			runtime,
			scope,
		});
		return () => registration.dispose();
	}, [configuredCommandCatalog, configuredCommandKeys, configuredCommands, runtime, scope]);

	const invoke = useMemoizedFn(
		<Id extends ActionCatalogId<WorkbenchActionCatalog>>(
			actionId: Id,
			input: ActionInputFor<WorkbenchActionCatalog, Id>,
			source: ActionInvocationSource,
			details?: Pick<ActionInvocation, 'capabilities' | 'focus' | 'target'>,
		) => typedRuntime.execute(actionId, input, { ...details, scope, source }),
	);
	const invokeShortcut = useMemoizedFn(
		(binding: WorkbenchShortcutBinding, source: ActionInvocationSource) => {
			const invocation = { scope, source };
			switch (binding.action) {
				case 'newTerminal': {
					const definition = WORKBENCH_SHORTCUTS.newTerminal;
					const input: WorkbenchTerminalCreateInput = definition.input;
					if (
						typedRuntime.resolveAction(definition.id, input, invocation)?.availability.kind !==
						'enabled'
					) {
						return false;
					}
					typedRuntime.execute(definition.id, input, invocation);
					return true;
				}
				case 'actionsSearch':
				case 'search': {
					const definition = WORKBENCH_SHORTCUTS[binding.action];
					if (
						typedRuntime.resolveAction(definition.id, definition.input, invocation)?.availability
							.kind !== 'enabled'
					) {
						return false;
					}
					typedRuntime.execute(definition.id, definition.input, invocation);
					return true;
				}
				case 'selectTab1':
				case 'selectTab2':
				case 'selectTab3':
				case 'selectTab4':
				case 'selectTab5':
				case 'selectTab6':
				case 'selectTab7':
				case 'selectTab8':
				case 'selectTab9':
				case 'focusPanel1':
				case 'focusPanel2':
				case 'focusPanel3':
				case 'focusPanel4':
				case 'focusPanel5':
				case 'focusPanel6':
				case 'focusPanel7':
				case 'focusPanel8':
				case 'focusPanel9':
				case 'focusPanel10': {
					const definition = WORKBENCH_SHORTCUTS[binding.action];
					if (
						typedRuntime.resolveAction(definition.id, definition.input, invocation)?.availability
							.kind !== 'enabled'
					) {
						return false;
					}
					typedRuntime.execute(definition.id, definition.input, invocation);
					return true;
				}
				case 'newWorkspace':
				case 'settings':
				case 'shortcuts':
				case 'closeActiveTerminal':
				case 'newTerminalInCurrentDirectory':
				case 'decreaseTerminalFontSize':
				case 'equalizePanels':
				case 'focusActivePanelDown':
				case 'focusActivePanelLeft':
				case 'focusActivePanelNext':
				case 'focusActivePanelPrevious':
				case 'focusActivePanelRight':
				case 'focusActivePanelUp':
				case 'focusActivePanelVisible':
				case 'focusLastPanel':
				case 'focusNextRegion':
				case 'focusPreviousRegion':
				case 'increaseTerminalFontSize':
				case 'maximizeActivePanel':
				case 'moveActivePanelDown':
				case 'moveActivePanelLeft':
				case 'moveActivePanelRight':
				case 'moveActivePanelUp':
				case 'moveActiveTabBackward':
				case 'moveActiveTabForward':
				case 'selectNextTab':
				case 'selectPreviousTab':
				case 'resetTerminalFontSize':
				case 'resizeActivePanelDown':
				case 'resizeActivePanelLeft':
				case 'resizeActivePanelRight':
				case 'resizeActivePanelUp':
				case 'setTabTitle':
				case 'startAgent':
				case 'splitActivePanelBelow':
				case 'splitActivePanelRight':
				case 'swapActivePanelVisible':
				case 'toggleSidebar': {
					const definition = WORKBENCH_SHORTCUTS[binding.action];
					if (
						typedRuntime.resolveAction(definition.id, undefined, invocation)?.availability.kind !==
						'enabled'
					) {
						return false;
					}
					typedRuntime.execute(definition.id, undefined, invocation);
					return true;
				}
			}
		},
	);

	return { focusAfterTabClose, invoke, invokeShortcut, scope };
}
