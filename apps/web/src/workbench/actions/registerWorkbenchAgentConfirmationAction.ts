import {
	defineAction,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import type { WorkbenchAgentConfirmations } from '../types/workbench-agent-confirmations';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	workbenchAgentConfirmationChooseInput,
} from './workbench-action-ids';

interface RegisterWorkbenchAgentConfirmationActionOptions {
	/** Absent where no Server answers Agent confirmations, so no surface offers the Action. */
	readonly confirmations: WorkbenchAgentConfirmations | undefined;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

/**
 * Allow confirmations: says whether Orchbee answers the routine confirmations of the Agent in one
 * terminal: never, for this run, or for every Agent of its type. `no` answers an offer and ends
 * offers for every Agent. The Server decides and owns the value; this Client shows only what the
 * Server's row reports afterwards, so a request that was not applied changes nothing here.
 */
export function registerWorkbenchAgentConfirmationAction({
	confirmations,
	scope,
	typedRuntime,
}: RegisterWorkbenchAgentConfirmationActionOptions): ActionDisposable {
	return typedRuntime.register(
		scope,
		defineAction({
			category: { id: 'terminal', title: workbenchMessages.actionCategoryTerminal },
			execution: {
				// One answer at a time for a terminal: a second press waits for the Server's row.
				concurrency: 'reject-duplicate',
				concurrencyKey: (input) => `agent-confirmation:${input.presentationId}`,
				mode: 'foreground',
			},
			getAvailability: (_context, input) => {
				if (confirmations === undefined) return { kind: 'hidden' };
				return confirmations.getSnapshot().has(input.presentationId)
					? { kind: 'enabled' }
					: { kind: 'disabled' };
			},
			handler: {
				execute: async (context, input) => {
					if (context.signal.aborted) return { kind: 'cancelled' };
					if (confirmations?.getSnapshot().has(input.presentationId) !== true) {
						return { code: 'agent-confirmation-target-unavailable', kind: 'blocked' };
					}
					// The request is the commit: the Server may apply it although its answer is lost.
					context.markCommitted();
					const applied =
						input.answer === 'no'
							? await confirmations.decline(input.presentationId)
							: await confirmations.choose(input.presentationId, input.answer);
					return applied
						? { kind: 'completed' }
						: { failure: { code: 'agent-confirmation-not-changed' }, kind: 'failed' };
				},
			},
			id: WORKBENCH_ACTION_IDS.chooseAgentConfirmation,
			input: workbenchAgentConfirmationChooseInput,
			title: workbenchMessages.agentConfirmationMenu,
		}),
	);
}
