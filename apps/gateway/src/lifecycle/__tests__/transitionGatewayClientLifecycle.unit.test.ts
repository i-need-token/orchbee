import { GatewayClientConnectionIdSchema } from '@orchbee/gateway-protocol';
import { describe, expect, it } from 'vitest';

import { transitionGatewayClientLifecycle } from '../transitionGatewayClientLifecycle';

const connectionId = GatewayClientConnectionIdSchema.parse('gateway:client:1');

describe('Gateway client lifecycle model', () => {
	it('creates Desktop authority only after capability authentication', () => {
		const rejected = transitionGatewayClientLifecycle(
			{ phase: 'awaiting-authentication' },
			{ accepted: false, connectionId, type: 'authentication-completed' },
		);
		const accepted = transitionGatewayClientLifecycle(
			{ phase: 'awaiting-authentication' },
			{ accepted: true, connectionId, type: 'authentication-completed' },
		);

		expect(rejected).toEqual({
			commands: [{ type: 'close-control-socket' }],
			state: { phase: 'closed' },
		});
		expect(accepted).toEqual({
			commands: [{ connectionId, type: 'request-client-open' }],
			state: { connectionId, phase: 'opening-client' },
		});
	});

	it('releases exactly the authenticated page session on disconnect', () => {
		expect(
			transitionGatewayClientLifecycle(
				{ connectionId, phase: 'active' },
				{ type: 'socket-closed' },
			),
		).toEqual({
			commands: [{ connectionId, type: 'request-client-close' }],
			state: { phase: 'closed' },
		});
	});

	it('disposes a late Desktop port bundle after the Browser has gone', () => {
		expect(
			transitionGatewayClientLifecycle(
				{ phase: 'closed' },
				{ connectionId, type: 'desktop-client-opened' },
			),
		).toEqual({
			commands: [{ type: 'dispose-desktop-ports' }],
			state: { phase: 'closed' },
		});
	});

	it('fails closed on stale or out-of-order lifecycle events', () => {
		expect(
			transitionGatewayClientLifecycle(
				{ connectionId, phase: 'active' },
				{ connectionId, type: 'desktop-client-opened' },
			).state,
		).toEqual({ phase: 'closed' });
	});
});
