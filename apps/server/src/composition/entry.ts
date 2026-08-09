import { resolveServerConfiguration } from './resolveServerConfiguration';
import { startOrchbeeServer } from './startOrchbeeServer';

const configuration = resolveServerConfiguration(process.env);
void startOrchbeeServer(configuration).then(
	(server) => {
		process.stdout.write(`Orchbee Server listening at ${server.origin}\n`);
		const closeServer = (): void => {
			void server.close();
		};
		process.once('SIGINT', closeServer);
		process.once('SIGTERM', closeServer);
	},
	(error: unknown) => {
		const message = error instanceof Error ? error.message : String(error);
		process.stderr.write(`Orchbee Server failed to start: ${message}\n`);
		process.exitCode = 1;
	},
);
