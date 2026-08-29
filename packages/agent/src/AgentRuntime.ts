import type { AgentObservationEvent, AgentSemanticEvent } from './events';
import { AgentRuntimeSnapshotSchema, type AgentRuntimeSnapshot } from './model';
import {
	createAgentRuntimeSnapshot,
	transitionAgentRuntime,
	type AgentTransition,
} from './reducer';

export type AgentRuntimeListener = () => void;
export type AgentSemanticEventListener = (event: AgentSemanticEvent) => void;

export interface AgentSnapshotSource {
	getSnapshot(): AgentRuntimeSnapshot;
	subscribe(listener: AgentRuntimeListener): () => void;
	subscribeSemanticEvents?(listener: AgentSemanticEventListener): () => void;
}

export class AgentRuntime {
	#disposed = false;
	readonly #listeners = new Set<AgentRuntimeListener>();
	readonly #semanticEventListeners = new Set<AgentSemanticEventListener>();
	#snapshot: AgentRuntimeSnapshot;

	constructor(snapshot: AgentRuntimeSnapshot = createAgentRuntimeSnapshot()) {
		this.#snapshot = AgentRuntimeSnapshotSchema.parse(snapshot);
	}

	dispatch(event: AgentObservationEvent): AgentTransition {
		if (this.#disposed) {
			return {
				kind: 'rejected',
				reason: 'runtime-disposed',
				semanticEvents: [],
				snapshot: this.#snapshot,
			};
		}
		const transition = transitionAgentRuntime(this.#snapshot, event);
		if (transition.kind !== 'applied') return transition;
		this.#snapshot = transition.snapshot;
		const listeners = Array.from(this.#listeners);
		for (const listener of listeners) listener();
		for (const semanticEvent of transition.semanticEvents) {
			const semanticEventListeners = Array.from(this.#semanticEventListeners);
			for (const listener of semanticEventListeners) listener(semanticEvent);
		}
		return transition;
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#listeners.clear();
		this.#semanticEventListeners.clear();
	}

	getSnapshot = (): AgentRuntimeSnapshot => this.#snapshot;

	subscribe(listener: AgentRuntimeListener): () => void {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	subscribeSemanticEvents(listener: AgentSemanticEventListener): () => void {
		if (this.#disposed) return () => undefined;
		this.#semanticEventListeners.add(listener);
		return () => this.#semanticEventListeners.delete(listener);
	}
}
