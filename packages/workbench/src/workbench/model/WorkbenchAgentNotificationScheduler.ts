import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAgentState,
} from '../types/workbench-agent-observation';

export interface WorkbenchAgentNotificationCandidate {
	/** `done` follows a quiet transition and does not prove a successful turn. */
	readonly kind: 'done' | 'request';
	readonly observation: WorkbenchAgentObservation;
	readonly presentationId: string;
}

interface PendingNotification {
	readonly kind: WorkbenchAgentNotificationCandidate['kind'];
	readonly observation: WorkbenchAgentObservation;
	readonly timer: ReturnType<typeof setTimeout>;
}

/** Long enough for the window to paint a visible terminal, which then needs no notification. */
const NOTIFICATION_DELAY_MILLISECONDS = 1000;

/**
 * This Client's automatic Agent notifications, under the TUI's rules: a first observation, a
 * replacement terminal or a changed Agent label is a silent baseline; only `working` becoming
 * `idle` or `done`, or `idle` or `working` becoming `blocked`, schedules one. Any later state
 * change cancels it, and so does painting that exact state in front of the person.
 */
export class WorkbenchAgentNotificationScheduler {
	readonly #deliver: (candidate: WorkbenchAgentNotificationCandidate) => void;
	readonly #observed = new Map<string, WorkbenchAgentObservation>();
	readonly #pending = new Map<string, PendingNotification>();

	constructor(deliver: (candidate: WorkbenchAgentNotificationCandidate) => void) {
		this.#deliver = deliver;
	}

	observe(snapshot: WorkbenchAgentSnapshot): void {
		for (const id of this.#observed.keys()) {
			if (snapshot.presentations.has(id)) continue;
			this.#observed.delete(id);
			this.#cancel(id);
		}
		for (const [id, current] of snapshot.presentations) {
			const previous = this.#observed.get(id);
			this.#observed.set(id, current);
			if (previous?.sequence === current.sequence && previous.binding === current.binding) continue;
			this.#cancel(id);
			if (previous === undefined || previous.binding !== current.binding) continue;
			if (previous.agent !== current.agent) continue;
			const kind = transitionKind(previous.state, current.state);
			if (kind !== null) this.#schedule(id, kind, current);
		}
	}

	/** The exact states a window painted where the person can see them. */
	painted = (snapshot: WorkbenchAgentSnapshot, presentationIds: ReadonlySet<string>): void => {
		for (const id of presentationIds) {
			const pending = this.#pending.get(id);
			const shown = snapshot.presentations.get(id);
			if (pending !== undefined && shown !== undefined && sameVersion(pending.observation, shown)) {
				this.#cancel(id);
			}
		}
	};

	/** Forgets every observation, so the next one is a baseline again. */
	reset(): void {
		for (const pending of this.#pending.values()) clearTimeout(pending.timer);
		this.#pending.clear();
		this.#observed.clear();
	}

	#schedule(
		id: string,
		kind: WorkbenchAgentNotificationCandidate['kind'],
		observation: WorkbenchAgentObservation,
	): void {
		const timer = setTimeout(() => {
			if (this.#pending.get(id)?.timer !== timer) return;
			this.#pending.delete(id);
			const current = this.#observed.get(id);
			if (current === undefined || !sameVersion(current, observation)) return;
			this.#deliver({ kind, observation, presentationId: id });
		}, NOTIFICATION_DELAY_MILLISECONDS);
		this.#pending.set(id, { kind, observation, timer });
	}

	#cancel(id: string): void {
		const pending = this.#pending.get(id);
		if (pending === undefined) return;
		clearTimeout(pending.timer);
		this.#pending.delete(id);
	}
}

function transitionKind(
	previous: WorkbenchAgentState,
	current: WorkbenchAgentState,
): WorkbenchAgentNotificationCandidate['kind'] | null {
	if (previous === 'working' && (current === 'idle' || current === 'done')) return 'done';
	if ((previous === 'idle' || previous === 'working') && current === 'blocked') return 'request';
	return null;
}

function sameVersion(left: WorkbenchAgentObservation, right: WorkbenchAgentObservation): boolean {
	return left.binding === right.binding && left.sequence === right.sequence;
}
