// @vitest-environment jsdom

import { GatewayBrowserConnectionSchema } from '@orchbee/gateway-protocol';
import { afterEach, describe, expect, it } from 'vitest';

import { createGatewayRoutingHistory } from '../createGatewayRoutingHistory';

afterEach(() => {
	window.history.replaceState({}, '', '/');
});

describe('createGatewayRoutingHistory', () => {
	it('keeps canonical non-secret Gateway bootstrap parameters across route changes', () => {
		window.history.replaceState(
			{},
			'',
			'/?orchbee-runtime=gateway&orchbee-gateway-version=1.3&orchbee-gateway-port=42000',
		);
		const history = createGatewayRoutingHistory(
			window,
			GatewayBrowserConnectionSchema.parse({ port: 42_000, protocolVersion: '1.3' }),
		);
		try {
			history.replace(
				'/terminal?panel=compact&orchbee-gateway-port=1&orchbee-runtime=browser#focus',
			);
			history.flush();

			const location = new URL(window.location.href);
			expect(location.pathname).toBe('/terminal');
			expect(location.searchParams.get('panel')).toBe('compact');
			expect(location.searchParams.getAll('orchbee-runtime')).toEqual(['gateway']);
			expect(location.searchParams.getAll('orchbee-gateway-version')).toEqual(['1.3']);
			expect(location.searchParams.getAll('orchbee-gateway-port')).toEqual(['42000']);
			expect(location.hash).toBe('#focus');
		} finally {
			history.destroy();
		}
	});
});
