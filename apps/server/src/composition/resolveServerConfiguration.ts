import { isAbsolute } from 'node:path';
import {
	BrowserSessionSigningKeySchema,
	PersonalAccessTokenVerifierSchema,
	type BrowserSessionSigningKey,
	type PersonalAccessTokenVerifier,
} from '@orchbee/remote-client-protocol';
import { z } from 'zod';

const ServerConfigurationInputSchema = z.strictObject({
	host: z.enum(['127.0.0.1', '0.0.0.0']).default('127.0.0.1'),
	port: z
		.string()
		.regex(/^\d+$/u)
		.transform(Number)
		.pipe(z.number().int().min(0).max(65_535))
		.default(0),
	personalAccessTokenVerifier: PersonalAccessTokenVerifierSchema,
	rendererRoot: z.string().min(1).refine(isAbsolute, 'Renderer root must be an absolute path.'),
	sessionSigningKey: BrowserSessionSigningKeySchema,
});

export interface ServerConfiguration {
	readonly host: '0.0.0.0' | '127.0.0.1';
	readonly personalAccessTokenVerifier: PersonalAccessTokenVerifier;
	readonly port: number;
	readonly rendererRoot: string;
	readonly sessionSigningKey: BrowserSessionSigningKey;
}

export function resolveServerConfiguration(environment: NodeJS.ProcessEnv): ServerConfiguration {
	return ServerConfigurationInputSchema.parse({
		host: environment['ORCHBEE_SERVER_HOST'],
		personalAccessTokenVerifier: environment['ORCHBEE_SERVER_PERSONAL_ACCESS_TOKEN_VERIFIER'],
		port: environment['ORCHBEE_SERVER_PORT'],
		rendererRoot: environment['ORCHBEE_SERVER_RENDERER_ROOT'],
		sessionSigningKey: environment['ORCHBEE_SERVER_SESSION_SIGNING_KEY'],
	});
}
