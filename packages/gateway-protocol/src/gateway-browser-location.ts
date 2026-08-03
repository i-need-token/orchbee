import {
	GATEWAY_PROTOCOL_VERSION,
	GatewayBrowserConnectionSchema,
	type GatewayBrowserConnection,
} from './gateway-control-contract';

const CAPABILITY_KEY = 'orchbee-gateway-capability';
const PORT_KEY = 'orchbee-gateway-port';
const VERSION_KEY = 'orchbee-gateway-version';
const CONNECTION_KEYS = new Set([CAPABILITY_KEY, PORT_KEY, VERSION_KEY]);

export function encodeGatewayBrowserFragment(connection: GatewayBrowserConnection): string {
	const canonical = GatewayBrowserConnectionSchema.parse(connection);
	const parameters = new URLSearchParams();
	parameters.set(VERSION_KEY, canonical.protocolVersion);
	parameters.set(PORT_KEY, String(canonical.port));
	parameters.set(CAPABILITY_KEY, canonical.capability);
	return parameters.toString();
}

export function decodeGatewayBrowserFragment(fragment: string): GatewayBrowserConnection | null {
	const source = fragment.startsWith('#') ? fragment.slice(1) : fragment;
	const parameters = new URLSearchParams(source);
	for (const key of parameters.keys()) {
		if (!CONNECTION_KEYS.has(key)) return null;
	}
	const versions = parameters.getAll(VERSION_KEY);
	const ports = parameters.getAll(PORT_KEY);
	const capabilities = parameters.getAll(CAPABILITY_KEY);
	if (versions.length !== 1 || ports.length !== 1 || capabilities.length !== 1) return null;
	if (versions[0] !== GATEWAY_PROTOCOL_VERSION) return null;
	const portText = ports[0];
	const capability = capabilities[0];
	if (portText === undefined || capability === undefined || !/^[1-9][0-9]*$/u.test(portText)) {
		return null;
	}
	const parsed = GatewayBrowserConnectionSchema.safeParse({
		capability,
		port: Number(portText),
		protocolVersion: versions[0],
	});
	return parsed.success ? parsed.data : null;
}
