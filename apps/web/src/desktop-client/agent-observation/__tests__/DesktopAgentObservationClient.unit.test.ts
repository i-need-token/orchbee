import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRequestIdSchema,
	AgentSessionIdSchema,
	AgentSemanticEventSchema,
	createAgentRuntimeSnapshot,
	transitionAgentRuntime,
} from '@orchbee/agent';
import {
	DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
	DesktopAgentObservationMainEventSchema,
	DesktopAgentObservationRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import { DesktopAgentObservationClient } from '../DesktopAgentObservationClient';

describe('DesktopAgentObservationClient', () => {
	it('initializes from a correlated full snapshot and publishes increasing revisions', async () => {
		const port = new FakeMessagePort();
		const client = new DesktopAgentObservationClient(port.asMessagePort(), () => 'request');
		const listener = vi.fn<() => void>();
		client.subscribe(listener);

		const initialization = client.initialize();
		const request = DesktopAgentObservationRendererCommandSchema.parse(port.messages[0]);
		port.emit(publication(request.requestId, createAgentRuntimeSnapshot()));

		await expect(initialization).resolves.toEqual({ kind: 'ready' });
		expect(client.getSnapshot()).toEqual(createAgentRuntimeSnapshot());
		expect(listener).toHaveBeenCalledOnce();

		const observed = observedSnapshot();
		port.emit(publication(null, observed));
		port.emit(publication(null, createAgentRuntimeSnapshot()));

		expect(client.getSnapshot()).toEqual(observed);
		expect(listener).toHaveBeenCalledTimes(2);
	});

	it('publishes semantic events independently from snapshots', async () => {
		const port = new FakeMessagePort();
		const client = new DesktopAgentObservationClient(port.asMessagePort(), () => 'request');
		const listener = vi.fn<(event: ReturnType<typeof AgentSemanticEventSchema.parse>) => void>();
		client.subscribeSemanticEvents(listener);

		const initialization = client.initialize();
		const request = DesktopAgentObservationRendererCommandSchema.parse(port.messages[0]);
		port.emit(publication(request.requestId, createAgentRuntimeSnapshot()));
		await expect(initialization).resolves.toEqual({ kind: 'ready' });

		const event = AgentSemanticEventSchema.parse({
			eventId: 'event-attention',
			request: {
				actionPath: 'navigate-only',
				kind: 'question',
				requestId: AgentRequestIdSchema.parse('request-1'),
				turnId: null,
			},
			sessionId: AgentSessionIdSchema.parse('session-1'),
			type: 'attention-requested',
		});
		port.emit({
			event,
			type: 'desktop-agent-observation-semantic-event-published',
			version: DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
		});

		expect(listener).toHaveBeenCalledOnce();
		expect(listener).toHaveBeenCalledWith(event);
	});

	it('retires a pending initialization on malformed or uncorrelated publication', async () => {
		const malformedPort = new FakeMessagePort();
		const malformedClient = new DesktopAgentObservationClient(
			malformedPort.asMessagePort(),
			() => 'request',
		);
		const malformedInitialization = malformedClient.initialize();
		malformedPort.emit({ prompt: 'untrusted provider content' });

		await expect(malformedInitialization).resolves.toEqual({ kind: 'unavailable' });
		expect(malformedPort.close).toHaveBeenCalledOnce();

		const uncorrelatedPort = new FakeMessagePort();
		const uncorrelatedClient = new DesktopAgentObservationClient(
			uncorrelatedPort.asMessagePort(),
			() => 'request',
		);
		const uncorrelatedInitialization = uncorrelatedClient.initialize();
		uncorrelatedPort.emit(publication('unrelated:1', createAgentRuntimeSnapshot()));

		await expect(uncorrelatedInitialization).resolves.toEqual({ kind: 'unavailable' });
		expect(uncorrelatedPort.close).toHaveBeenCalledOnce();
	});
});

function observedSnapshot() {
	const transition = transitionAgentRuntime(createAgentRuntimeSnapshot(), {
		capabilities: ['sessionLifecycle'],
		eventId: AgentEventIdSchema.parse('event-1'),
		executionLocation: {
			executionLocationId: AgentExecutionLocationIdSchema.parse('execution-location-1'),
		},
		observation: {
			fidelity: 'rich',
			freshness: 'fresh',
			generation: 0,
			sequence: 1,
			source: 'hook',
		},
		parentSessionId: null,
		presentationAttachments: [
			{
				generation: 'binding-1',
				navigable: true,
				presentationId: 'presentation-1',
				visibility: 'visible',
			},
		],
		providerId: AgentProviderIdSchema.parse('claude-code'),
		sessionId: AgentSessionIdSchema.parse('session-1'),
		type: 'session-observed',
	});
	if (transition.kind !== 'applied') throw new Error('Expected an observed Agent Session.');
	return transition.snapshot;
}

function publication(requestId: string | null, snapshot: ReturnType<typeof observedSnapshot>) {
	return DesktopAgentObservationMainEventSchema.parse({
		requestId,
		snapshot,
		type: 'desktop-agent-observation-snapshot-published',
		version: DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
	});
}

class FakeMessagePort {
	readonly close = vi.fn<() => void>();
	readonly #messageErrorListeners = new Set<(event: MessageEvent<unknown>) => void>();
	readonly #messageListeners = new Set<(event: MessageEvent<unknown>) => void>();
	readonly messages: unknown[] = [];
	readonly postMessage = vi.fn<(message: unknown) => void>((message) => {
		this.messages.push(message);
	});

	addEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
		if (type === 'message') {
			this.#messageListeners.add(listener as (event: MessageEvent<unknown>) => void);
		} else if (type === 'messageerror') {
			this.#messageErrorListeners.add(listener as (event: MessageEvent<unknown>) => void);
		}
	}

	asMessagePort(): MessagePort {
		return this as unknown as MessagePort;
	}

	emit(data: unknown): void {
		for (const listener of this.#messageListeners) {
			listener({ data, ports: [] } as unknown as MessageEvent<unknown>);
		}
	}

	removeEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
		if (type === 'message') {
			this.#messageListeners.delete(listener as (event: MessageEvent<unknown>) => void);
		} else if (type === 'messageerror') {
			this.#messageErrorListeners.delete(listener as (event: MessageEvent<unknown>) => void);
		}
	}

	start(): void {}
}
