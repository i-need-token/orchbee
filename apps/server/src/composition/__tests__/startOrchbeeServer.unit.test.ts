import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import {
	BrowserSessionSigningKeySchema,
	PersonalAccessTokenVerifierSchema,
} from '@orchbee/remote-client-protocol';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import type { RunningOrchbeeServer } from '../startOrchbeeServer';
import { startOrchbeeServer } from '../startOrchbeeServer';

let rendererRoot = '';
let runningServer: RunningOrchbeeServer | undefined;
const authentication = {
	personalAccessTokenVerifier: PersonalAccessTokenVerifierSchema.parse('v'.repeat(43)),
	sessionSigningKey: BrowserSessionSigningKeySchema.parse('s'.repeat(43)),
};

afterEach(async () => {
	await runningServer?.close();
	runningServer = undefined;
	if (rendererRoot !== '') await rm(rendererRoot, { force: true, recursive: true });
	rendererRoot = '';
});

describe('startOrchbeeServer', () => {
	it('returns the selected loopback origin and a closeable listener', async () => {
		rendererRoot = await mkdtemp(join(tmpdir(), 'orchbee-running-server-'));
		await mkdir(join(rendererRoot, 'static'));
		await writeFile(join(rendererRoot, 'index.html'), '<main>running</main>');
		runningServer = await startOrchbeeServer({
			...authentication,
			host: '127.0.0.1',
			port: 0,
			rendererRoot,
		});

		const response = await fetch(`${runningServer.origin}/login`, {
			headers: { accept: 'text/html' },
		});

		expect(runningServer.origin).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/u);
		expect(runningServer).toMatchObject({ host: '127.0.0.1', port: expect.any(Number) });
		expect(await response.text()).toContain('running');

		await runningServer.close();
		runningServer = undefined;
		await expect(fetch(`${response.url}/after-close`)).rejects.toThrow(/fetch failed/u);
	});

	it('binds the explicit LAN-preview listener while retaining a closeable handle', async () => {
		rendererRoot = await mkdtemp(join(tmpdir(), 'orchbee-running-server-'));
		await mkdir(join(rendererRoot, 'static'));
		await writeFile(join(rendererRoot, 'index.html'), '<main>lan-preview</main>');
		runningServer = await startOrchbeeServer({
			...authentication,
			host: '0.0.0.0',
			port: 0,
			rendererRoot,
		});

		const response = await fetch(`http://127.0.0.1:${runningServer.port}/login`, {
			headers: { accept: 'text/html' },
		});

		expect(runningServer.host).toBe('0.0.0.0');
		expect(await response.text()).toContain('lan-preview');
	});
});
