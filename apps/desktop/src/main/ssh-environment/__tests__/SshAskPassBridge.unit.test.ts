import { once } from 'node:events';
import { access, stat } from 'node:fs/promises';
import { connect } from 'node:net';
import { dirname } from 'node:path';

import { describe, expect, it, vi } from 'vitest';

import { SshAskPassBridge, type SshAskPassBridgeOptions } from '../SshAskPassBridge';

describe('SSH AskPass bridge', () => {
	it('uses an owner-only one-generation capability and removes its IPC on disposal', async () => {
		const interaction = vi.fn<SshAskPassBridgeOptions['interaction']>().mockResolvedValue({
			kind: 'submitted' as const,
			response: 'transient secret',
		});
		const bridge = new SshAskPassBridge({
			helperEntryPath: '/tmp/orchbee-askpass-helper.cjs',
			interaction,
			runtimeExecutablePath: '/usr/bin/node',
			temporaryRoot: '/tmp',
		});

		const launch = await bridge.start();
		const socketPath = launch.environment['ORCHBEE_ASKPASS_SOCKET'];
		const capability = launch.environment['ORCHBEE_ASKPASS_CAPABILITY'];
		expect(socketPath).toBeDefined();
		expect(capability).toMatch(/^[0-9a-f]{64}$/u);
		expect(launch.environment).toMatchObject({
			DISPLAY: 'orchbee-askpass',
			SSH_ASKPASS_REQUIRE: 'force',
		});
		if (socketPath === undefined || capability === undefined) throw new Error('missing test IPC');
		const [directoryStatus, socketStatus] = await Promise.all([
			stat(dirname(socketPath)),
			stat(socketPath),
		]);
		expect(directoryStatus.mode & 0o777).toBe(0o700);
		expect(socketStatus.mode & 0o777).toBe(0o600);

		await expect(
			request(socketPath, {
				version: 1,
				capability,
				kind: 'none',
				prompt: '(builder@host) Password:',
			}),
		).resolves.toEqual({ kind: 'submitted', response: 'transient secret' });
		expect(interaction).toHaveBeenCalledWith({
			handle: expect.stringMatching(/^ssh-prompt-[0-9a-f]{32}$/u),
			kind: 'none',
			prompt: '(builder@host) Password:',
		});

		await bridge.dispose();
		await expect(access(dirname(socketPath))).rejects.toThrow(/ENOENT/u);
	});

	it('rejects a request with the wrong capability without presenting its prompt', async () => {
		const interaction = vi.fn<SshAskPassBridgeOptions['interaction']>();
		const bridge = new SshAskPassBridge({
			helperEntryPath: '/tmp/orchbee-askpass-helper.cjs',
			interaction,
			runtimeExecutablePath: '/usr/bin/node',
			temporaryRoot: '/tmp',
		});
		const launch = await bridge.start();
		const socketPath = launch.environment['ORCHBEE_ASKPASS_SOCKET'];
		if (socketPath === undefined) throw new Error('missing test IPC');

		await expect(
			request(socketPath, {
				version: 1,
				capability: '0'.repeat(64),
				kind: 'none',
				prompt: 'must not be presented',
			}),
		).rejects.toThrow(/./u);
		expect(interaction).not.toHaveBeenCalled();
		await bridge.dispose();
	});
});

function request(socketPath: string, payload: unknown): Promise<unknown> {
	return new Promise((resolve, reject) => {
		const socket = connect(socketPath);
		let response = '';
		socket.setEncoding('utf8');
		socket.once('connect', () => socket.write(`${JSON.stringify(payload)}\n`));
		socket.on('data', (chunk) => {
			response += chunk;
		});
		socket.once('end', () => {
			try {
				resolve(JSON.parse(response));
			} catch (error: unknown) {
				reject(error);
			}
		});
		socket.once('error', reject);
		void once(socket, 'close');
	});
}
