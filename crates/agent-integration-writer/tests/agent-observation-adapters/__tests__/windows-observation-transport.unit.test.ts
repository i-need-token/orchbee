import { execFile, spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { createServer, type Socket } from 'node:net';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { promisify } from 'node:util';

import { describe, expect, it, vi } from 'vitest';
import { z } from 'zod';

const resourceRoot = path.resolve(
	import.meta.dirname,
	'../../../agent-observation-adapters/bundle',
);
const transportUrl = pathToFileURL(
	path.join(resourceRoot, 'scripts/observation-transport.mjs'),
).href;
const endpointName = 'ORCHBEE_AGENT_OBSERVATION_SOCKET';
const consoleTarget = String.raw`\\.\CONOUT$`;
const endpoint = String.raw`\\.\pipe\orchbee-0123456789abcdef0123456789abcdef-observation`;
const ack = '{"accepted":true,"version":1}\n';
const observation = {
	activity: null,
	agentKey: null,
	event: 'session-started',
	operationKey: null,
	sessionKey: 'windows-session-1',
	turnKey: null,
	version: 1,
};

describe('Windows Agent observation transport', () => {
	it('admits only the exact injected native observation pipe without SSH discovery', async () => {
		const transport = await import(transportUrl);
		await expect(
			transport.discoverObservationSockets({ [endpointName]: endpoint }, { platform: 'win32' }),
		).resolves.toEqual([endpoint]);
		for (const candidate of [
			undefined,
			'',
			String.raw`\\remote\pipe\orchbee-0123456789abcdef0123456789abcdef-observation`,
			endpoint.replace('-observation', '-client'),
			endpoint.replace('012345', 'ABCDEF'),
			`${endpoint}\u0000`,
			`${endpoint}\n`,
			`${endpoint}\\child`,
			'C:\\ordinary-file',
			'/tmp/orchbee-agent-v1-forwarded.sock',
		]) {
			await expect(
				transport.discoverObservationSockets(
					{ [endpointName]: candidate, SSH_CONNECTION: 'remote', SSH_TTY: '/dev/pts/1' },
					{ platform: 'win32' },
				),
			).resolves.toEqual([]);
		}
	});

	it('selects only the fixed explicit console device and never resolves an inner Unix mux', async () => {
		const transport = await import(transportUrl);
		const execute = vi.fn<() => Promise<string>>();
		await expect(
			transport.resolveObservationTargets(
				{ SSH_TTY: '/dev/pts/1', TMUX_PANE: '%1', ZELLIJ_SESSION_NAME: 'z', ZELLIJ_PANE_ID: '1' },
				{ execute, platform: 'win32' },
			),
		).resolves.toEqual([{ tty: consoleTarget, visibility: 'unknown' }]);
		expect(execute).not.toHaveBeenCalled();
	});

	it('writes a matching console hint only after an exact positive delivery result', async () => {
		const transport = await import(transportUrl);
		for (const accepted of [false, true]) {
			const order: string[] = [];
			const envelopes: unknown[] = [];
			const hints: unknown[] = [];
			let identity = 0;
			await transport.forwardSanitizedObservation('codex', observation, {
				allocateIdentity: () => `windows-identity-${++identity}`,
				discoverSockets: async () => [endpoint],
				resolveTargets: async () => [{ tty: consoleTarget, visibility: 'unknown' }],
				sendEnvelope: async (_target: string, envelope: unknown) => {
					order.push('envelope');
					envelopes.push(envelope);
					return accepted;
				},
				writeHint: async (target: string, hint: unknown) => {
					order.push('hint');
					hints.push({ target, hint });
				},
			});
			expect(order).toEqual(accepted ? ['envelope', 'hint'] : ['envelope']);
			expect(envelopes).toEqual([
				{
					adapterId: 'codex',
					correlations: [{ correlationId: 'windows-identity-2', visibility: 'unknown' }],
					eventId: 'windows-identity-1',
					observation,
					version: 1,
				},
			]);
			expect(hints).toEqual(
				accepted
					? [
							{
								target: consoleTarget,
								hint: {
									correlationId: 'windows-identity-2',
									eventId: 'windows-identity-1',
									kind: 'correlation',
									version: 1,
								},
							},
						]
					: [],
			);
		}
	});

	it.runIf(process.platform === 'win32')(
		'receives fragmented native pipe ACKs and rejects false, excess or absent ACKs',
		async () => {
			const transport = await import(transportUrl);
			for (const [response, accepted] of [
				[ack, true],
				['{"accepted":false,"version":1}\n', false],
				[`${ack}x`, false],
				['', false],
			] as const) {
				expect(
					await withPipe(response, (pipe) =>
						transport.sendObservationEnvelope(pipe, { version: 1 }),
					),
				).toBe(accepted);
			}
		},
	);

	it.runIf(process.platform === 'win32')(
		'never writes a hint to a relative console name or redirected ordinary path',
		async () => {
			const transport = await import(transportUrl);
			for (const target of ['CONOUT$', process.cwd(), '/dev/tty', String.raw`\\remote\CONOUT$`]) {
				await expect(
					transport.writeCorrelationHint(target, { eventId: 'ignored' }),
				).resolves.toBeUndefined();
			}
		},
	);

	it.runIf(process.platform === 'win32')(
		'runs every Codex Windows hook silently in each shell Codex uses when the plugin runtime is missing',
		async () => {
			for (const command of await codexWindowsCommands()) {
				for (const shell of codexHookShells(command)) {
					const child = spawnSync(shell.file, shell.args, {
						windowsVerbatimArguments: shell.verbatim,
						encoding: 'utf8',
						env: { ...process.env, PLUGIN_ROOT: path.join(resourceRoot, 'missing-plugin-root') },
						input: '{}',
						timeout: 10_000,
						windowsHide: true,
					});
					expect({
						shell: shell.file,
						error: child.error,
						status: child.status,
						stdout: child.stdout,
						stderr: child.stderr,
					}).toEqual({ shell: shell.file, error: undefined, status: 0, stdout: '', stderr: '' });
				}
			}
		},
	);

	it.runIf(process.platform === 'win32')(
		'delivers only sanitized data from the Codex Windows hook in each shell Codex uses',
		async () => {
			const commands = await codexWindowsCommands();
			for (const command of commands) {
				for (const shell of codexHookShells(command)) {
					// Codex canonicalizes an explicit CODEX_HOME, which gives a verbatim plugin root.
					for (const pluginRoot of [resourceRoot, `\\\\?\\${resourceRoot}`]) {
						const envelopes: string[] = [];
						// Reject the envelope so this process test never writes to the runner's console.
						await withPipe(
							'{"accepted":false,"version":1}\n',
							async (pipe) => {
								const child = promisify(execFile)(shell.file, shell.args, {
									// The source Capsule has no owner record, so it is release's.
									env: {
										...process.env,
										ORCHBEE_ENVIRONMENT: 'orchbee',
										PLUGIN_ROOT: pluginRoot,
										[endpointName]: pipe,
									},
									timeout: 10_000,
									windowsHide: true,
									windowsVerbatimArguments: shell.verbatim,
								});
								child.child.stdin?.end(
									JSON.stringify({
										hook_event_name: 'SessionStart',
										session_id: 'windows-session-1',
										prompt: 'PRIVATE_PROMPT',
										cwd: 'PRIVATE_DIRECTORY',
									}),
								);
								const output = await child;
								expect(output.stdout).toBe('');
								expect(output.stderr).toBe('');
								return true;
							},
							(payload) => envelopes.push(payload),
						);
						expect({ shell: shell.file, pluginRoot, count: envelopes.length }).toEqual({
							shell: shell.file,
							pluginRoot,
							count: 1,
						});
						const envelope = z
							.object({
								adapterId: z.literal('codex'),
								observation: z.object({
									sessionKey: z.literal('windows-session-1'),
									event: z.literal('session-started'),
								}),
							})
							.parse(JSON.parse(envelopes[0] ?? 'null'));
						expect(envelope.adapterId).toBe('codex');
						expect(envelopes.join('')).not.toContain('PRIVATE_');
					}
				}
			}
		},
	);
});

async function codexWindowsCommands(): Promise<Set<string>> {
	const hooks = z
		.object({
			hooks: z.record(
				z.string(),
				z.array(z.object({ hooks: z.array(z.object({ commandWindows: z.string() })) })),
			),
		})
		.parse(JSON.parse(await readFile(path.join(resourceRoot, 'hooks/codex.json'), 'utf8')));
	return new Set(
		Object.values(hooks.hooks).flatMap((groups) =>
			groups.flatMap((group) => group.hooks.map((hook) => hook.commandWindows)),
		),
	);
}

/**
 * Codex runs a Windows hook through its session shell: PowerShell (pwsh, else Windows
 * PowerShell) with `-NoProfile -Command`, or cmd with `/c` and the command quoted raw.
 */
function codexHookShells(
	command: string,
): Array<{ file: string; args: string[]; verbatim: boolean }> {
	const systemRoot = process.env['SystemRoot'] ?? String.raw`C:\Windows`;
	const shells = [
		{
			file: path.join(systemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe'),
			args: ['-NoProfile', '-Command', command],
			verbatim: false,
		},
		{
			file: process.env['COMSPEC'] ?? 'cmd.exe',
			args: ['/c', `"${command}"`],
			verbatim: true,
		},
	];
	const pwsh = spawnSync('where.exe', ['pwsh.exe'], { encoding: 'utf8', windowsHide: true });
	const pwshPath = pwsh.status === 0 ? pwsh.stdout.split(/\r?\n/u)[0]?.trim() : undefined;
	if (pwshPath !== undefined && pwshPath !== '') {
		shells.push({ file: pwshPath, args: ['-NoProfile', '-Command', command], verbatim: false });
	}
	return shells;
}

async function withPipe(
	response: string,
	send: (pipe: string) => Promise<boolean>,
	observe?: (payload: string) => void,
): Promise<boolean> {
	const pipe = String.raw`\\.\pipe\orchbee-${randomBytes(16).toString('hex')}-observation`;
	const sockets = new Set<Socket>();
	const server = createServer({ allowHalfOpen: true }, (socket) => {
		sockets.add(socket);
		socket.once('close', () => sockets.delete(socket));
		socket.once('data', (data: Buffer) => {
			observe?.(data.toString('utf8'));
			if (response === '') return;
			socket.write(response.slice(0, 7));
			socket.end(response.slice(7));
		});
	});
	try {
		await new Promise<void>((resolve, reject) => {
			server.once('error', reject);
			server.listen(pipe, resolve);
		});
		return await send(pipe);
	} finally {
		for (const socket of sockets) socket.destroy();
		await new Promise<void>((resolve, reject) =>
			server.close((error) => (error ? reject(error) : resolve())),
		);
	}
}
