import { createOrchbeeServer } from '../server-ingress';
import type { ServerConfiguration } from './resolveServerConfiguration';

export interface RunningOrchbeeServer {
	readonly close: () => Promise<void>;
	readonly origin: string;
}

export async function startOrchbeeServer(
	configuration: ServerConfiguration,
): Promise<RunningOrchbeeServer> {
	const server = createOrchbeeServer({ rendererRoot: configuration.rendererRoot });
	const origin = await server.listen({
		host: configuration.host,
		port: configuration.port,
	});
	return {
		close: () => server.close(),
		origin,
	};
}
