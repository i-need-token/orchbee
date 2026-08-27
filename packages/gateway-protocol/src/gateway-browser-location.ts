import {
	GATEWAY_PROTOCOL_VERSION,
	GatewayBrowserConnectionSchema,
	type GatewayBrowserConnection,
} from './gateway-control-contract';

const PORT_KEY = 'orchbee-gateway-port';
const VERSION_KEY = 'orchbee-gateway-version';

export function encodeGatewayBrowserSearch(connection: GatewayBrowserConnection): string {
	const canonical = GatewayBrowserConnectionSchema.parse(connection);
	const parameters = new URLSearchParams();
	parameters.set(VERSION_KEY, canonical.protocolVersion);
	parameters.set(PORT_KEY, String(canonical.port));
	return parameters.toString();
}

export function decodeGatewayBrowserSearch(search: string): GatewayBrowserConnection | null {
	const source = search.startsWith('?') ? search.slice(1) : search;
	const parameters = new URLSearchParams(source);
	const versions = parameters.getAll(VERSION_KEY);
	const ports = parameters.getAll(PORT_KEY);
	if (versions.length !== 1 || ports.length !== 1) return null;
	if (versions[0] !== GATEWAY_PROTOCOL_VERSION) return null;
	const portText = ports[0];
	if (portText === undefined || !/^[1-9][0-9]*$/u.test(portText)) {
		return null;
	}
	const parsed = GatewayBrowserConnectionSchema.safeParse({
		port: Number(portText),
		protocolVersion: versions[0],
	});
	return parsed.success ? parsed.data : null;
}
