import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentSessionIdSchema,
	type AgentObservationEvent,
} from '@orchbee/agent';
import {
	DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
	DesktopAgentObservationMainEventSchema,
	DesktopAgentObservationRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { DesktopAgentObservationBroker } from '../DesktopAgentObservationBroker';
import { DesktopAgentObservationRuntime } from '../DesktopAgentObservationRuntime';

describe('DesktopAgentObservationBroker', () => {
	it('serves a full recovery snapshot and publishes later revisions without renderer ownership', () => {
		const port = new FakeMessagePort();
		const runtime = new DesktopAgentObservationRuntime({
			allocateEventId: () => 'desktop-event-1',
		});
		const broker = new DesktopAgentObservationBroker(port.asMessagePortMain(), runtime);
		broker.start();

		port.emit(
			DesktopAgentObservationRendererCommandSchema.parse({
				requestId: 'agent-observation:request-1',
				type: 'desktop-agent-observation-snapshot-requested',
				version: DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
			}),
		);
		expect(DesktopAgentObservationMainEventSchema.parse(port.messages[0])).toMatchObject({
			requestId: 'agent-observation:request-1',
			snapshot: { revision: 0, sessions: [] },
		});

		runtime.dispatch(observed());
		expect(DesktopAgentObservationMainEventSchema.parse(port.messages[1])).toMatchObject({
			requestId: null,
			snapshot: { revision: 1, sessions: [{ sessionId: 'session-1' }] },
		});
	});

	it('retires only the renderer capability on malformed input', () => {
		const port = new FakeMessagePort();
		const runtime = new DesktopAgentObservationRuntime({
			allocateEventId: () => 'desktop-event-1',
		});
		const broker = new DesktopAgentObservationBroker(port.asMessagePortMain(), runtime);
		broker.start();

		port.emit({ prompt: 'untrusted provider content' });

		expect(port.close).toHaveBeenCalledOnce();
		expect(runtime.dispatch(observed()).kind).toBe('applied');
		expect(port.messages).toEqual([]);
	});
});

function observed(): Extract<AgentObservationEvent, { readonly type: 'session-observed' }> {
	return {
		capabilities: ['sessionLifecycle'],
		eventId: AgentEventIdSchema.parse('provider-event-1'),
		executionLocation: {
			executionLocationId: AgentExecutionLocationIdSchema.parse('execution-1'),
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
	};
}

class FakeMessagePort {
	readonly #closeListeners = new Set<() => void>();
	readonly close = vi.fn<() => void>();
	readonly messages: unknown[] = [];
	readonly #messageListeners = new Set<(event: MessageEvent) => void>();

	asMessagePortMain(): MessagePortMain {
		return this as unknown as MessagePortMain;
	}

	emit(data: unknown): void {
		for (const listener of this.#messageListeners) {
			listener({ data, ports: [] } as unknown as MessageEvent);
		}
	}

	off(event: string, listener: (...arguments_: never[]) => void): void {
		if (event === 'message') {
			this.#messageListeners.delete(listener as unknown as (event: MessageEvent) => void);
		} else if (event === 'close') {
			this.#closeListeners.delete(listener as unknown as () => void);
		}
	}

	on(event: string, listener: (...arguments_: never[]) => void): void {
		if (event === 'message') {
			this.#messageListeners.add(listener as unknown as (event: MessageEvent) => void);
		} else if (event === 'close') {
			this.#closeListeners.add(listener as unknown as () => void);
		}
	}

	postMessage(message: unknown): void {
		this.messages.push(message);
	}

	start(): void {}
}
