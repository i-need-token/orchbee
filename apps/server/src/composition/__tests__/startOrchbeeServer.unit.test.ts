import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import type { RunningOrchbeeServer } from '../startOrchbeeServer';
import { startOrchbeeServer } from '../startOrchbeeServer';

let rendererRoot = '';
let runningServer: RunningOrchbeeServer | undefined;

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
			host: '127.0.0.1',
			port: 0,
			rendererRoot,
		});

		const response = await fetch(`${runningServer.origin}/settings`, {
			headers: { accept: 'text/html' },
		});

		expect(runningServer.origin).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/u);
		expect(await response.text()).toContain('running');

		await runningServer.close();
		runningServer = undefined;
		await expect(fetch(`${response.url}/after-close`)).rejects.toThrow(/fetch failed/u);
	});
});
