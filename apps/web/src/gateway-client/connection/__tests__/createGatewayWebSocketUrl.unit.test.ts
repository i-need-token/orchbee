import { describe, expect, it } from 'vitest';

import { createGatewayWebSocketUrl } from '../createGatewayWebSocketUrl';

describe('createGatewayWebSocketUrl', () => {
	it('uses the Browser page host for loopback and private-network Gateway sockets', () => {
		expect(createGatewayWebSocketUrl('127.0.0.1', 42_000, '/v1/control')).toBe(
			'ws://127.0.0.1:42000/v1/control',
		);
		expect(createGatewayWebSocketUrl('192.168.1.14', 42_001, '/v1/attachment')).toBe(
			'ws://192.168.1.14:42001/v1/attachment',
		);
	});

	it('rejects host syntax that could redirect a capability to another endpoint', () => {
		for (const hostname of ['localhost', '192.168.1.14.example.com', '192.168.1.999', '[::1]']) {
			expect(() => createGatewayWebSocketUrl(hostname, 42_000, '/v1/control')).toThrow(
				/requires an IPv4/u,
			);
		}
	});
});
