import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';

import { resolveServerConfiguration } from '../resolveServerConfiguration';

const rendererRoot = join(tmpdir(), 'orchbee-renderer');
const authenticationEnvironment = {
	ORCHBEE_SERVER_PERSONAL_ACCESS_TOKEN_VERIFIER: createHash('sha256')
		.update('personal-access-token', 'utf8')
		.digest('base64url'),
	ORCHBEE_SERVER_SESSION_SIGNING_KEY: 's'.repeat(43),
};

describe('resolveServerConfiguration', () => {
	it('defaults to an ephemeral loopback listener', () => {
		expect(
			resolveServerConfiguration({
				...authenticationEnvironment,
				ORCHBEE_SERVER_RENDERER_ROOT: rendererRoot,
			}),
		).toEqual({
			host: '127.0.0.1',
			personalAccessTokenVerifier:
				authenticationEnvironment.ORCHBEE_SERVER_PERSONAL_ACCESS_TOKEN_VERIFIER,
			port: 0,
			rendererRoot,
			sessionSigningKey: authenticationEnvironment.ORCHBEE_SERVER_SESSION_SIGNING_KEY,
		});
	});

	it('accepts an explicit loopback port', () => {
		expect(
			resolveServerConfiguration({
				...authenticationEnvironment,
				ORCHBEE_SERVER_HOST: '127.0.0.1',
				ORCHBEE_SERVER_PORT: '43110',
				ORCHBEE_SERVER_RENDERER_ROOT: rendererRoot,
			}).port,
		).toBe(43_110);
	});

	it('accepts the explicit LAN-preview listener and rejects invalid deployment inputs', () => {
		expect(
			resolveServerConfiguration({
				...authenticationEnvironment,
				ORCHBEE_SERVER_HOST: '0.0.0.0',
				ORCHBEE_SERVER_RENDERER_ROOT: rendererRoot,
			}),
		).toMatchObject({ host: '0.0.0.0', port: 0, rendererRoot });
		expect(() =>
			resolveServerConfiguration({
				...authenticationEnvironment,
				ORCHBEE_SERVER_HOST: '192.168.1.14',
				ORCHBEE_SERVER_RENDERER_ROOT: rendererRoot,
			}),
		).toThrow(/127\.0\.0\.1|0\.0\.0\.0/u);
		expect(() =>
			resolveServerConfiguration({
				...authenticationEnvironment,
				ORCHBEE_SERVER_PORT: '65536',
				ORCHBEE_SERVER_RENDERER_ROOT: rendererRoot,
			}),
		).toThrow(/65_?535|65535/u);
		expect(() =>
			resolveServerConfiguration({
				...authenticationEnvironment,
				ORCHBEE_SERVER_RENDERER_ROOT: 'apps/web/dist',
			}),
		).toThrow(/absolute path/u);
		expect(() => resolveServerConfiguration({})).toThrow(/expected string/u);
	});
});
