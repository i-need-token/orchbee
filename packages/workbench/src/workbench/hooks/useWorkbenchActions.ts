import {
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionCatalogId,
	type ActionExecution,
	type ActionInputFor,
	type ActionInvocation,
	type ActionInvocationSource,
	type ActionOutputFor,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { workbenchMessages } from '@orchbee/i18n';
import type { TabStripOrientation } from '@orchbee/tab-strip';
import * as React from 'react';

import type { ProjectWorkspaceState } from '../../project-workspace';
import { registerWorkbenchProjectWorkspaceActions } from '../actions/registerWorkbenchProjectWorkspaceActions';
import { registerWorkbenchActiveTerminalActions } from '../actions/registerWorkbenchActiveTerminalActions';
import { registerWorkbenchActiveTabMovementActions } from '../actions/registerWorkbenchActiveTabMovementActions';
import {
	registerWorkbenchTabTitleAction,
	type WorkbenchTabTitleEditorRequest,
} from '../actions/registerWorkbenchTabTitleAction';
import { registerWorkbenchTabSelectionActions } from '../actions/registerWorkbenchTabSelectionActions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchSearchOpenInput,
	workbenchSearchOpenInput,
	workbenchTabStripOrientationInput,
	type WorkbenchTerminalCreateInput,
	workbenchTerminalCreateInput,
} from '../actions/workbench-action-ids';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchHostActionCallback,
	WorkbenchNavigationPort,
	WorkbenchTerminalFontSizeChange,
	WorkbenchTerminalFontSizeChangeCallback,
	WorkbenchTerminalCreateCallback,
} from '../types/workbench-host-actions';
import { WORKBENCH_SHORTCUTS, type WorkbenchShortcutBinding } from '../utils/workbench-shortcuts';

interface UseWorkbenchActionsOptions {
	readonly isSidebarVisible: boolean;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly onNewTerminal: WorkbenchTerminalCreateCallback | undefined;
	readonly openSearch: (input: WorkbenchSearchOpenInput) => void;
	readonly openTabTitleEditor: (request: WorkbenchTabTitleEditorRequest) => void;
	readonly onSettings: WorkbenchHostActionCallback | undefined;
	readonly onTerminalFontSizeChange?: WorkbenchTerminalFontSizeChangeCallback | undefined;
	readonly primaryWorkspaceName: string;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly runtime: ActionRuntime;
	readonly setTabStripOrientation: (orientation: TabStripOrientation) => void;
	readonly tabStripOrientation: TabStripOrientation;
	readonly toggleSidebar: () => void;
	readonly workspaceCopySuffix: string;
	readonly workspaceTerminalTitle: string;
}

interface WorkbenchActions {
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
	isSidebarVisible,
	navigation,
	onNewTerminal,
	openSearch,
	openTabTitleEditor,
	onSettings,
	onTerminalFontSizeChange,
	primaryWorkspaceName,
	projectWorkspaceRuntime,
	projectWorkspaceState,
	runtime,
	setTabStripOrientation,
	tabStripOrientation,
	toggleSidebar,
	workspaceCopySuffix,
	workspaceTerminalTitle,
}: UseWorkbenchActionsOptions): WorkbenchActions {
	const scope = React.useMemo(() => runtime.createScope('workbench'), [runtime]);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
		[runtime],
	);

	React.useEffect(() => {
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
		const availabilityFor = (callback: unknown) => () =>
			callback === undefined
				? {
						kind: 'disabled' as const,
						reason: workbenchMessages.actionUnavailable,
					}
				: { kind: 'enabled' as const };
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
			registerWorkbenchTabTitleAction({
				openEditor: openTabTitleEditor,
				projectWorkspaceRuntime,
				scope,
				typedRuntime,
			}),
			...registerWorkbenchActiveTabMovementActions({
				projectWorkspaceRuntime,
				runtime,
				scope,
				typedRuntime,
			}),
			...registerWorkbenchActiveTerminalActions({
				projectWorkspaceRuntime,
				runtime,
				scope,
				typedRuntime,
			}),
			...registerWorkbenchProjectWorkspaceActions({
				navigation,
				primaryWorkspaceName,
				projectWorkspaceRuntime,
				scope,
				typedRuntime,
				workspaceCopySuffix,
				workspaceTerminalTitle,
			}),
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
						concurrency: 'parallel',
						concurrencyKey: (input) => `${input.groupId ?? 'ungrouped'}:${input.index ?? 'end'}`,
						mode: 'foreground',
					},
					getAvailability: availabilityFor(onNewTerminal),
					handler: {
						execute: (context, input) => {
							if (onNewTerminal === undefined) {
								return {
									code: 'host-capability-unavailable',
									kind: 'blocked',
								};
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
					getAvailability: () => ({ kind: 'enabled' }),
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
					category: categoryView,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: () => 'tab-strip-orientation',
						mode: 'foreground',
					},
					getDiscoveryAvailability: () => ({ kind: 'enabled' }),
					getAvailability: (_context, input) => ({
						checked: input.orientation === tabStripOrientation,
						kind: 'enabled',
					}),
					handler: {
						execute: (context, input) => {
							if (input.orientation === tabStripOrientation) {
								return { kind: 'unchanged' };
							}
							context.markCommitted();
							setTabStripOrientation(input.orientation);
							return { kind: 'completed' };
						},
					},
					id: WORKBENCH_ACTION_IDS.setTabStripOrientation,
					input: workbenchTabStripOrientationInput,
					title: workbenchMessages.tabStripOrientation,
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
		isSidebarVisible,
		navigation,
		onNewTerminal,
		onSettings,
		onTerminalFontSizeChange,
		openSearch,
		openTabTitleEditor,
		primaryWorkspaceName,
		projectWorkspaceRuntime,
		projectWorkspaceState,
		runtime,
		setTabStripOrientation,
		scope,
		tabStripOrientation,
		toggleSidebar,
		typedRuntime,
		workspaceCopySuffix,
		workspaceTerminalTitle,
	]);

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
				case 'selectTab9': {
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
				case 'settings':
				case 'closeActiveTerminal':
				case 'decreaseTerminalFontSize':
				case 'equalizePanels':
				case 'focusActivePanelDown':
				case 'focusActivePanelLeft':
				case 'focusActivePanelRight':
				case 'focusActivePanelUp':
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
				case 'resizeActivePanel':
				case 'setTabTitle':
				case 'splitActivePanelBelow':
				case 'splitActivePanelRight':
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

	return { invoke, invokeShortcut, scope };
}
