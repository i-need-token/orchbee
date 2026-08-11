import type { NetworkInterfaceInfo } from 'node:os';

import type { DesktopBrowserAccessMode } from '@orchbee/desktop-client-protocol';
import type { GatewayBindHost } from '@orchbee/gateway-protocol';

const MAXIMUM_NETWORK_ORIGINS = 31;

interface DesktopBrowserAccessEndpointPlan {
	readonly bindHost: GatewayBindHost;
	readonly localOrigin: string;
	readonly networkOrigins: readonly string[];
}

class DesktopBrowserAccessNetworkError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'DesktopBrowserAccessNetworkError';
	}
}

export function resolveDesktopBrowserAccessEndpoints(
	mode: DesktopBrowserAccessMode,
	port: number,
	interfaces: Readonly<Record<string, readonly NetworkInterfaceInfo[] | undefined>>,
): DesktopBrowserAccessEndpointPlan {
	if (!Number.isInteger(port) || port < 1 || port > 65_535) {
		throw new DesktopBrowserAccessNetworkError('Browser access requires a valid Server port.');
	}
	const localOrigin = `http://127.0.0.1:${port}`;
	if (mode === 'loopback') {
		return { bindHost: '127.0.0.1', localOrigin, networkOrigins: [] };
	}
	const addresses = privateIPv4Addresses(interfaces);
	if (addresses.length === 0) {
		throw new DesktopBrowserAccessNetworkError(
			'Local Network Preview requires one RFC 1918 IPv4 interface.',
		);
	}
	if (addresses.length > MAXIMUM_NETWORK_ORIGINS) {
		throw new DesktopBrowserAccessNetworkError(
			'Local Network Preview has too many eligible IPv4 interfaces.',
		);
	}
	return {
		bindHost: '0.0.0.0',
		localOrigin,
		networkOrigins: addresses.map((address) => `http://${address}:${port}`),
	};
}

function privateIPv4Addresses(
	interfaces: Readonly<Record<string, readonly NetworkInterfaceInfo[] | undefined>>,
): readonly string[] {
	const addresses = new Set<string>();
	for (const records of Object.values(interfaces)) {
		if (records === undefined) continue;
		for (const record of records) {
			if (record.internal || record.family !== 'IPv4') continue;
			if (isPrivateIPv4(record.address)) addresses.add(record.address);
		}
	}
	return [...addresses].sort(compareIPv4);
}

function isPrivateIPv4(value: string): boolean {
	const octets = value.split('.');
	if (octets.length !== 4) return false;
	const numbers = octets.map((octet) => Number(octet));
	if (
		numbers.some(
			(number, index) =>
				!Number.isInteger(number) || number < 0 || number > 255 || String(number) !== octets[index],
		)
	) {
		return false;
	}
	const first = numbers[0];
	const second = numbers[1];
	return (
		first === 10 ||
		(first === 172 && second !== undefined && second >= 16 && second <= 31) ||
		(first === 192 && second === 168)
	);
}

function compareIPv4(left: string, right: string): number {
	const leftOctets = left.split('.').map(Number);
	const rightOctets = right.split('.').map(Number);
	for (let index = 0; index < 4; index += 1) {
		const difference = (leftOctets[index] ?? 0) - (rightOctets[index] ?? 0);
		if (difference !== 0) return difference;
	}
	return 0;
}
