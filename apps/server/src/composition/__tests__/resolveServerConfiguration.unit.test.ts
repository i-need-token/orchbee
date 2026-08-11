import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

import { resolveServerConfiguration } from '../resolveServerConfiguration';

const rendererRoot = join(tmpdir(), 'orchbee-renderer');

describe('resolveServerConfiguration', () => {
	it('defaults to an ephemeral loopback listener', () => {
		expect(resolveServerConfiguration({ ORCHBEE_SERVER_RENDERER_ROOT: rendererRoot })).toEqual({
			host: '127.0.0.1',
			port: 0,
			rendererRoot,
		});
	});

	it('accepts an explicit loopback port', () => {
		expect(
			resolveServerConfiguration({
				ORCHBEE_SERVER_HOST: '127.0.0.1',
				ORCHBEE_SERVER_PORT: '43110',
				ORCHBEE_SERVER_RENDERER_ROOT: rendererRoot,
			}).port,
		).toBe(43_110);
	});

	it('accepts the explicit LAN-preview listener and rejects invalid deployment inputs', () => {
		expect(
			resolveServerConfiguration({
				ORCHBEE_SERVER_HOST: '0.0.0.0',
				ORCHBEE_SERVER_RENDERER_ROOT: rendererRoot,
			}),
		).toEqual({ host: '0.0.0.0', port: 0, rendererRoot });
		expect(() =>
			resolveServerConfiguration({
				ORCHBEE_SERVER_HOST: '192.168.1.14',
				ORCHBEE_SERVER_RENDERER_ROOT: rendererRoot,
			}),
		).toThrow(/127\.0\.0\.1|0\.0\.0\.0/u);
		expect(() =>
			resolveServerConfiguration({
				ORCHBEE_SERVER_PORT: '65536',
				ORCHBEE_SERVER_RENDERER_ROOT: rendererRoot,
			}),
		).toThrow(/65_?535|65535/u);
		expect(() =>
			resolveServerConfiguration({ ORCHBEE_SERVER_RENDERER_ROOT: 'apps/web/dist' }),
		).toThrow(/absolute path/u);
		expect(() => resolveServerConfiguration({})).toThrow(/expected string/u);
	});
});
