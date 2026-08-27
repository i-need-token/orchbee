import { GatewayBrowserConnectionSchema } from '../gateway-control-contract';
import {
	decodeGatewayBrowserSearch,
	encodeGatewayBrowserSearch,
} from '../gateway-browser-location';
import { describe, expect, it } from 'vitest';

describe('Gateway Browser location', () => {
	it('round-trips only non-secret Gateway bootstrap data through URL search', () => {
		const connection = GatewayBrowserConnectionSchema.parse({
			port: 42_000,
			protocolVersion: '1.3',
		});
		const search = encodeGatewayBrowserSearch(connection);

		expect(search).toBe('orchbee-gateway-version=1.3&orchbee-gateway-port=42000');
		expect(search).not.toContain('capability');
		expect(decodeGatewayBrowserSearch(`?orchbee-runtime=gateway&${search}`)).toEqual(connection);
	});

	it('rejects duplicates, noncanonical ports, and protocol drift', () => {
		const base = 'orchbee-gateway-version=1.3&orchbee-gateway-port=42000';

		expect(decodeGatewayBrowserSearch(`${base}&orchbee-gateway-port=42001`)).toBeNull();
		expect(decodeGatewayBrowserSearch(base.replace('42000', '042000'))).toBeNull();
		expect(decodeGatewayBrowserSearch(base.replace('1.3', '2.0'))).toBeNull();
	});
});
