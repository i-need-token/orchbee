import type { ChildProcessWithoutNullStreams, SpawnOptionsWithoutStdio } from 'node:child_process';
import type * as childProcessModule from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { NativeUserPreferencesClient } from '../NativeUserPreferencesClient';

const launch = vi.hoisted(() =>
	vi.fn<
		(
			executable: string,
			arguments_: readonly string[],
			options: SpawnOptionsWithoutStdio,
		) => ChildProcessWithoutNullStreams
	>(),
);
vi.mock('node:child_process', async (original) => ({
	...(await original<typeof childProcessModule>()),
	spawn: launch,
}));

// A deliberately small substitute peer exercises actual pipes, close, deadline and process reaping.
// Rust application tests separately exercise the real native helper and guarded persistence.
const peer = `
for await (const chunk of process.stdin) { /* consume the one closed request */ }
const mode = process.env.ORCHBEE_SETTINGS_BRIDGE_TEST_MODE;
if (mode === 'wait') setInterval(() => {}, 1000);
else if (mode === 'overflow') {
  process.stdout.write('x'.repeat(2 * 1024 * 1024 + 2)); setInterval(() => {}, 1000);
} else if (mode === 'invalid') process.stdout.write(Buffer.from([255]));
else if (mode === 'shell') process.stdout.write(JSON.stringify(process.execPath));
else {
  process.stdout.write(JSON.stringify({kind: 'refused', reason: 'source-changed'}) + '\\n');
  process.exitCode = 1;
}
`;

describe('verified native preference exchange', () => {
	let directory = '';
	let mode = 'refused';
	let client: NativeUserPreferencesClient | undefined;
	const children: ChildProcessWithoutNullStreams[] = [];
	beforeEach(async () => {
		directory = await mkdtemp(path.join(tmpdir(), 'orchbee-preferences-bridge-test-'));
		const executable = process.platform === 'win32' ? 'orchbee-settings.exe' : 'orchbee-settings';
		const contents = Buffer.from('verified fixture artifact');
		await writeFile(path.join(directory, executable), contents);
		await writeFile(
			path.join(directory, 'manifest.json'),
			JSON.stringify({
				version: 1,
				executable,
				sha256: createHash('sha256').update(contents).digest('hex'),
			}),
		);
		mode = 'refused';
		children.length = 0;
		const actual = await vi.importActual<typeof childProcessModule>('node:child_process');
		launch.mockReset();
		launch.mockImplementation((_executable, _arguments, options) => {
			const child = actual.spawn(process.execPath, ['--input-type=module', '-e', peer], {
				...options,
				env: { ...options.env, ORCHBEE_SETTINGS_BRIDGE_TEST_MODE: mode },
			});
			children.push(child);
			return child;
		});
		client = new NativeUserPreferencesClient({
			artifactDirectory: directory,
			environment: process.env,
		});
	});
	afterEach(async () => {
		await client?.dispose();
		for (const child of children) {
			if (child.exitCode === null && child.signalCode === null)
				throw new Error('The preference client left its child running after disposal.');
		}
		await rm(directory, { recursive: true });
	});
	it('uses the same verified artifact for a read-only system shell resolution', async () => {
		mode = 'shell';
		expect(await client?.resolveSystemShell()).toBe(process.execPath);
		expect(launch.mock.calls[0]?.[1]).toEqual(['--system-shell-v1']);
	});
	it('waits for shell discovery before handing a preference request off once', async () => {
		mode = 'shell';
		const shell = client?.resolveSystemShell();
		const request = client?.execute({ operation: 'read' });
		expect(await shell).toBe(process.execPath);
		mode = 'refused';
		expect(await request).toEqual({
			kind: 'response',
			response: { kind: 'refused', reason: 'source-changed' },
		});
		expect(launch.mock.calls.map((call) => call[1])).toEqual([
			['--system-shell-v1'],
			['--stdio-v1'],
		]);
	});

	it('uses one verified absolute artifact with hidden, shell-free, pipe-only launch', async () => {
		expect(await client?.execute({ operation: 'read' })).toEqual({
			kind: 'response',
			response: { kind: 'refused', reason: 'source-changed' },
		});
		expect(launch).toHaveBeenCalledTimes(1);
		const call = launch.mock.calls[0];
		expect(call?.[0]).toBe(
			path.join(
				directory,
				process.platform === 'win32' ? 'orchbee-settings.exe' : 'orchbee-settings',
			),
		);
		expect(call?.[1]).toEqual(['--stdio-v1']);
		expect(call?.[2]).toMatchObject({
			cwd: directory,
			shell: false,
			stdio: ['pipe', 'pipe', 'pipe'],
			windowsHide: true,
		});
	});
	it('refuses altered artifacts before starting any process', async () => {
		await writeFile(
			path.join(
				directory,
				process.platform === 'win32' ? 'orchbee-settings.exe' : 'orchbee-settings',
			),
			'changed',
		);
		expect(await client?.execute({ operation: 'read' })).toEqual({ kind: 'unavailable' });
		expect(launch).not.toHaveBeenCalled();
	});
	it('reports a lost write response as unknown and never replays it', async () => {
		mode = 'invalid';
		expect(
			await client?.execute({
				operation: 'update',
				source: 'missing',
				changes: { soundEnabled: true },
			}),
		).toEqual({ kind: 'outcome-unknown' });
		expect(launch).toHaveBeenCalledTimes(1);
	});
	it('terminates an oversized response and joins the writer', async () => {
		mode = 'overflow';
		expect(
			await client?.execute({
				operation: 'update',
				source: 'missing',
				changes: { soundEnabled: true },
			}),
		).toEqual({ kind: 'outcome-unknown' });
		expect(launch).toHaveBeenCalledTimes(1);
	});
	it('bounds a silent peer and keeps a second request out of the active exchange', async () => {
		mode = 'wait';
		const pending = client?.execute({ operation: 'read' });
		expect(await client?.execute({ operation: 'read' })).toEqual({
			kind: 'response',
			response: { kind: 'refused', reason: 'busy' },
		});
		expect(await pending).toEqual({ kind: 'unavailable' });
		expect(launch).toHaveBeenCalledTimes(1);
	}, 5000);
	it('waits for an active read before shell discovery and joins queued discovery on disposal', async () => {
		mode = 'wait';
		const pending = client?.execute({ operation: 'read' });
		await vi.waitFor(() => expect(launch).toHaveBeenCalledOnce());
		const discovery = client?.resolveSystemShell();
		await client?.dispose();
		expect(await pending).toEqual({ kind: 'unavailable' });
		expect(await discovery).toBeNull();
		expect(launch).toHaveBeenCalledOnce();
	});
	it('disposal cancels and joins a submitted writer without claiming rollback', async () => {
		mode = 'wait';
		const pending = client?.execute({
			operation: 'update',
			source: 'missing',
			changes: { soundEnabled: true },
		});
		await vi.waitFor(() => expect(launch).toHaveBeenCalledTimes(1));
		await client?.dispose();
		expect(await pending).toEqual({ kind: 'outcome-unknown' });
		expect(await client?.execute({ operation: 'read' })).toEqual({ kind: 'unavailable' });
	});
});
