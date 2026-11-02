import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { projectedTerminalMessages } from '@orchbee/i18n';

import type { ProjectedTerminalTakeControlOutcome } from '../terminal-projection';
import {
	PROJECTED_TERMINAL_ACTION_IDS,
	type ProjectedTerminalActionCatalog,
} from './projected-terminal-action-ids';

/** What a surface says about another Client holding its terminal's input and size. */
export type ProjectedTerminalControlPresentation = 'none' | 'taking-control' | 'view-only';

export interface ProjectedTerminalControlTarget {
	readonly controlPresentation: ProjectedTerminalControlPresentation;
	takeControl(): Promise<ProjectedTerminalTakeControlOutcome>;
}

/** As in the TUI, Take control exists only while another Client holds the terminal. */
export function registerProjectedTerminalControlAction(
	typedRuntime: TypedActionRuntime<ProjectedTerminalActionCatalog>,
	scope: ActionScope,
	control: ProjectedTerminalControlTarget,
): ActionDisposable {
	return typedRuntime.register(
		scope,
		defineAction({
			category: { id: 'terminal', title: projectedTerminalMessages.actionCategory },
			execution: {
				concurrency: 'reject-duplicate',
				concurrencyKey: () => 'terminal-control-take',
				mode: 'foreground',
			},
			getAvailability: () => {
				switch (control.controlPresentation) {
					case 'view-only':
						return { kind: 'enabled' };
					case 'taking-control':
						return { kind: 'disabled', reason: projectedTerminalMessages.controlTaking };
					case 'none':
						return { kind: 'hidden' };
				}
			},
			handler: {
				// The request cannot be withdrawn once sent, so cancellation does not stop it.
				execute: async (context) => {
					const outcome = await control.takeControl();
					if (outcome === 'acquired') {
						context.markCommitted();
						return { kind: 'completed' };
					}
					return {
						failure: {
							code:
								outcome === 'unknown'
									? 'terminal-control-outcome-unknown'
									: 'terminal-control-refused',
						},
						kind: 'failed',
					};
				},
			},
			id: PROJECTED_TERMINAL_ACTION_IDS.takeControl,
			input: noActionInput,
			title: projectedTerminalMessages.takeControlAction,
		}),
	);
}
