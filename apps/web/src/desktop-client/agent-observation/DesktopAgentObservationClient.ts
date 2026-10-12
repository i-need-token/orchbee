import type {
	AgentRuntimeSnapshot,
	AgentSemanticEvent,
	AgentSemanticEventListener,
	AgentSnapshotSource,
} from '@orchbee/agent';
import {
	DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
	DesktopAgentObservationMainEventSchema,
	DesktopAgentObservationRendererCommandSchema,
	DesktopAgentObservationRequestIdSchema,
	type DesktopAgentObservationRequestId,
} from '@orchbee/desktop-client-protocol';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';

type DesktopAgentObservationClientInitialization =
	| { readonly kind: 'ready' }
	| { readonly kind: 'unavailable' };

export class DesktopAgentObservationClient implements AgentSnapshotSource {
	readonly #createRequestId: () => DesktopAgentObservationRequestId;
	#disposed = false;
	#initialization: Promise<DesktopAgentObservationClientInitialization> | undefined;
	readonly #listeners = new Set<() => void>();
	#pendingRequest:
		| {
				readonly requestId: DesktopAgentObservationRequestId;
				readonly resolve: (ready: boolean) => void;
		  }
		| undefined;
	readonly #port: MessagePort;
	#requestSequence = 0;
	readonly #semanticEventListeners = new Set<AgentSemanticEventListener>();
	#snapshot: AgentRuntimeSnapshot | undefined;

	constructor(port: MessagePort, createRequestId: () => string = defaultRequestId) {
		this.#port = port;
		this.#createRequestId = () =>
			DesktopAgentObservationRequestIdSchema.parse(
				`${createRequestId()}:${++this.#requestSequence}`,
			);
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	initialize(): Promise<DesktopAgentObservationClientInitialization> {
		this.#initialization ??= this.#initialize();
		return this.#initialization;
	}

	getSnapshot = (): AgentRuntimeSnapshot => {
		if (this.#snapshot === undefined) {
			throw new Error('Agent observation must initialize before reading its snapshot.');
		}
		return this.#snapshot;
	};

	subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	subscribeSemanticEvents = (listener: AgentSemanticEventListener): (() => void) => {
		this.#semanticEventListeners.add(listener);
		return () => this.#semanticEventListeners.delete(listener);
	};

	dispose(): void {
		this.#retire(true);
	}

	async #initialize(): Promise<DesktopAgentObservationClientInitialization> {
		if (this.#snapshot !== undefined) return { kind: 'ready' };
		if (this.#disposed || this.#pendingRequest !== undefined) return { kind: 'unavailable' };
		const requestId = this.#createRequestId();
		const result = new Promise<boolean>((resolve) => {
			this.#pendingRequest = { requestId, resolve };
		});
		try {
			this.#port.postMessage(
				DesktopAgentObservationRendererCommandSchema.parse({
					requestId,
					type: 'desktop-agent-observation-snapshot-requested',
					version: DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
				}),
			);
		} catch {
			this.#retire(true);
		}
		return (await result) ? { kind: 'ready' } : { kind: 'unavailable' };
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopAgentObservationMainEventSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const publication = parsed.data;
		if (publication.type === 'desktop-agent-observation-semantic-event-published') {
			this.#emitSemanticEvent(publication.event);
			return;
		}
		const pending = this.#pendingRequest;
		if (
			publication.requestId !== null &&
			(pending === undefined || publication.requestId !== pending.requestId)
		) {
			this.#retire(true);
			return;
		}
		const previous = this.#snapshot;
		if (previous === undefined || publication.snapshot.revision > previous.revision) {
			this.#snapshot = publication.snapshot;
			this.#emit();
		}
		if (publication.requestId !== null && pending !== undefined) {
			this.#pendingRequest = undefined;
			pending.resolve(true);
		}
	};

	readonly #handleMessageError = (): void => {
		this.#retire(true);
	};

	#emit(): void {
		const listeners = Array.from(this.#listeners);
		for (const listener of listeners) {
			try {
				listener();
			} catch {
				// Observation cannot strand the correlated snapshot receipt or other observers.
			}
		}
	}

	#emitSemanticEvent(event: AgentSemanticEvent): void {
		const listeners = Array.from(this.#semanticEventListeners);
		for (const listener of listeners) {
			try {
				listener(event);
			} catch {
				// One semantic observer cannot suppress delivery to the remaining cohort.
			}
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		if (closePort) this.#port.close();
		const pending = this.#pendingRequest;
		this.#pendingRequest = undefined;
		pending?.resolve(false);
		this.#listeners.clear();
		this.#semanticEventListeners.clear();
	}
}

function defaultRequestId(): string {
	return `agent-observation:${createBrowserRandomIdentifier()}`;
}
