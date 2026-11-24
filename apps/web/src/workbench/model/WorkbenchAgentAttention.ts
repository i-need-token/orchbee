import { resolveWorkbenchAgentDisplayState } from '../utils/resolveWorkbenchAgentDisplayState';
import { EMPTY_WORKBENCH_AGENT_SNAPSHOT } from '../constants/empty-workbench-agent-snapshot';
import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAgentSource,
	WorkbenchAgentState,
} from '../types/workbench-agent-observation';

export interface WorkbenchAgentAttentionSnapshot {
	readonly observation: WorkbenchAgentSnapshot;
	/** Display-only projection; notification inputs stay in observation. */
	readonly presentation: WorkbenchAgentSnapshot;
	/** Presentations whose Agent has something new this Client has not shown its user. */
	readonly unread: ReadonlySet<string>;
}

/** One Presentation's last folded Agent version and whether this Client has shown it. */
interface Acknowledgement {
	readonly observation: WorkbenchAgentObservation;
	readonly seen: boolean;
}

/** One exact observed state: a newer sequence or a replacement terminal is another version. */
function version(observation: WorkbenchAgentObservation): string {
	return `${observation.binding}\0${observation.sequence}`;
}

/**
 * Whether a new Agent version starts seen, as the TUI decides it. A finished turn, a request or a
 * failure hands the person something new, so it starts unseen. Entering work, starting, or an
 * unrecognised screen shows nothing new and keeps the previous acknowledgement, so a finished turn
 * nobody has read stays unread while the Agent works again. `previous` is the same Agent's older
 * version in the same terminal, absent when this Client has none. An idle version after an idle
 * one means versions went unread in between, which may hold a whole turn, so it counts as finished.
 */
function versionStartsSeen(
	previous: Acknowledgement | undefined,
	current: WorkbenchAgentState,
): boolean {
	const before = previous?.observation.state;
	const handedOver =
		current === 'done' ||
		current === 'error' ||
		current === 'blocked' ||
		(current === 'idle' && (before === 'working' || before === 'idle'));
	return !handedOver && (previous?.seen ?? true);
}

/**
 * Client-local acknowledgement of the Server's terminal Agent states, as the TUI keeps it: the
 * first complete observation is what the person already had in front of them, and a later version
 * that hands them something new stays unread until a focused window paints it. It never writes an
 * Agent state.
 */
export class WorkbenchAgentAttention {
	readonly #source: WorkbenchAgentSource | undefined;
	readonly #listeners = new Set<() => void>();
	#unsubscribe: (() => void) | undefined;
	#snapshot: WorkbenchAgentAttentionSnapshot;
	/** Each Presentation's last folded version; null until the first complete observation. */
	#acknowledgements: Map<string, Acknowledgement> | null = null;

	constructor(source: WorkbenchAgentSource | undefined) {
		this.#source = source;
		const observation = source?.getSnapshot() ?? EMPTY_WORKBENCH_AGENT_SNAPSHOT;
		this.#snapshot = {
			observation,
			presentation: projectObservation(observation),
			unread: new Set(),
		};
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
		const acknowledgements = this.#acknowledgements;
		if (acknowledgements === null) {
			if (observation.complete) {
				this.#acknowledgements = new Map(
					Array.from(observation.presentations, ([id, current]) => [
						id,
						{ observation: current, seen: true },
					]),
				);
			}
			return;
		}
		// A Presentation that loses its Agent keeps no acknowledgement for a later one.
		for (const id of acknowledgements.keys()) {
			if (!observation.presentations.has(id)) acknowledgements.delete(id);
		}
		for (const [id, current] of observation.presentations) {
			const previous = acknowledgements.get(id);
			if (previous !== undefined && version(previous.observation) === version(current)) continue;
			// A replacement terminal or another Agent starts without the earlier acknowledgement.
			const sameAgent =
				previous?.observation.binding === current.binding &&
				previous.observation.agent === current.agent
					? previous
					: undefined;
			acknowledgements.set(id, {
				observation: current,
				seen: versionStartsSeen(sameAgent, current.state),
			});
		}
	}

	/** A stale paint or an unfocused window cannot acknowledge a state. */
	acknowledge(
		painted: WorkbenchAgentAttentionSnapshot,
		visiblePresentations: ReadonlySet<string>,
		focused: boolean,
	): void {
		const acknowledgements = this.#acknowledgements;
		if (
			!focused ||
			acknowledgements === null ||
			painted !== this.#snapshot ||
			this.#source?.getSnapshot() !== painted.observation
		)
			return;
		let changed = false;
		for (const id of visiblePresentations) {
			const current = painted.observation.presentations.get(id);
			const acknowledgement = acknowledgements.get(id);
			if (
				current === undefined ||
				acknowledgement === undefined ||
				acknowledgement.seen ||
				version(acknowledgement.observation) !== version(current)
			)
				continue;
			acknowledgements.set(id, { observation: acknowledgement.observation, seen: true });
			changed = true;
		}
		if (changed) this.#publish(painted.observation);
	}

	#publish(observation: WorkbenchAgentSnapshot): void {
		const unread = new Set<string>();
		for (const id of observation.presentations.keys()) {
			if (this.#acknowledgements?.get(id)?.seen === false) unread.add(id);
		}
		const presentation = projectObservation(observation);
		this.#snapshot = { observation, presentation, unread };
		for (const listener of this.#listeners) listener();
	}
}

function projectObservation(observation: WorkbenchAgentSnapshot): WorkbenchAgentSnapshot {
	const presentations = new Map<string, WorkbenchAgentObservation>();
	let changed = false;
	for (const [id, current] of observation.presentations) {
		const state = resolveWorkbenchAgentDisplayState(current);
		changed ||= state !== current.state;
		presentations.set(id, state === current.state ? current : { ...current, state });
	}
	return changed ? { ...observation, presentations } : observation;
}
