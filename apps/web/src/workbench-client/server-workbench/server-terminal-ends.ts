import type {
	TerminalPresentationLifecycle,
	WorkbenchPresentation,
} from '@orchbee/workbench-client-protocol';

import type { ServerTerminalSettlement } from '../server-connection';
import { serverTerminalBindingKey } from './server-terminal-binding';

type EndedTerminalLifecycle = Extract<
	TerminalPresentationLifecycle,
	{ readonly kind: 'exited' | 'failed' }
>;

/** How this window saw a terminal end that the Server still publishes as bound. */
export interface ObservedServerTerminalEnd {
	/** The binding that ended, as {@link serverTerminalBindingKey} names it. */
	readonly binding: string;
	readonly lifecycle: EndedTerminalLifecycle;
}

/** Every end this window saw, by the Presentation that showed it. */
export type ObservedServerTerminalEnds = ReadonlyMap<string, ObservedServerTerminalEnd>;

/** The end this window saw of the terminal a Presentation is still bound to, if it saw one. */
export function observedServerTerminalEnd(
	presentation: WorkbenchPresentation,
	observedEnds: ObservedServerTerminalEnds,
): EndedTerminalLifecycle | null {
	const end = observedEnds.get(presentation.presentationId);
	return end === undefined ||
		presentation.binding === null ||
		end.binding !== serverTerminalBindingKey(presentation.binding)
		? null
		: end.lifecycle;
}

/** What a Host settlement leaves of a terminal: its output, and nothing running to end. */
export function settledServerTerminalLifecycle(
	settlement: ServerTerminalSettlement,
): EndedTerminalLifecycle {
	if (settlement.kind === 'terminal-failed') {
		return { failure: { code: settlement.event.failure.kind, retryable: false }, kind: 'failed' };
	}
	const { exit } = settlement.event;
	return {
		exitCode: exit.kind === 'exit-code' ? exit.code : null,
		kind: 'exited',
		signal: exit.kind === 'signal' ? String(exit.signal) : null,
	};
}
