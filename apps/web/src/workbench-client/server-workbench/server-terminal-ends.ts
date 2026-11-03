import type {
	TerminalPresentationLifecycle,
	WorkbenchPanelNode,
	WorkbenchPresentation,
	WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import type { ServerTerminalSettlement } from '../server-connection';
import { findServerTerminalBinding, serverTerminalBindingKey } from './server-terminal-binding';

type EndedTerminalLifecycle = Extract<
	TerminalPresentationLifecycle,
	{ readonly kind: 'exited' | 'failed' }
>;

/** How this window saw a terminal end that the Server still publishes as bound. */
interface ObservedServerTerminalEnd {
	/** The binding that ended, as {@link serverTerminalBindingKey} names it. */
	readonly binding: string;
	readonly lifecycle: EndedTerminalLifecycle;
}

/** Every end this window saw, by the Presentation that showed it. */
export type ObservedServerTerminalEnds = ReadonlyMap<string, ObservedServerTerminalEnd>;

/** The TUI's bound, within which the Server's Close removes a Panel whose shell exited cleanly. */
const NORMAL_EXIT_HOLD_MILLISECONDS = 1_000;

/**
 * Keeps what this window saw end while each Presentation stays bound to the terminal that ended,
 * because the Server keeps an ended terminal bound until a person closes it and publishes nothing.
 * As the TUI holds a pane whose shell exited cleanly, the window sees exit code 0 of all a Panel
 * holds only after a second: the Server closes that Panel itself, so its removal comes first.
 */
export class ServerTerminalEndObserver {
	readonly #ends = new Map<string, ObservedServerTerminalEnd>();
	readonly #holds = new Map<
		string,
		{ readonly binding: string; readonly timer: ReturnType<typeof setTimeout> }
	>();
	readonly #onChange: () => void;
	readonly #snapshot: () => WorkbenchSnapshot;
	#disposed = false;

	/** `onChange` hears each end once the window sees it, so the snapshot can be projected again. */
	public constructor(snapshot: () => WorkbenchSnapshot, onChange: () => void) {
		this.#onChange = onChange;
		this.#snapshot = snapshot;
	}

	public get ends(): ObservedServerTerminalEnds {
		return this.#ends;
	}

	/** Hears an end the projection of `binding` saw while it presented `presentationId`. */
	public observe(
		presentationId: string,
		binding: Parameters<typeof serverTerminalBindingKey>[0],
		lifecycle: EndedTerminalLifecycle,
	): void {
		const ended = serverTerminalBindingKey(binding);
		if (
			!this.#binds(presentationId, ended) ||
			this.#ends.get(presentationId)?.binding === ended ||
			this.#holds.get(presentationId)?.binding === ended
		) {
			return;
		}
		if (
			lifecycle.kind === 'exited' &&
			lifecycle.exitCode === 0 &&
			holdsAlone(this.#snapshot(), presentationId)
		) {
			this.#hold(presentationId, ended, lifecycle);
			return;
		}
		this.#see(presentationId, ended, lifecycle);
	}

	/** Forgets each end and hold whose Presentation `snapshot` removed or bound to another terminal. */
	public prune(snapshot: WorkbenchSnapshot): void {
		for (const [presentationId, end] of this.#ends) {
			if (boundTerminal(snapshot, presentationId) !== end.binding) {
				this.#ends.delete(presentationId);
			}
		}
		for (const [presentationId, hold] of this.#holds) {
			if (boundTerminal(snapshot, presentationId) !== hold.binding) {
				this.#release(presentationId);
			}
		}
	}

	public dispose(): void {
		this.#disposed = true;
		for (const { timer } of this.#holds.values()) {
			clearTimeout(timer);
		}
		this.#holds.clear();
	}

	#binds(presentationId: string, ended: string): boolean {
		return !this.#disposed && boundTerminal(this.#snapshot(), presentationId) === ended;
	}

	#hold(presentationId: string, ended: string, lifecycle: EndedTerminalLifecycle): void {
		this.#release(presentationId);
		const timer = setTimeout(() => {
			this.#holds.delete(presentationId);
			this.#see(presentationId, ended, lifecycle);
		}, NORMAL_EXIT_HOLD_MILLISECONDS);
		this.#holds.set(presentationId, { binding: ended, timer });
	}

	#release(presentationId: string): void {
		const hold = this.#holds.get(presentationId);
		if (hold === undefined) return;
		clearTimeout(hold.timer);
		this.#holds.delete(presentationId);
	}

	#see(presentationId: string, ended: string, lifecycle: EndedTerminalLifecycle): void {
		if (!this.#binds(presentationId, ended)) return;
		this.#ends.set(presentationId, { binding: ended, lifecycle });
		this.#onChange();
	}
}

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

function boundTerminal(snapshot: WorkbenchSnapshot, presentationId: string): string | null {
	const binding = findServerTerminalBinding(snapshot, presentationId);
	return binding === null ? null : serverTerminalBindingKey(binding);
}

/** Whether the Presentation is all its Panel holds, as the Server requires to close it on exit. */
function holdsAlone(snapshot: WorkbenchSnapshot, presentationId: string): boolean {
	return snapshot.terminalContexts.some((context) =>
		context.tabs.some((tab) =>
			panelsOf(tab.root).some(
				(panel) =>
					panel.primaryPresentation.presentationId === presentationId &&
					panel.childPresentations.length === 0,
			),
		),
	);
}

function panelsOf(
	node: WorkbenchPanelNode,
): readonly Extract<WorkbenchPanelNode, { readonly kind: 'panel' }>[] {
	return node.kind === 'panel' ? [node] : node.children.flatMap((slot) => panelsOf(slot.node));
}
