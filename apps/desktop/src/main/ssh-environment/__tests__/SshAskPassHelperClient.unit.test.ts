import { mkdtemp, rm } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, describe, expect, it, vi } from 'vitest';

import { SshAskPassHelperClient } from '../SshAskPassHelperClient';

const cleanupTasks: Array<() => Promise<void>> = [];

afterEach(async () => {
	await Promise.all(cleanupTasks.splice(0).map((cleanup) => cleanup()));
});

describe('SshAskPassHelperClient', () => {
	it('exchanges one capability-bound request and writes only the submitted response', async () => {
		const directory = await mkdtemp(join(tmpdir(), 'orchbee-askpass-helper-test-'));
		const socketPath = join(directory, 'a.s');
		const capability = 'a'.repeat(64);
		const server = createServer((socket) => {
			let request = '';
			socket.setEncoding('utf8');
			socket.on('data', (chunk) => {
				request += chunk;
				if (!request.endsWith('\n')) return;
				expect(JSON.parse(request)).toEqual({
					version: 1,
					capability,
					kind: 'none',
					prompt: '(builder@host) Password:',
				});
				socket.end('{"kind":"submitted","response":"one-use secret"}\n');
			});
		});
		await new Promise<void>((resolve, reject) => {
			server.once('error', reject);
			server.listen(socketPath, () => resolve());
		});
		cleanupTasks.push(async () => {
			await new Promise<void>((resolve) => {
				server.close(() => resolve());
			});
			await rm(directory, { force: true, recursive: true });
		});
		const writeOutput = vi.fn<(output: string) => void>();
		const client = new SshAskPassHelperClient({
			arguments: ['(builder@host) Password:'],
			environment: {
				ORCHBEE_ASKPASS_CAPABILITY: capability,
				ORCHBEE_ASKPASS_SOCKET: socketPath,
				SSH_ASKPASS_PROMPT: 'none',
			},
			writeOutput,
		});

		await expect(client.run()).resolves.toBe(0);
		expect(writeOutput).toHaveBeenCalledExactlyOnceWith('one-use secret\n');
	});
});
