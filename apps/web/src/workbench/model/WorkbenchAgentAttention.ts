import { EMPTY_WORKBENCH_AGENT_SNAPSHOT } from '../constants/empty-workbench-agent-snapshot';
import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAgentSource,
} from '../types/workbench-agent-observation';

export interface WorkbenchAgentAttentionSnapshot {
	readonly observation: WorkbenchAgentSnapshot;
	/** Display-only projection; notification inputs stay in observation. */
	readonly presentation: WorkbenchAgentSnapshot;
	/** Presentations whose current Agent state this Client has not shown its user. */
	readonly unread: ReadonlySet<string>;
}

/** One exact observed state: a newer sequence or a replacement terminal is another version. */
function version(observation: WorkbenchAgentObservation): string {
	return `${observation.binding}\0${observation.sequence}`;
}

/**
 * Client-local acknowledgement of the Server's terminal Agent states, as the TUI keeps it: the
 * first complete observation is what the person already had in front of them, and every later
 * version stays unread until a focused window paints it. It never writes an Agent state.
 */
export class WorkbenchAgentAttention {
	readonly #source: WorkbenchAgentSource | undefined;
	readonly #listeners = new Set<() => void>();
	#unsubscribe: (() => void) | undefined;
	#snapshot: WorkbenchAgentAttentionSnapshot;
	/** The version last shown in each Presentation; null until the first complete observation. */
	#seen: Map<string, string> | null = null;
	/** Completions the person typed into afterwards, shown as idle until the state changes. */
	readonly #edited = new Map<string, string>();

	constructor(source: WorkbenchAgentSource | undefined) {
		this.#source = source;
		const observation = source?.getSnapshot() ?? EMPTY_WORKBENCH_AGENT_SNAPSHOT;
		this.#snapshot = { observation, presentation: observation, unread: new Set() };
		this.#advance(observation);
	}

	getSnapshot = (): WorkbenchAgentAttentionSnapshot => this.#snapshot;

	subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		if (this.#listeners.size === 1) {
			this.#unsubscribe = this.#source?.subscribe(this.#observe);
			this.#observe();
		}
		return () => {
			this.#listeners.delete(listener);
			if (this.#listeners.size === 0) {
				this.#unsubscribe?.();
				this.#unsubscribe = undefined;
			}
		};
	};

	#observe = (): void => {
		const observation = this.#source?.getSnapshot() ?? EMPTY_WORKBENCH_AGENT_SNAPSHOT;
		if (observation === this.#snapshot.observation) return;
		this.#advance(observation);
		this.#publish(observation);
	};

	#advance(observation: WorkbenchAgentSnapshot): void {
		if (this.#seen === null) {
			if (observation.complete) {
				this.#seen = new Map();
				for (const [id, current] of observation.presentations) this.#seen.set(id, version(current));
			}
		} else {
			// A Presentation that loses its Agent keeps no acknowledgement for a later one.
			for (const id of this.#seen.keys()) {
				if (!observation.presentations.has(id)) this.#seen.delete(id);
			}
		}
		for (const [id, edited] of this.#edited) {
			const current = observation.presentations.get(id);
			if (current === undefined || version(current) !== edited) this.#edited.delete(id);
		}
	}

	/** A stale paint or an unfocused window cannot acknowledge a state. */
	acknowledge(
		painted: WorkbenchAgentAttentionSnapshot,
		visiblePresentations: ReadonlySet<string>,
		focused: boolean,
	): void {
		const seen = this.#seen;
		if (
			!focused ||
			seen === null ||
			painted !== this.#snapshot ||
			this.#source?.getSnapshot() !== painted.observation
		)
			return;
		let changed = false;
		for (const id of visiblePresentations) {
			const current = painted.observation.presentations.get(id);
			if (current === undefined || seen.get(id) === version(current)) continue;
			seen.set(id, version(current));
			changed = true;
		}
		if (changed) this.#publish(painted.observation);
	}

	/** The terminal adapter invokes this only after it accepts text, paste or an editing key. */
	terminalEdited = (presentationId: string): void => {
		this.#observe();
		const current = this.#snapshot.observation.presentations.get(presentationId);
		if (current?.state !== 'done' || this.#edited.get(presentationId) === version(current)) return;
		this.#edited.set(presentationId, version(current));
		this.#publish(this.#snapshot.observation);
	};

	#publish(observation: WorkbenchAgentSnapshot): void {
		const unread = new Set<string>();
		if (this.#seen !== null) {
			for (const [id, current] of observation.presentations) {
				if (this.#seen.get(id) !== version(current)) unread.add(id);
			}
		}
		let presentation = observation;
		if (this.#edited.size > 0) {
			const presentations = new Map<string, WorkbenchAgentObservation>();
			for (const [id, current] of observation.presentations) {
				presentations.set(
					id,
					this.#edited.get(id) === version(current) ? { ...current, state: 'idle' } : current,
				);
			}
			presentation = { complete: observation.complete, presentations };
		}
		this.#snapshot = { observation, presentation, unread };
		for (const listener of this.#listeners) listener();
	}
}
