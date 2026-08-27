import { createOrchbeeServer } from '../server-ingress';
import type { ServerConfiguration } from './resolveServerConfiguration';

export interface RunningOrchbeeServer {
	readonly close: () => Promise<void>;
	readonly host: ServerConfiguration['host'];
	readonly origin: string;
	readonly port: number;
}

export async function startOrchbeeServer(
	configuration: ServerConfiguration,
): Promise<RunningOrchbeeServer> {
	const server = createOrchbeeServer({
		authentication: {
			personalAccessTokenVerifier: configuration.personalAccessTokenVerifier,
			sessionSigningKey: configuration.sessionSigningKey,
		},
		rendererRoot: configuration.rendererRoot,
	});
	const origin = await server.listen({
		host: configuration.host,
		port: configuration.port,
	});
	const address = server.server.address();
	if (address === null || typeof address === 'string') {
		await server.close();
		throw new Error('Orchbee Server did not expose a TCP listener address.');
	}
	return {
		close: () => server.close(),
		host: configuration.host,
		origin,
		port: address.port,
	};
}
