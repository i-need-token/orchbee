import { GatewayUtilityEventSchema, type GatewayUtilityEvent } from '@orchbee/gateway-protocol';
import { describe, expect, it } from 'vitest';

import type { GatewayParentMessageEvent, GatewayParentPort } from '../../browser-gateway';
import { startGatewayUtilityProcess } from '../startGatewayUtilityProcess';

describe('startGatewayUtilityProcess', () => {
	it('binds and disposes one Gateway against the supplied parent port', async () => {
		const parent = new FakeParentPort();
		const gateway = startGatewayUtilityProcess(parent);

		expect(parent.listenerCount).toBe(1);
		await gateway.dispose();
		expect(parent.listenerCount).toBe(0);
		expect(parent.events).toEqual([{ type: 'gateway-stopped' }]);
	});
});

class FakeParentPort implements GatewayParentPort {
	readonly events: GatewayUtilityEvent[] = [];
	readonly #listeners = new Set<(event: GatewayParentMessageEvent) => void>();

	get listenerCount(): number {
		return this.#listeners.size;
	}

	off(_event: 'message', listener: (event: GatewayParentMessageEvent) => void): void {
		this.#listeners.delete(listener);
	}

	on(_event: 'message', listener: (event: GatewayParentMessageEvent) => void): void {
		this.#listeners.add(listener);
	}

	postMessage(message: unknown): void {
		this.events.push(GatewayUtilityEventSchema.parse(message));
	}
}
