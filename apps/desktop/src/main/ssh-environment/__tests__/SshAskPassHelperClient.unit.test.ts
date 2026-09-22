import { randomUUID } from 'node:crypto';
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
		const socketPath =
			process.platform === 'win32'
				? `\\\\.\\pipe\\orchbee-askpass-helper-test-${randomUUID()}`
				: join(directory, 'a.s');
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
		cleanupTasks.push(async () => {
			await new Promise<void>((resolve) => {
				server.close(() => resolve());
			});
			await rm(directory, { force: true, recursive: true });
		});
		await new Promise<void>((resolve, reject) => {
			server.once('error', reject);
			server.listen(socketPath, () => resolve());
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

	it('recovers the canonical OpenSSH host confirmation kind when the platform omits its hint', async () => {
		const directory = await mkdtemp(join(tmpdir(), 'orchbee-askpass-helper-test-'));
		const socketPath =
			process.platform === 'win32'
				? `\\\\.\\pipe\\orchbee-askpass-helper-test-${randomUUID()}`
				: join(directory, 'a.s');
		const capability = 'b'.repeat(64);
		const prompt = [
			"The authenticity of host 'builder.example' can't be established.",
			'ED25519 key fingerprint is SHA256:example.',
			'Are you sure you want to continue connecting (yes/no/[fingerprint])? ',
		].join('\n');
		const server = createServer((socket) => {
			let request = '';
			socket.setEncoding('utf8');
			socket.on('data', (chunk) => {
				request += chunk;
				if (!request.endsWith('\n')) return;
				expect(JSON.parse(request)).toEqual({
					version: 1,
					capability,
					kind: 'confirm',
					prompt,
				});
				socket.end('{"kind":"submitted","response":"yes"}\n');
			});
		});
		cleanupTasks.push(async () => {
			await new Promise<void>((resolve) => {
				server.close(() => resolve());
			});
			await rm(directory, { force: true, recursive: true });
		});
		await new Promise<void>((resolve, reject) => {
			server.once('error', reject);
			server.listen(socketPath, () => resolve());
		});
		const writeOutput = vi.fn<(output: string) => void>();
		const client = new SshAskPassHelperClient({
			arguments: [prompt],
			environment: {
				ORCHBEE_ASKPASS_CAPABILITY: capability,
				ORCHBEE_ASKPASS_SOCKET: socketPath,
			},
			writeOutput,
		});

		await expect(client.run()).resolves.toBe(0);
		expect(writeOutput).toHaveBeenCalledExactlyOnceWith('yes\n');
	});
});
