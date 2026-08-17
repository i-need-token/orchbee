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
	WorkbenchTerminalCreateCallback,
} from '../types/workbench-host-actions';
import { WORKBENCH_SHORTCUTS, type WorkbenchShortcutBinding } from '../utils/workbench-shortcuts';

interface UseWorkbenchActionsOptions {
	readonly isSidebarVisible: boolean;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly onNewTerminal: WorkbenchTerminalCreateCallback | undefined;
	readonly openSearch: (input: WorkbenchSearchOpenInput) => void;
	readonly onSettings: WorkbenchHostActionCallback | undefined;
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
	onSettings,
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
		const registrations = [
			...registerWorkbenchProjectWorkspaceActions({
				navigation,
				primaryWorkspaceName,
				projectWorkspaceRuntime,
				scope,
				typedRuntime,
				workspaceCopySuffix,
				workspaceTerminalTitle,
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
		openSearch,
		primaryWorkspaceName,
		projectWorkspaceRuntime,
		projectWorkspaceState,
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
				case 'settings':
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
