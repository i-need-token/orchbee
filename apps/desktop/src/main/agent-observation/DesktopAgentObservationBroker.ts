import {
	DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
	DesktopAgentObservationMainEventSchema,
	DesktopAgentObservationRendererCommandSchema,
	type DesktopAgentObservationRequestId,
} from '@orchbee/desktop-client-protocol';
import type { AgentRuntimeSnapshot, AgentSemanticEvent } from '@orchbee/agent';
import type { MessageEvent, MessagePortMain } from 'electron';

export interface DesktopAgentObservationBrokerSource {
	getSnapshot(): AgentRuntimeSnapshot;
	subscribe(listener: () => void): () => void;
	subscribeSemanticEvents(listener: (event: AgentSemanticEvent) => void): () => void;
}

export class DesktopAgentObservationBroker {
	#disposed = false;
	readonly #port: MessagePortMain;
	readonly #source: DesktopAgentObservationBrokerSource;
	#started = false;
	#stopObservation: (() => void) | undefined;
	#stopSemanticEvents: (() => void) | undefined;

	constructor(port: MessagePortMain, source: DesktopAgentObservationBrokerSource) {
		this.#port = port;
		this.#source = source;
	}

	start(): void {
		if (this.#started || this.#disposed) return;
		this.#started = true;
		this.#port.on('message', this.#handleMessage);
		this.#port.on('close', this.#handleClose);
		this.#stopObservation = this.#source.subscribe(this.#publishChange);
		this.#stopSemanticEvents = this.#source.subscribeSemanticEvents(this.#publishSemanticEvent);
		this.#port.start();
	}

	dispose(): void {
		this.#retire(true);
	}

	readonly #handleClose = (): void => {
		this.#retire(false);
	};

	readonly #handleMessage = (event: MessageEvent): void => {
		if (event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopAgentObservationRendererCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		this.#post(parsed.data.requestId);
	};

	readonly #publishChange = (): void => {
		this.#post(null);
	};

	readonly #publishSemanticEvent = (event: AgentSemanticEvent): void => {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(
				DesktopAgentObservationMainEventSchema.parse({
					event,
					type: 'desktop-agent-observation-semantic-event-published',
					version: DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
				}),
			);
		} catch {
			this.#retire(true);
		}
	};

	#post(requestId: DesktopAgentObservationRequestId | null): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(
				DesktopAgentObservationMainEventSchema.parse({
					requestId,
					snapshot: this.#source.getSnapshot(),
					type: 'desktop-agent-observation-snapshot-published',
					version: DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
				}),
			);
		} catch {
			this.#retire(true);
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#stopObservation?.();
		this.#stopObservation = undefined;
		this.#stopSemanticEvents?.();
		this.#stopSemanticEvents = undefined;
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		if (closePort) this.#port.close();
	}
}
