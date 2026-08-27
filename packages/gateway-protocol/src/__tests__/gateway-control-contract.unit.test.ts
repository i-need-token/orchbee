import { describe, expect, it } from 'vitest';

import {
	GatewayBrowserConnectionSchema,
	GatewayControlServerMessageSchema,
	GatewayDesktopCommandSchema,
	GatewayUtilityEventSchema,
} from '../gateway-control-contract';

describe('Gateway control contract', () => {
	it('accepts one loopback connection descriptor shape without embedding a host name', () => {
		expect(
			GatewayBrowserConnectionSchema.parse({
				port: 42_000,
				protocolVersion: '1.3',
			}),
		).toMatchObject({ port: 42_000, protocolVersion: '1.3' });
	});

	it('bounds the trusted Desktop listener and Origin configuration', () => {
		expect(
			GatewayDesktopCommandSchema.parse({
				allowedOrigins: ['http://127.0.0.1:3000'],
				bindHost: '127.0.0.1',
				sessionSigningKey: 's'.repeat(43),
				type: 'gateway-start',
			}),
		).toMatchObject({ bindHost: '127.0.0.1' });
		expect(
			GatewayDesktopCommandSchema.safeParse({
				allowedOrigins: [],
				bindHost: '0.0.0.0',
				sessionSigningKey: 's'.repeat(43),
				type: 'gateway-start',
			}).success,
		).toBe(false);
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

	it('frames release completion as control data without attachment authority', () => {
		expect(
			GatewayControlServerMessageSchema.parse({
				event: {
					presentationId: 'presentation-1',
					requestId: 'projection-1',
					type: 'terminal-projection-released',
				},
				type: 'terminal-projection-control-event',
			}),
		).toMatchObject({ event: { requestId: 'projection-1' } });
	});
});
