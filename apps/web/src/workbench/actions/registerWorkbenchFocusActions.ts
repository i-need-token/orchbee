import {
	defineAction,
	noActionInput,
	type ActionAvailabilityContext,
	type ActionDisposable,
	type ActionMessageDescriptor,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import type {
	WorkbenchFocusIntent,
	WorkbenchFocusRegion,
	WorkbenchFocusResolution,
} from '../model/resolveWorkbenchFocusTarget';
import type { WorkbenchFocusRouting } from '../types/workbench-focus-routing';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

type WorkbenchFocusActionId =
	| typeof WORKBENCH_ACTION_IDS.focusActiveTerminal
	| typeof WORKBENCH_ACTION_IDS.focusNextRegion
	| typeof WORKBENCH_ACTION_IDS.focusPreviousRegion
	| typeof WORKBENCH_ACTION_IDS.focusSidebar
	| typeof WORKBENCH_ACTION_IDS.focusTabStrip;

type WorkbenchFocusRouter = Pick<WorkbenchFocusRouting, 'focus' | 'getResolution'>;

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
				getAvailability: (context) => {
					const resolution = focusRouter.getResolution(intent, focusInvocation(context));
					return resolution.kind === 'unavailable'
						? { kind: 'disabled', reason: focusUnavailableReason(resolution.reason, intent) }
						: { kind: 'enabled' };
				},
				handler: {
					execute: (context) => {
						const invocation = focusInvocation(context);
						const resolution = focusRouter.getResolution(intent, invocation);
						if (resolution.kind === 'unavailable') {
							return { code: `focus-${resolution.reason}`, kind: 'blocked' };
						}
						if (resolution.kind === 'unchanged') return { kind: 'unchanged' };
						context.markCommitted();
						const committed = focusRouter.focus(intent, invocation);
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

/** A Search row acts once the Search has closed, so the Search never blocks its own rows. */
function focusInvocation(context: Pick<ActionAvailabilityContext, 'source'>) {
	return { fromCommandSurface: context.source === 'command-surface' };
}

const HIDDEN_REGION_REASON = {
	sidebar: workbenchMessages.focusUnavailableSidebarHidden,
	'tab-strip': workbenchMessages.focusUnavailableTabStripHidden,
	terminal: workbenchMessages.focusUnavailableNoTerminal,
} as const satisfies Readonly<Record<WorkbenchFocusRegion, ActionMessageDescriptor>>;

/** Names what stops the focus move, so a disabled row says why. */
function focusUnavailableReason(
	reason: Extract<WorkbenchFocusResolution, { kind: 'unavailable' }>['reason'],
	intent: WorkbenchFocusIntent,
): ActionMessageDescriptor {
	if (reason === 'exclusive-surface-active') return workbenchMessages.focusUnavailableSurfaceOpen;
	// A named region is unavailable only while it is not shown.
	if (intent.kind === 'direct') return HIDDEN_REGION_REASON[intent.region];
	return reason === 'no-different-region'
		? workbenchMessages.focusUnavailableNoOtherRegion
		: workbenchMessages.focusUnavailableNoRegion;
}
