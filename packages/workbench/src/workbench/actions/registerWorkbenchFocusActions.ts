import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import type {
	WorkbenchFocusIntent,
	WorkbenchFocusResolution,
} from '../model/resolveWorkbenchFocusTarget';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

type WorkbenchFocusActionId =
	| typeof WORKBENCH_ACTION_IDS.focusActiveTerminal
	| typeof WORKBENCH_ACTION_IDS.focusNextRegion
	| typeof WORKBENCH_ACTION_IDS.focusPreviousRegion
	| typeof WORKBENCH_ACTION_IDS.focusSidebar
	| typeof WORKBENCH_ACTION_IDS.focusTabStrip;

interface WorkbenchFocusRouter {
	readonly focus: (intent: WorkbenchFocusIntent) => WorkbenchFocusResolution;
	readonly getResolution: (intent: WorkbenchFocusIntent) => WorkbenchFocusResolution;
}

interface RegisterWorkbenchFocusActionsOptions {
	readonly focusRouter: WorkbenchFocusRouter;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

const FOCUS_ACTIONS: readonly {
	readonly id: WorkbenchFocusActionId;
	readonly intent: WorkbenchFocusIntent;
	readonly title:
		| typeof workbenchMessages.focusActiveTerminal
		| typeof workbenchMessages.focusNextRegion
		| typeof workbenchMessages.focusPreviousRegion
		| typeof workbenchMessages.focusSidebar
		| typeof workbenchMessages.focusTabStrip;
}[] = [
	{
		id: WORKBENCH_ACTION_IDS.focusActiveTerminal,
		intent: { kind: 'direct', region: 'terminal' },
		title: workbenchMessages.focusActiveTerminal,
	},
	{
		id: WORKBENCH_ACTION_IDS.focusTabStrip,
		intent: { kind: 'direct', region: 'tab-strip' },
		title: workbenchMessages.focusTabStrip,
	},
	{
		id: WORKBENCH_ACTION_IDS.focusSidebar,
		intent: { kind: 'direct', region: 'sidebar' },
		title: workbenchMessages.focusSidebar,
	},
	{
		id: WORKBENCH_ACTION_IDS.focusNextRegion,
		intent: { direction: 'next', kind: 'cycle' },
		title: workbenchMessages.focusNextRegion,
	},
	{
		id: WORKBENCH_ACTION_IDS.focusPreviousRegion,
		intent: { direction: 'previous', kind: 'cycle' },
		title: workbenchMessages.focusPreviousRegion,
	},
];

export function registerWorkbenchFocusActions({
	focusRouter,
	scope,
	typedRuntime,
}: RegisterWorkbenchFocusActionsOptions): readonly ActionDisposable[] {
	return FOCUS_ACTIONS.map(({ id, intent, title }) =>
		typedRuntime.register(
			scope,
			defineAction({
				category: {
					id: 'navigation',
					title: workbenchMessages.actionCategoryNavigation,
				},
				execution: {
					concurrency: 'replace-latest',
					concurrencyKey: () => 'workbench-focus-routing',
					mode: 'foreground',
				},
				getAvailability: () =>
					focusRouter.getResolution(intent).kind === 'unavailable'
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				handler: {
					execute: (context) => {
						const resolution = focusRouter.getResolution(intent);
						if (resolution.kind === 'unavailable') {
							return { code: `focus-${resolution.reason}`, kind: 'blocked' };
						}
						if (resolution.kind === 'unchanged') return { kind: 'unchanged' };
						context.markCommitted();
						const committed = focusRouter.focus(intent);
						return committed.kind === 'target'
							? { kind: 'completed' }
							: { code: 'focus-target-changed', kind: 'blocked' };
					},
				},
				id,
				input: noActionInput,
				title,
			}),
		),
	);
}
