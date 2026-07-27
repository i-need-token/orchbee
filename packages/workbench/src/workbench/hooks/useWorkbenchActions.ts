import {
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionCatalogId,
	type ActionExecution,
	type ActionInputFor,
	type ActionInvocationSource,
	type ActionOutputFor,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';
import type { TabStripOrientation } from '@orchbee/tab-strip';
import * as React from 'react';

import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchActionId,
	workbenchTabStripOrientationInput,
	type WorkbenchTerminalCreateInput,
	workbenchTerminalCreateInput,
} from '../actions/workbench-action-ids';
import type {
	WorkbenchHostActionCallback,
	WorkbenchTerminalCreateCallback,
} from '../types/workbench-host-actions';

interface UseWorkbenchActionsOptions {
	readonly isSidebarVisible: boolean;
	readonly onNewTerminal: WorkbenchTerminalCreateCallback | undefined;
	readonly onSearch: WorkbenchHostActionCallback | undefined;
	readonly onSettings: WorkbenchHostActionCallback | undefined;
	readonly runtime: ActionRuntime;
	readonly setTabStripOrientation: (orientation: TabStripOrientation) => void;
	readonly tabStripOrientation: TabStripOrientation;
	readonly toggleSidebar: () => void;
}

interface WorkbenchActions {
	readonly invoke: <Id extends ActionCatalogId<WorkbenchActionCatalog>>(
		actionId: Id,
		input: ActionInputFor<WorkbenchActionCatalog, Id>,
		source: ActionInvocationSource,
	) => ActionExecution<ActionOutputFor<WorkbenchActionCatalog, Id>>;
	readonly invokeIfEnabled: (
		actionId: WorkbenchActionId,
		source: ActionInvocationSource,
	) => boolean;
	readonly scope: ActionScope;
}

export function useWorkbenchActions({
	isSidebarVisible,
	onNewTerminal,
	onSearch,
	onSettings,
	runtime,
	setTabStripOrientation,
	tabStripOrientation,
	toggleSidebar,
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
					getAvailability: availabilityFor(onSearch),
					handler: handlerFor(onSearch),
					id: WORKBENCH_ACTION_IDS.openSearch,
					input: noActionInput,
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
		return () => {
			for (const registration of registrations) {
				registration.dispose();
			}
		};
	}, [
		isSidebarVisible,
		onNewTerminal,
		onSearch,
		onSettings,
		setTabStripOrientation,
		scope,
		tabStripOrientation,
		toggleSidebar,
		typedRuntime,
	]);

	const invoke = React.useCallback(
		<Id extends ActionCatalogId<WorkbenchActionCatalog>>(
			actionId: Id,
			input: ActionInputFor<WorkbenchActionCatalog, Id>,
			source: ActionInvocationSource,
		) => typedRuntime.execute(actionId, input, { scope, source }),
		[scope, typedRuntime],
	);
	const invokeIfEnabled = React.useCallback(
		(actionId: WorkbenchActionId, source: ActionInvocationSource) => {
			const invocation = { scope, source };
			switch (actionId) {
				case WORKBENCH_ACTION_IDS.createTerminal: {
					const input: WorkbenchTerminalCreateInput = { groupId: null };
					if (
						typedRuntime.resolveAction(actionId, input, invocation)?.availability.kind !== 'enabled'
					) {
						return false;
					}
					typedRuntime.execute(actionId, input, invocation);
					return true;
				}
				case WORKBENCH_ACTION_IDS.openSearch:
				case WORKBENCH_ACTION_IDS.openSettings:
				case WORKBENCH_ACTION_IDS.toggleSidebar:
					if (
						typedRuntime.resolveAction(actionId, undefined, invocation)?.availability.kind !==
						'enabled'
					) {
						return false;
					}
					typedRuntime.execute(actionId, undefined, invocation);
					return true;
				case WORKBENCH_ACTION_IDS.setTabStripOrientation:
					return false;
			}
		},
		[scope, typedRuntime],
	);

	return { invoke, invokeIfEnabled, scope };
}
