import { isAbsolute } from 'node:path';
import { z } from 'zod';

const ServerConfigurationInputSchema = z.strictObject({
	host: z.enum(['127.0.0.1', '0.0.0.0']).default('127.0.0.1'),
	port: z
		.string()
		.regex(/^\d+$/u)
		.transform(Number)
		.pipe(z.number().int().min(0).max(65_535))
		.default(0),
	rendererRoot: z.string().min(1).refine(isAbsolute, 'Renderer root must be an absolute path.'),
});

export interface ServerConfiguration {
	readonly host: '0.0.0.0' | '127.0.0.1';
	readonly port: number;
	readonly rendererRoot: string;
}

export function resolveServerConfiguration(environment: NodeJS.ProcessEnv): ServerConfiguration {
	return ServerConfigurationInputSchema.parse({
		host: environment['ORCHBEE_SERVER_HOST'],
		port: environment['ORCHBEE_SERVER_PORT'],
		rendererRoot: environment['ORCHBEE_SERVER_RENDERER_ROOT'],
	});
}
