import type { NetworkInterfaceInfo } from 'node:os';

import { describe, expect, it } from 'vitest';

import { resolveDesktopBrowserAccessEndpoints } from '../resolveDesktopBrowserAccessEndpoints';

describe('resolveDesktopBrowserAccessEndpoints', () => {
	it('uses loopback by default without consulting network interfaces', () => {
		expect(resolveDesktopBrowserAccessEndpoints('loopback', 43_110, {})).toEqual({
			bindHost: '127.0.0.1',
			localOrigin: 'http://127.0.0.1:43110',
			networkOrigins: [],
		});
	});

	it('publishes unique sorted RFC 1918 IPv4 origins for the explicit preview', () => {
		expect(
			resolveDesktopBrowserAccessEndpoints('local-network', 43_110, {
				en0: [address('192.168.50.12'), address('2001:db8::1', 'IPv6')],
				en1: [address('10.1.2.3'), address('192.168.50.12')],
				lo0: [address('127.0.0.1', 'IPv4', true)],
				public: [address('203.0.113.10')],
			}),
		).toEqual({
			bindHost: '0.0.0.0',
			localOrigin: 'http://127.0.0.1:43110',
			networkOrigins: ['http://10.1.2.3:43110', 'http://192.168.50.12:43110'],
		});
	});

	it('fails before binding when no private IPv4 address can be shared', () => {
		expect(() =>
			resolveDesktopBrowserAccessEndpoints('local-network', 43_110, {
				en0: [address('203.0.113.10')],
			}),
		).toThrow('Local Network Preview requires one RFC 1918 IPv4 interface.');
	});
});

function address(
	value: string,
	family: NetworkInterfaceInfo['family'] = 'IPv4',
	internal = false,
): NetworkInterfaceInfo {
	return {
		address: value,
		cidr: null,
		family,
		internal,
		mac: '00:00:00:00:00:00',
		netmask: family === 'IPv4' ? '255.255.255.0' : 'ffff:ffff:ffff:ffff::',
		scopeid: 0,
	};
}
