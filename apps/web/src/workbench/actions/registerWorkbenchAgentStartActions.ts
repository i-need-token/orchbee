import {
	defineAction,
	noActionInput,
	type ActionAvailability,
	type ActionDisposable,
	type ActionInvocation,
	type ActionResult,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { agentStartMessages, workbenchMessages } from '@orchbee/i18n';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import {
	WORKBENCH_AGENT_START_PLACES,
	type WorkbenchAgentCatalogSource,
	type WorkbenchAgentStartPlace,
} from '../types/workbench-agent-start';
import type { WorkbenchAuthority } from '../types/workbench-authority';
import type { WorkbenchTerminalHandover } from '../types/workbench-focus-routing';
import { createSharedTab } from './workbench-shared-tab-creation';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	workbenchAgentStartInput,
} from './workbench-action-ids';

interface RegisterWorkbenchAgentStartActionsOptions {
	readonly agentCatalog: WorkbenchAgentCatalogSource;
	readonly authority: WorkbenchAuthority;
	readonly beginTerminalHandover: () => WorkbenchTerminalHandover;
	/** Opens the dialog offering the places a start can use now. */
	readonly openAgentStart: (places: readonly WorkbenchAgentStartPlace[]) => void;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

const HIDDEN = { kind: 'hidden' } as const;

/** The existing Action each place delegates to, which owns its availability and focus. */
const SPLIT_ACTIONS = {
	'split-down': WORKBENCH_ACTION_IDS.splitActivePanelBelow,
	'split-right': WORKBENCH_ACTION_IDS.splitActivePanelRight,
} as const;

function withoutValue<Value>(result: ActionResult<Value>): ActionResult<void> {
	return result.kind === 'completed' || result.kind === 'unchanged'
		? { kind: result.kind }
		: result;
}

/**
 * Start Agent and its start, the TUI's `orchbee.native.agent.start`. The dialog opens only while the
 * Server says this Session can start an Agent. A start names an Agent and a place: New Tab and the
 * splits create a terminal whose first command the Server delivers, and New Worktree… runs New
 * Worktree from the focused Panel: its form opens with a branch proposed for the Agent, and once it is
 * submitted the Server starts the Agent in the Workspace the Worktree opens; cancelling it starts
 * nothing. Each place keeps the availability and focus handover of the command it delegates to.
 */
export function registerWorkbenchAgentStartActions({
	agentCatalog,
	authority,
	beginTerminalHandover,
	openAgentStart,
	projectWorkspaceRuntime,
	scope,
	typedRuntime,
}: RegisterWorkbenchAgentStartActionsOptions): readonly ActionDisposable[] {
	const category = { id: 'terminal', title: workbenchMessages.actionCategoryTerminal };
	const startable = (): boolean => agentCatalog.getSnapshot()?.startable === true;
	const placeAvailability = (
		place: WorkbenchAgentStartPlace,
		invocation: ActionInvocation,
	): ActionAvailability => {
		switch (place) {
			case 'new-tab':
				return projectWorkspaceRuntime.getActiveTerminalContext() === null
					? { kind: 'disabled', reason: workbenchMessages.workspaceUnavailableRequired }
					: { kind: 'enabled' };
			case 'split-down':
			case 'split-right':
				return (
					typedRuntime.resolveAction(SPLIT_ACTIONS[place], undefined, invocation)?.availability ??
					HIDDEN
				);
			case 'new-worktree':
				return authority.worktrees === undefined
					? HIDDEN
					: (typedRuntime.resolveAction(
							WORKBENCH_ACTION_IDS.createFocusedPanelWorktree,
							undefined,
							invocation,
						)?.availability ?? HIDDEN);
		}
	};

	return [
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'agent-start-open',
					mode: 'foreground',
				},
				getAvailability: () => (startable() ? { kind: 'enabled' } : HIDDEN),
				handler: {
					execute: (context) => {
						context.markCommitted();
						const invocation = { scope, source: context.source };
						openAgentStart(
							WORKBENCH_AGENT_START_PLACES.filter(
								(place) => placeAvailability(place, invocation).kind === 'enabled',
							),
						);
						return { kind: 'completed' };
					},
				},
				id: WORKBENCH_ACTION_IDS.openAgentStart,
				input: noActionInput,
				title: agentStartMessages.title,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'reject-duplicate',
					concurrencyKey: (input) => `${input.place}:${input.providerId}`,
					mode: 'foreground',
				},
				getAvailability: (context, input) =>
					startable() ? placeAvailability(input.place, context) : HIDDEN,
				handler: {
					execute: async (context, { place, providerId }) => {
						const invocation = { scope, source: context.source };
						switch (place) {
							case 'new-tab':
								return await createSharedTab(
									context,
									{ agentStart: providerId, placement: null },
									{ authority, beginTerminalHandover, runtime: projectWorkspaceRuntime },
								);
							case 'split-down':
							case 'split-right':
								return await typedRuntime.execute(
									SPLIT_ACTIONS[place],
									{ agentStart: providerId },
									invocation,
								).completion;
							case 'new-worktree':
								return withoutValue(
									await typedRuntime.execute(
										WORKBENCH_ACTION_IDS.createFocusedPanelWorktree,
										{ agentStart: providerId },
										invocation,
									).completion,
								);
						}
					},
				},
				id: WORKBENCH_ACTION_IDS.startAgent,
				input: workbenchAgentStartInput,
				title: agentStartMessages.title,
			}),
		),
	];
}
