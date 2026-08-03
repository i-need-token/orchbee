import { describe, expect, it } from 'vitest';

import {
	GatewayBrowserConnectionSchema,
	GatewayDesktopCommandSchema,
	GatewayUtilityEventSchema,
} from '../gateway-control-contract';

describe('Gateway control contract', () => {
	it('accepts one loopback connection descriptor shape without embedding a host name', () => {
		expect(
			GatewayBrowserConnectionSchema.parse({
				capability: 'a'.repeat(43),
				port: 42_000,
				protocolVersion: '1.0',
			}),
		).toMatchObject({ port: 42_000, protocolVersion: '1.0' });
	});

	it('keeps transferred ports outside structured payload authority', () => {
		expect(
			GatewayDesktopCommandSchema.safeParse({
				bootstrap: {},
				connectionId: 'gateway:client:1',
				ports: ['host'],
				type: 'gateway-client-opened',
			}).success,
		).toBe(false);
		expect(
			GatewayUtilityEventSchema.parse({
				connectionId: 'gateway:client:1',
				type: 'gateway-client-open-requested',
			}),
		).toMatchObject({ connectionId: 'gateway:client:1' });
	});
});
