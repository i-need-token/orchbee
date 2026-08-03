import { GatewayBrowserConnectionSchema } from '../gateway-control-contract';
import {
	decodeGatewayBrowserFragment,
	encodeGatewayBrowserFragment,
} from '../gateway-browser-location';
import { describe, expect, it } from 'vitest';

describe('Gateway Browser location', () => {
	it('round-trips the capability only through the URL fragment', () => {
		const connection = GatewayBrowserConnectionSchema.parse({
			capability: 'a'.repeat(43),
			port: 42_000,
			protocolVersion: '1.0',
		});
		const fragment = encodeGatewayBrowserFragment(connection);

		expect(fragment).not.toContain('?');
		expect(decodeGatewayBrowserFragment(`#${fragment}`)).toEqual(connection);
	});

	it('rejects duplicates, unknown fields, noncanonical ports, and protocol drift', () => {
		const capability = 'a'.repeat(43);
		const base = `orchbee-gateway-version=1.0&orchbee-gateway-port=42000&orchbee-gateway-capability=${capability}`;

		expect(decodeGatewayBrowserFragment(`${base}&unexpected=true`)).toBeNull();
		expect(decodeGatewayBrowserFragment(`${base}&orchbee-gateway-port=42001`)).toBeNull();
		expect(decodeGatewayBrowserFragment(base.replace('42000', '042000'))).toBeNull();
		expect(decodeGatewayBrowserFragment(base.replace('1.0', '2.0'))).toBeNull();
	});
});
