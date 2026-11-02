import { randomUUID } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { closeSync, openSync } from 'node:fs';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createServer, type Socket } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { describe, expect, it, vi } from 'vitest';

import { AgentHookRichObservationEnvelopeSchema } from '../observation-envelope';

const resourceRoot = path.resolve(
	import.meta.dirname,
	'../../../agent-observation-adapters/bundle',
);

// Fixture-only bounds protecting test execution; the shipped hook `timeout`
// fields are the harness-owned product budgets and are not measured here.
const FIXTURE_DEADLINE_MS = 30_000;
const SPAWN_WATCHDOG_MS = 5_000;
const transportUrl = resourceUrl('scripts/observation-transport.mjs');
const sanitizersUrl = resourceUrl('scripts/hook-sanitizers.mjs');
const openCodeUrl = resourceUrl('opencode/orchbee-agent-observation.mjs');
const piUrl = resourceUrl('pi/orchbee-agent-observation.mjs');
const piEntryUrl = resourceUrl('pi/orchbee-agent-observation.cjs');
const ompUrl = resourceUrl('omp/orchbee-agent-observation.mjs');
const ompEntryUrl = resourceUrl('omp/orchbee-agent-observation.cjs');
const clineUrl = resourceUrl('cline/orchbee-agent-observation.js');

describe('Mastracode lifecycle boundary', () => {
	it('keeps run identity and permission correlation without private content', async () => {
		const { sanitizeAgentHookInput: sanitize } = await import(sanitizersUrl);
		const base = {
			session_id: 'session-1',
			run_id: 'run-1',
			tool_call_id: 'call-1',
			tool_name: 'shell',
			permission_kind: 'tool_approval',
			tool_input: 'PRIVATE',
			result: 'PRIVATE',
			task: 'PRIVATE',
		};
		for (const [hook, event] of [
			['SessionStart', 'session-identified'],
			['SessionEnd', 'session-ended'],
			['AgentStart', 'turn-started'],
			['PreToolUse', 'tool-started'],
			['PostToolUse', 'tool-finished'],
			['PermissionRequest', 'approval-requested'],
			['SubagentStart', 'subagent-started'],
			['SubagentEnd', 'subagent-stopped'],
		]) {
			const result = sanitize('mastracode', { ...base, hook_event_name: hook });
			expect(result).toMatchObject({ event, sessionKey: 'session-1' });
			expect(JSON.stringify(result)).not.toContain('PRIVATE');
		}
		for (const decision of [
			'approved',
			'declined',
			'dismissed',
			'auto_approved',
			'auto_declined',
		]) {
			expect(
				sanitize('mastracode', { ...base, hook_event_name: 'PermissionResult', decision }),
			).toMatchObject({ event: 'attention-resolved', operationKey: 'call-1', turnKey: 'run-1' });
		}
		for (const [stop_reason, event] of [
			['complete', 'turn-stopped'],
			['aborted', 'turn-canceled'],
			['error', 'turn-failed'],
		]) {
			expect(
				sanitize('mastracode', { ...base, hook_event_name: 'AgentEnd', stop_reason }),
			).toMatchObject({ event, turnKey: 'run-1' });
		}
		expect(
			sanitize('mastracode', { ...base, hook_event_name: 'PostToolUse', tool_error: true }),
		).toMatchObject({ event: 'tool-failed' });
		expect(
			sanitize('mastracode', { ...base, hook_event_name: 'PreToolUse', tool_call_id: undefined }),
		).toMatchObject({ event: 'tool-started', operationKey: null });
		for (const changed of [
			{ hook_event_name: 'AgentEnd', stop_reason: 'suspended' },
			{ hook_event_name: 'AgentEnd', stop_reason: 'future' },
			{ hook_event_name: 'AgentStart', run_id: undefined },
			{ hook_event_name: 'AgentStart', run_id: 'bad/id' },
			{ hook_event_name: 'PermissionRequest', tool_call_id: undefined },
			{ hook_event_name: 'PermissionRequest', permission_kind: 'future' },
			{ hook_event_name: 'PermissionResult', decision: 'future' },
			{ hook_event_name: 'PostToolUse', tool_error: 'false' },
			{ hook_event_name: 'SubagentEnd', tool_call_id: undefined },
			{ hook_event_name: 'UserPromptSubmit' },
			{ hook_event_name: 'Stop' },
			{ hook_event_name: 'Interrupt' },
		])
			expect(sanitize('mastracode', { ...base, ...changed })).toBeNull();
	});
});

describe('OpenCode-family observation boundary', () => {
	it('observes Antigravity conversation identity without requiring undocumented event fields', async () => {
		const sanitizers = await import(resourceUrl('scripts/hook-sanitizers.mjs'));
		const input = {
			conversationId: 'agy-conversation',
			invocationNum: 3,
			transcriptPath: 'PRIVATE',
			workspacePaths: ['PRIVATE'],
		};
		const result = sanitizers.sanitizeAgentHookInput('antigravity-cli', input);
		expect(result).toMatchObject({ event: 'session-identified', sessionKey: 'agy-conversation' });
		expect(JSON.stringify(result)).not.toContain('PRIVATE');
		for (const changed of [
			{ conversationId: '' },
			{ conversationId: 3 },
			{ hook_event_name: 'Stop' },
			{ hookEventName: 'PostInvocation' },
		]) {
			expect(
				sanitizers.sanitizeAgentHookInput('antigravity-cli', { ...input, ...changed }),
			).toBeNull();
		}
	});
	it('admits only interactive Hermes identities with CLI-only resume observations', async () => {
		const sanitizers = await import(resourceUrl('scripts/hook-sanitizers.mjs'));
		for (const event of ['on_session_start', 'on_session_reset', 'pre_llm_call']) {
			for (const platform of ['cli', 'tui', 'desktop', 'acp', 'gateway', null]) {
				const result = sanitizers.sanitizeAgentHookInput('hermes', {
					event,
					platform,
					session_id: 'hermes-session',
					messages: 'PRIVATE',
					tool_input: 'PRIVATE',
				});
				const admitted =
					platform !== null &&
					platform !== 'gateway' &&
					(event !== 'pre_llm_call' || platform === 'cli');
				expect(result).toEqual(
					admitted
						? {
								activity: null,
								agentKey: null,
								event: 'session-identified',
								operationKey: null,
								sessionKey: 'hermes-session',
								turnKey: null,
								version: 1,
							}
						: null,
				);
				expect(JSON.stringify(result)).not.toContain('PRIVATE');
			}
		}
		for (const session_id of ['', null, 'a/b', 'x'.repeat(129)])
			expect(
				sanitizers.sanitizeAgentHookInput('hermes', {
					event: 'on_session_start',
					platform: 'cli',
					session_id,
				}),
			).toBeNull();
		expect(
			sanitizers.sanitizeAgentHookInput('hermes', {
				event: 'post_llm_call',
				platform: 'cli',
				session_id: 'valid',
			}),
		).toBeNull();
	});
	it('sanitizes all six Devin identity hooks without guessing turn completion', async () => {
		const sanitizers = await import(resourceUrl('scripts/hook-sanitizers.mjs'));
		for (const hook_event_name of [
			'SessionStart',
			'UserPromptSubmit',
			'PreToolUse',
			'PostToolUse',
			'PermissionRequest',
			'Stop',
		]) {
			const input = {
				hook_event_name,
				session_id: 'devin-session',
				prompt_id: 'PRIVATE',
				cwd: 'PRIVATE',
				tool_input: { secret: 'PRIVATE' },
			};
			const result = sanitizers.sanitizeAgentHookInput('devin', input);
			expect(result).toMatchObject({ sessionKey: 'devin-session', event: 'session-identified' });
			expect(JSON.stringify(result)).not.toContain('PRIVATE');
			for (const session_id of [null, undefined, '', 'x'.repeat(129), 'bad id'])
				expect(sanitizers.sanitizeAgentHookInput('devin', { ...input, session_id })).toBeNull();
		}
		for (const hook_event_name of ['SessionEnd', 'PostCompaction', 'unknown', null])
			expect(
				sanitizers.sanitizeAgentHookInput('devin', {
					hook_event_name,
					session_id: 'devin-session',
				}),
			).toBeNull();
	});
	it('keeps Letta default conversations distinct and excludes prompt feedback', async () => {
		const sanitizers = await import(resourceUrl('scripts/hook-sanitizers.mjs'));
		const input = {
			event_type: 'SessionStart',
			is_new_session: false,
			conversation_id: 'default',
			agent_id: 'agent-a',
			working_directory: 'PRIVATE',
			agent_name: 'PRIVATE',
		};
		const first = sanitizers.sanitizeAgentHookInput('letta', input);
		const second = sanitizers.sanitizeAgentHookInput('letta', { ...input, agent_id: 'agent-b' });
		expect(first).toMatchObject({ event: 'session-identified', sessionKey: 'default:agent-a' });
		expect(second).toMatchObject({ sessionKey: 'default:agent-b' });
		expect(JSON.stringify(first)).not.toContain('PRIVATE');
		expect(
			sanitizers.sanitizeAgentHookInput('letta', { ...input, conversation_id: 'conversation-1' }),
		).toMatchObject({ sessionKey: 'conversation-1' });
		for (const changed of [
			{ agent_id: undefined },
			{ agent_id: '' },
			{ agent_id: 'x'.repeat(121) },
			{ conversation_id: 3 },
			{ is_new_session: 'true' },
			{ event_type: 'Stop' },
			{ event_type: undefined },
		]) {
			expect(sanitizers.sanitizeAgentHookInput('letta', { ...input, ...changed })).toBeNull();
		}
		expect(
			sanitizers.sanitizeAgentHookInput('letta', { ...input, agent_id: 'x'.repeat(120) }),
		).not.toBeNull();
	});
	it('observes Kimi session lifecycle without guessing turn completion', async () => {
		const sanitizers = await import(resourceUrl('scripts/hook-sanitizers.mjs'));
		for (const [hook, event] of [
			['SessionStart', 'session-identified'],
			['SessionEnd', 'session-ended'],
		]) {
			const result = sanitizers.sanitizeAgentHookInput('kimi', {
				hook_event_name: hook,
				session_id: 'kimi-1',
				cwd: 'PRIVATE',
				prompt: 'PRIVATE',
				session_title: 'PRIVATE',
			});
			expect(result).toMatchObject({ event, sessionKey: 'kimi-1' });
			expect(JSON.stringify(result)).not.toContain('PRIVATE');
		}
		for (const changed of [
			{ session_id: '' },
			{ session_id: 3 },
			{ hook_event_name: 'Stop' },
			{ hook_event_name: 'TurnStarted' },
			{ hook_event_name: undefined },
		]) {
			expect(
				sanitizers.sanitizeAgentHookInput('kimi', {
					hook_event_name: 'SessionStart',
					session_id: 'kimi-1',
					...changed,
				}),
			).toBeNull();
		}
	});
	it('limits Droid to exact SessionStart identity and omits content', async () => {
		const sanitizers = await import(resourceUrl('scripts/hook-sanitizers.mjs'));
		const input = {
			hook_event_name: 'SessionStart',
			session_id: 'droid-1',
			prompt: 'PRIVATE',
			source: 'compact',
		};
		const result = sanitizers.sanitizeAgentHookInput('droid', input);
		expect(result).toMatchObject({ event: 'session-identified', sessionKey: 'droid-1' });
		expect(JSON.stringify(result)).not.toContain('PRIVATE');
		for (const changed of [
			{ session_id: '' },
			{ session_id: 3 },
			{ hook_event_name: 'Stop' },
			{ hook_event_name: 'sessionStart' },
			{ hook_event_name: undefined },
		]) {
			expect(sanitizers.sanitizeAgentHookInput('droid', { ...input, ...changed })).toBeNull();
		}
	});
	it('limits Qoder to exact SessionStart identity and omits content', async () => {
		const sanitizers = await import(resourceUrl('scripts/hook-sanitizers.mjs'));
		const input = {
			hook_event_name: 'SessionStart',
			session_id: 'qoder-1',
			prompt: 'PRIVATE',
			source: 'compact',
		};
		const result = sanitizers.sanitizeAgentHookInput('qodercli', input);
		expect(result).toMatchObject({ event: 'session-identified', sessionKey: 'qoder-1' });
		expect(JSON.stringify(result)).not.toContain('PRIVATE');
		for (const changed of [
			{ session_id: '' },
			{ session_id: 3 },
			{ hook_event_name: 'Stop' },
			{ hook_event_name: 'sessionStart' },
			{ hook_event_name: undefined },
		]) {
			expect(sanitizers.sanitizeAgentHookInput('qodercli', { ...input, ...changed })).toBeNull();
		}
	});
	it('admits Cursor identity aliases only when every supplied value agrees', async () => {
		const sanitizers = await import(
			pathToFileURL(path.join(resourceRoot, 'scripts/hook-sanitizers.mjs')).href
		);
		const input = {
			session_id: 'cursor-session',
			conversation_id: 'cursor-session',
			hook_event_name: 'sessionStart',
			prompt: 'PRIVATE_TEXT',
			cwd: 'PRIVATE_PATH',
		};
		const observation = sanitizers.sanitizeAgentHookInput('cursor', input);
		expect(observation).toMatchObject({
			event: 'session-identified',
			sessionKey: 'cursor-session',
		});
		expect(JSON.stringify(observation)).not.toContain('PRIVATE_');
		for (const changed of [
			{ conversation_id: 'another-session' },
			{ session_id: '' },
			{ hook_event_name: 'stop' },
			{ hookEventName: 'sessionEnd' },
		]) {
			expect(sanitizers.sanitizeAgentHookInput('cursor', { ...input, ...changed })).toBeNull();
		}
	});
	it('accepts real lifecycle versions and refuses conflicting or malformed identities', async () => {
		const family = await import(resourceUrl('scripts/opencode-family-observation.mjs'));
		for (const provider of ['opencode', 'kilo']) {
			for (const properties of [
				{ info: { id: 'session-1', title: 'PRIVATE_TITLE' } },
				{ sessionID: 'session-1', info: { id: 'session-1' } },
				{ sessionID: 'session-1' },
			]) {
				expect(
					family.sanitizeOpenCodeFamilyEvent(provider, { type: 'session.created', properties }),
				).toMatchObject({ event: 'session-started', sessionKey: 'session-1' });
			}
			for (const properties of [
				{ sessionID: 'one', info: { id: 'two' } },
				{ sessionID: null, info: { id: 'valid' } },
				{ info: { id: 'bad/id' } },
				{ sessionID: 'valid', info: {} },
			]) {
				expect(
					family.sanitizeOpenCodeFamilyEvent(provider, { type: 'session.deleted', properties }),
				).toBeNull();
			}
			expect(
				family.sanitizeOpenCodeFamilyEvent(provider, {
					type: 'session.idle',
					properties: { info: { id: 'session-1' } },
				}),
			).toBeNull();
		}
	});

	it('normalizes Kilo status and callback facts without content or permission decisions', async () => {
		const family = await import(resourceUrl('scripts/opencode-family-observation.mjs'));
		const results = [];
		for (const [status, event] of [
			[{ type: 'busy' }, 'turn-started'],
			[{ type: 'retry' }, 'tool-started'],
			[{ type: 'idle' }, 'session-idle'],
			[
				{ type: 'offline', requestID: 'question-1', message: 'PRIVATE_MESSAGE' },
				'question-requested',
			],
			['streaming', 'turn-started'],
			['idle', 'session-idle'],
		] as const) {
			const value = family.sanitizeOpenCodeFamilyEvent('kilo', {
				type: 'session.status',
				properties: { sessionID: 's', status },
			});
			expect(value).toMatchObject({ event, sessionKey: 's' });
			results.push(value);
		}
		for (const status of ['offline', 'unknown', { type: 'offline', requestID: 'bad/id' }]) {
			expect(
				family.sanitizeOpenCodeFamilyEvent('kilo', {
					type: 'session.status',
					properties: { sessionID: 's', status },
				}),
			).toBeNull();
		}
		expect(
			family.sanitizeOpenCodeFamilyEvent('opencode', {
				type: 'session.status',
				properties: { sessionID: 's', status: 'busy' },
			}),
		).toBeNull();
		const sent: unknown[] = [];
		const plugin = family.createOpenCodeObservation('kilo', {
			send: async (id: string, value: unknown) => {
				sent.push({ id, value });
			},
		});
		const output = {
			parts: ['PRIVATE_PROMPT'],
			args: { command: 'PRIVATE_COMMAND' },
			status: 'ask',
		};
		await plugin['chat.message']({ sessionID: 's' }, output);
		await plugin['tool.execute.before']({ sessionID: 's', callID: 'call-1', tool: 'bash' }, output);
		await plugin['tool.execute.after'](
			{ sessionID: 's', callID: 'call-1', tool: 'bash', args: output.args },
			output,
		);
		await vi.waitFor(() => expect(sent).toHaveLength(3));
		expect(JSON.stringify({ results, sent })).not.toContain('PRIVATE');
		expect(output).toEqual({
			parts: ['PRIVATE_PROMPT'],
			args: { command: 'PRIVATE_COMMAND' },
			status: 'ask',
		});
		await plugin.dispose();
	});

	it('bounds pending delivery and stops at overflow or disposal without reopening a dropped lifecycle', async () => {
		const family = await import(resourceUrl('scripts/opencode-family-observation.mjs'));
		let release: () => void = () => {
			throw new Error('fixture was not initialized');
		};
		const held = new Promise<void>((resolve) => {
			release = resolve;
		});
		const sent: string[] = [];
		const plugin = family.createOpenCodeObservation('kilo', {
			send: async (_id: string, value: { sessionKey: string }) => {
				sent.push(value.sessionKey);
				await held;
			},
		});
		for (let index = 0; index < 65; index++) {
			await plugin.event({
				event: { type: 'session.created', properties: { sessionID: `session-${index}` } },
			});
		}
		expect(sent).toEqual(['session-0']);
		release();
		await held;
		await plugin.event({ event: { type: 'session.idle', properties: { sessionID: 'late' } } });
		expect(sent).toEqual(['session-0']);
		await plugin.dispose();
		const delivered: string[] = [];
		const next = family.createOpenCodeObservation('opencode', {
			send: async (_id: string, value: { sessionKey: string }) => {
				delivered.push(value.sessionKey);
				if (value.sessionKey === 'first') throw new Error('transport failed');
			},
		});
		for (const sessionID of ['first', 'second'])
			await next.event({ event: { type: 'session.created', properties: { sessionID } } });
		await vi.waitFor(() => expect(delivered).toEqual(['first', 'second']));
		await next.dispose();
		await next.event({ event: { type: 'session.deleted', properties: { sessionID: 'second' } } });
		expect(delivered).toEqual(['first', 'second']);
	});
});

describe('Agent observation adapter resources', () => {
	it('emits only an allowlisted Claude Code envelope plus one hint per resolved TTY', async () => {
		const transport = await import(transportUrl);
		const sanitizers = await import(sanitizersUrl);
		const hints: Array<{ readonly hint: unknown; readonly tty: string }> = [];
		const envelopes: unknown[] = [];
		let identity = 0;
		const observation = sanitizers.sanitizeAgentHookInput('claude-code', {
			cwd: '/private/secret/project',
			hook_event_name: 'PreToolUse',
			prompt: 'SUPER_SECRET_PROMPT',
			session_id: 'claude-session-1',
			tool_input: { command: 'cat ~/.ssh/id_ed25519' },
			tool_name: 'Bash',
			tool_output: 'SUPER_SECRET_OUTPUT',
			tool_use_id: 'tool-use-1',
			transcript_path: '/private/secret/transcript.jsonl',
		});

		await transport.forwardSanitizedObservation('claude-code', observation, {
			allocateIdentity: () => `identity_${String(++identity).padStart(16, '0')}`,
			discoverSockets: async () => ['/tmp/orchbee-agent-v1-forwarded.sock'],
			environment: {},
			resolveTargets: async () => [
				{ tty: '/dev/pts/4', visibility: 'hidden' },
				{ tty: '/dev/pts/7', visibility: 'visible' },
			],
			sendEnvelope: async (_socketPath: string, envelope: unknown) => {
				envelopes.push(envelope);
				return true;
			},
			writeHint: async (tty: string, hint: unknown) => {
				hints.push({ hint, tty });
			},
		});

		expect(hints).toEqual([
			{
				hint: {
					correlationId: 'identity_0000000000000002',
					eventId: 'identity_0000000000000001',
					kind: 'correlation',
					version: 1,
				},
				tty: '/dev/pts/4',
			},
			{
				hint: {
					correlationId: 'identity_0000000000000003',
					eventId: 'identity_0000000000000001',
					kind: 'correlation',
					version: 1,
				},
				tty: '/dev/pts/7',
			},
		]);
		expect(envelopes).toEqual([
			{
				adapterId: 'claude-code',
				correlations: [
					{ correlationId: 'identity_0000000000000002', visibility: 'hidden' },
					{ correlationId: 'identity_0000000000000003', visibility: 'visible' },
				],
				eventId: 'identity_0000000000000001',
				observation: {
					activity: 'command',
					agentKey: null,
					event: 'tool-started',
					operationKey: 'tool-use-1',
					sessionKey: 'claude-session-1',
					turnKey: null,
					version: 1,
				},
				version: 1,
			},
		]);
		const serialized = JSON.stringify({ envelopes, hints });
		expect(serialized).not.toContain('SUPER_SECRET');
		expect(serialized).not.toContain('/private/secret');
		expect(serialized).not.toContain('id_ed25519');
	});

	it('forwards only a well-formed run token and both envelope shapes stay admissible', async () => {
		const transport = await import(transportUrl);
		const sanitizers = await import(sanitizersUrl);
		const observation = sanitizers.sanitizeAgentHookInput('claude-code', {
			hook_event_name: 'SessionStart',
			session_id: 'claude-session-1',
		});
		const token = '0123456789abcdef0123456789abcdef';
		const forwarded = async (environment: Record<string, string>) => {
			const envelopes: unknown[] = [];
			let identity = 0;
			await transport.forwardSanitizedObservation('claude-code', observation, {
				allocateIdentity: () => `identity_${String(++identity).padStart(16, '0')}`,
				discoverSockets: async () => ['/tmp/orchbee-agent-v1-forwarded.sock'],
				environment,
				resolveTargets: async () => [{ tty: '/dev/pts/4', visibility: 'unknown' }],
				sendEnvelope: async (_socketPath: string, envelope: unknown) => {
					envelopes.push(envelope);
					return true;
				},
				writeHint: async () => undefined,
			});
			expect(envelopes).toHaveLength(1);
			return envelopes[0];
		};

		const carried = await forwarded({ ORCHBEE_RUN_TOKEN: token });
		expect(carried).toMatchObject({ runToken: token });
		expect(AgentHookRichObservationEnvelopeSchema.safeParse(carried).success).toBe(true);
		for (const malformed of [
			'',
			'0'.repeat(32),
			token.toUpperCase(),
			`${token}\n`,
			'a'.repeat(33),
		]) {
			const omitted = await forwarded({ ORCHBEE_RUN_TOKEN: malformed });
			expect(omitted).not.toHaveProperty('runToken');
			const admitted = AgentHookRichObservationEnvelopeSchema.parse(omitted);
			expect(
				AgentHookRichObservationEnvelopeSchema.safeParse({ ...admitted, runToken: malformed })
					.success,
			).toBe(false);
		}
	});

	it('writes no terminal hint when no Orchbee observation endpoint is available', async () => {
		const transport = await import(transportUrl);
		const resolveTargets = vi.fn<
			() => Promise<Array<{ readonly tty: string; readonly visibility: string }>>
		>(async () => [{ tty: '/dev/ttys001', visibility: 'visible' }]);
		const writeHint = vi.fn<() => void>();

		await transport.forwardSanitizedObservation(
			'codex',
			{
				activity: 'tool',
				agentKey: null,
				event: 'tool-started',
				operationKey: 'codex-tool-1',
				sessionKey: 'codex-session-1',
				turnKey: 'codex-turn-1',
				version: 1,
			},
			{
				discoverSockets: async () => [],
				environment: { TERM_PROGRAM: 'WarpTerminal' },
				resolveTargets,
				writeHint,
			},
		);

		expect(resolveTargets).not.toHaveBeenCalled();
		expect(writeHint).not.toHaveBeenCalled();
	});

	it.runIf(process.platform !== 'win32')(
		'uses only the environment-addressed socket for a local Agent process',
		async () => {
			const transport = await import(transportUrl);
			const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-local-socket-'));
			const socketPath = path.join(directory, 'exact.sock');
			const server = createServer();
			try {
				await new Promise<void>((resolve, reject) => {
					server.once('error', reject);
					server.listen(socketPath, resolve);
				});
				await expect(
					transport.discoverObservationSockets({
						ORCHBEE_AGENT_OBSERVATION_SOCKET: socketPath,
					}),
				).resolves.toEqual([socketPath]);
				await expect(
					transport.discoverObservationSockets({
						ORCHBEE_AGENT_OBSERVATION_SOCKET: path.join(directory, 'missing.sock'),
					}),
				).resolves.toEqual([]);
			} finally {
				await new Promise<void>((resolve) => server.close(() => resolve()));
				await rm(directory, { force: true, recursive: true });
			}
		},
	);

	it.runIf(process.platform !== 'win32')(
		'discovers forwarded candidates only from an explicit remote SSH environment',
		async () => {
			const transport = await import(transportUrl);
			const socketPath = path.join(
				'/tmp',
				`orchbee-agent-v1-test_${process.pid}_${Date.now()}.sock`,
			);
			const server = createServer();
			try {
				await new Promise<void>((resolve, reject) => {
					server.once('error', reject);
					server.listen(socketPath, resolve);
				});
				await expect(
					transport.discoverObservationSockets({ TERM_PROGRAM: 'WarpTerminal' }),
				).resolves.toEqual([]);
				await expect(
					transport.discoverObservationSockets({
						SSH_CONNECTION: '192.0.2.10 50000 192.0.2.20 22',
					}),
				).resolves.toContain(socketPath);
			} finally {
				await new Promise<void>((resolve) => server.close(() => resolve()));
				await rm(socketPath, { force: true });
			}
		},
	);

	it('writes terminal hints once only after at least one endpoint returns the exact v3 ACK', async () => {
		const transport = await import(transportUrl);
		const writeHint = vi.fn<() => void>();
		const sendEnvelope = vi.fn<(socketPath: string) => Promise<boolean>>(async (socketPath) =>
			socketPath.endsWith('accepted.sock'),
		);
		const options = {
			discoverSockets: async () => ['/tmp/rejected.sock', '/tmp/accepted.sock'],
			environment: {},
			resolveTargets: async () => [
				{ tty: '/dev/ttys001', visibility: 'visible' },
				{ tty: '/dev/ttys002', visibility: 'hidden' },
			],
			sendEnvelope,
			writeHint,
		};

		await transport.forwardSanitizedObservation('codex', normalizedObservation(), options);

		expect(sendEnvelope).toHaveBeenCalledTimes(2);
		expect(writeHint).toHaveBeenCalledTimes(2);
		writeHint.mockClear();
		sendEnvelope.mockResolvedValue(false);

		await transport.forwardSanitizedObservation('codex', normalizedObservation(), options);

		expect(writeHint).not.toHaveBeenCalled();
	});

	it('accepts only the exact complete v3 socket acknowledgement', async () => {
		const transport = await import(transportUrl);
		const send = transport.sendObservationEnvelope as (
			socketPath: string,
			envelope: unknown,
		) => Promise<boolean>;

		await expect(
			withAckServer((socket) => socket.end('{"accepted":true,"version":1}\n'), send),
		).resolves.toBe(true);
		await expect(
			withAckServer((socket) => {
				socket.write('{"accepted":');
				socket.end('true,"version":1}\n');
			}, send),
		).resolves.toBe(true);
		await expect(withAckServer((socket) => socket.end('{"accepted":true'), send)).resolves.toBe(
			false,
		);
		await expect(
			withAckServer((socket) => socket.end('{"accepted":true,"version":1}\nextra'), send),
		).resolves.toBe(false);
		await expect(withAckServer((socket) => socket.destroy(), send)).resolves.toBe(false);
		await expect(withAckServer(() => undefined, send)).resolves.toBe(false);
	});

	it.runIf(process.platform !== 'win32')(
		'keeps missing scripts silent and distinguishes unavailable exec runtimes',
		// Secondary guard for the asynchronous reads; the owned bounds are the
		// monotonic fixture deadline and the per-spawn watchdog below.
		{ timeout: 35_000 },
		async () => {
			const startedAt = performance.now();
			const deadline = startedAt + FIXTURE_DEADLINE_MS;
			const hookFiles = [
				'hooks/hooks.json',
				'hooks/codex.json',
				'hooks/grok.json',
				'hooks/copilot.json',
				'gemini/hooks/hooks.json',
				'qwen-code/qwen-extension.json',
			];
			// Private stdin payload via a regular-file fd: a hook that never
			// reads stdin cannot break a parent pipe writer that does not exist.
			const stdinDir = await mkdtemp(path.join(tmpdir(), 'orchbee-hook-stdin-'));
			try {
				const stdinPath = path.join(stdinDir, 'input.json');
				await writeFile(stdinPath, '{"private":"content"}');
				for (const relativePath of hookFiles) {
					const value: unknown = JSON.parse(
						await readFile(path.join(resourceRoot, relativePath), 'utf8'),
					);
					for (const hook of commandHooks(value)) {
						for (const [scenario, environment] of [
							['runtime-absent', { PATH: '/definitely/missing' }],
							['capsule-script-absent', process.env],
						] as const) {
							const label = `${relativePath}:${scenario}`;
							// Freshly opened fd at offset 0 per invocation; owned by this
							// iteration and closed after the synchronous spawn settles.
							const stdinFd = openSync(stdinPath, 'r');
							try {
								const remaining = Math.ceil(deadline - performance.now());
								expect(remaining, `${label}: shared fixture deadline expired`).toBeGreaterThan(0);
								const invocation = guardedInvocation(hook);
								const result = spawnSync(invocation.command, invocation.arguments, {
									env: environment,
									stdio: [stdinFd, 'pipe', 'pipe'],
									// Disposable fixture child; SIGKILL suffices, no cleanup needed.
									killSignal: 'SIGKILL',
									timeout: Math.min(SPAWN_WATCHDOG_MS, remaining),
								});
								const runtimeAbsent = hook.command === 'node' && scenario === 'runtime-absent';
								const shellRuntimeAbsent =
									hook.command.startsWith('node -e ') && scenario === 'runtime-absent';
								const shellStatus = shellRuntimeAbsent ? 127 : 0;
								expect({
									label,
									status: result.status,
									signal: result.signal,
									hasError: result.error !== undefined,
									errorCode: spawnErrorCode(result.error),
									stderrEmpty: (result.stderr?.length ?? 0) === 0,
									stdoutBytes: result.stdout?.length ?? 0,
								}).toEqual({
									label,
									status: runtimeAbsent ? null : shellStatus,
									signal: null,
									hasError: runtimeAbsent,
									errorCode: runtimeAbsent ? 'ENOENT' : null,
									stderrEmpty: !shellRuntimeAbsent,
									stdoutBytes: 0,
								});
							} finally {
								closeSync(stdinFd);
							}
						}
					}
				}
				expect(performance.now() - startedAt).toBeLessThanOrEqual(FIXTURE_DEADLINE_MS);
			} finally {
				await rm(stdinDir, { recursive: true, force: true });
			}
		},
	);

	it('maps Codex and Grok through separate content-free wire contracts', async () => {
		const sanitizers = await import(sanitizersUrl);
		const codex = sanitizers.sanitizeAgentHookInput('codex', {
			cwd: '/secret/codex',
			hook_event_name: 'PermissionRequest',
			prompt: 'PRIVATE',
			session_id: 'codex-session-1',
			tool_input: { command: 'secret command' },
			tool_name: 'Bash',
			turn_id: 'codex-turn-1',
		});
		const grok = sanitizers.sanitizeAgentHookInput('grok', {
			hookEventName: 'notification',
			message: 'PRIVATE',
			notificationType: 'permission_prompt',
			promptId: 'grok-turn-1',
			sessionId: 'grok-session-1',
			toolInput: { command: 'secret command' },
			toolUseId: 'grok-tool-1',
		});

		expect(codex).toEqual({
			activity: null,
			agentKey: null,
			event: 'approval-requested',
			operationKey: 'codex-turn-1',
			sessionKey: 'codex-session-1',
			turnKey: 'codex-turn-1',
			version: 1,
		});
		expect(grok).toEqual({
			activity: null,
			agentKey: null,
			event: 'approval-requested',
			operationKey: 'grok-tool-1',
			sessionKey: 'grok-session-1',
			turnKey: 'grok-turn-1',
			version: 1,
		});
		expect(JSON.stringify({ codex, grok })).not.toContain('PRIVATE');
		expect(JSON.stringify({ codex, grok })).not.toContain('/secret');
		expect(
			sanitizers.sanitizeAgentHookInput('grok', {
				hookEventName: 'stop_cancelled',
				promptId: 'old-turn',
				reason: 'user_interrupt',
				cancelledBy: 'user',
				sessionId: 'grok-session-1',
			}),
		).toMatchObject({ event: 'turn-interrupted', turnKey: 'old-turn' });
		expect(
			sanitizers.sanitizeAgentHookInput('grok', {
				hookEventName: 'stop_failure',
				promptId: 'child-turn',
				sessionId: 'grok-session-1',
				subagentType: 'explore',
			}),
		).toBeNull();
	});

	it('maps Gemini, Copilot, and Qwen without promoting ambiguous upstream events', async () => {
		const sanitizers = await import(sanitizersUrl);
		const gemini = sanitizers.sanitizeAgentHookInput('gemini', {
			hook_event_name: 'BeforeTool',
			session_id: 'gemini-session-1',
			tool_input: { command: 'PRIVATE' },
			tool_name: 'run_shell_command',
		});
		const copilotPolicyCheck = sanitizers.sanitizeAgentHookInput('github-copilot', {
			hook_event_name: 'PermissionRequest',
			permission: 'PRIVATE',
			session_id: 'copilot-session-1',
		});
		const copilotActualPrompt = sanitizers.sanitizeAgentHookInput('github-copilot', {
			hook_event_name: 'Notification',
			message: 'PRIVATE',
			notification_type: 'permission_prompt',
			session_id: 'copilot-session-1',
		});
		const qwenContinuation = sanitizers.sanitizeAgentHookInput('qwen-code', {
			hook_event_name: 'UserPromptSubmit',
			session_id: 'qwen-session-1',
		});
		const qwenUserTurn = sanitizers.sanitizeAgentHookInput('qwen-code', {
			hook_event_name: 'UserPromptSubmit',
			session_id: 'qwen-session-1',
			submitted_prompt: 'PRIVATE',
		});

		expect(gemini).toMatchObject({
			activity: 'command',
			event: 'tool-started',
			operationKey: 'gemini-session-1',
		});
		expect(copilotPolicyCheck).toBeNull();
		expect(copilotActualPrompt).toMatchObject({
			event: 'approval-requested',
			operationKey: 'copilot-session-1',
		});
		expect(qwenContinuation).toBeNull();
		expect(qwenUserTurn).toMatchObject({ event: 'turn-started', sessionKey: 'qwen-session-1' });
		expect(JSON.stringify({ copilotActualPrompt, gemini, qwenUserTurn })).not.toContain('PRIVATE');
	});

	it('projects only Cline lifecycle identities and bounded result status', async () => {
		const cline = await import(clineUrl);
		expect(cline.plugin).toMatchObject({
			manifest: { capabilities: ['hooks'] },
			name: 'orchbee-agent-observation',
		});
		const question = cline.sanitizeClineRuntimeEvent({
			snapshot: { conversationId: 'cline-session-1', runId: 'cline-turn-1' },
			toolCall: {
				arguments: { question: 'PRIVATE' },
				toolCallId: 'cline-tool-1',
				toolName: 'ask_question',
			},
			type: 'tool-started',
		});
		const failed = cline.sanitizeClineRuntimeEvent({
			result: { error: 'PRIVATE', status: 'failed' },
			snapshot: { conversationId: 'cline-session-1', runId: 'cline-turn-1' },
			type: 'run-finished',
		});

		expect(question).toEqual({
			activity: null,
			agentKey: null,
			event: 'question-requested',
			operationKey: 'cline-tool-1',
			sessionKey: 'cline-session-1',
			turnKey: 'cline-turn-1',
			version: 1,
		});
		expect(failed).toMatchObject({ event: 'turn-failed', turnKey: 'cline-turn-1' });
		expect(JSON.stringify({ failed, question })).not.toContain('PRIVATE');
	});

	it('does not reopen a Cline run when completion arrives before delayed activity', async () => {
		const cline = await import(clineUrl);
		const observations: unknown[] = [];
		const deliver = captureObservations(observations);
		const deliverRuntimeEvent = cline.createClineRuntimeEventDelivery();
		const conversationId = 'cline-out-of-order-session';
		const runId = 'cline-out-of-order-run';
		const snapshot = { conversationId, runId };

		deliverRuntimeEvent(clineCompletion(conversationId, runId), deliver);
		deliverRuntimeEvent(clineStart(conversationId, runId), deliver);
		deliverRuntimeEvent(
			{
				snapshot,
				toolCall: { toolCallId: 'cline-late-tool', toolName: 'bash' },
				type: 'tool-started',
			},
			deliver,
		);
		deliverRuntimeEvent(clineCompletion(conversationId, runId), deliver);
		deliverRuntimeEvent(clineStart('cline-other-session', runId), deliver);

		expect(observations).toEqual([
			expect.objectContaining({
				event: 'turn-stopped',
				sessionKey: conversationId,
				turnKey: runId,
			}),
			expect.objectContaining({
				event: 'turn-started',
				sessionKey: 'cline-other-session',
				turnKey: runId,
			}),
		]);
	});

	it('bounds Cline completion tombstones per conversation and across conversations', async () => {
		const cline = await import(clineUrl);
		const completedRunCount = cline.CLINE_COMPLETION_RETENTION.completedRunsPerConversation;
		const conversationCount = cline.CLINE_COMPLETION_RETENTION.conversations;
		const perConversationObservations: unknown[] = [];
		const capturePerConversation = captureObservations(perConversationObservations);
		const deliverPerConversation = cline.createClineRuntimeEventDelivery();
		for (let index = 0; index <= completedRunCount; index += 1) {
			deliverPerConversation(
				clineCompletion('cline-bounded-session', `run-${index}`),
				capturePerConversation,
			);
		}
		const perConversationBaseline = perConversationObservations.length;
		deliverPerConversation(clineStart('cline-bounded-session', 'run-0'), capturePerConversation);
		deliverPerConversation(
			clineStart('cline-bounded-session', `run-${completedRunCount}`),
			capturePerConversation,
		);
		expect(perConversationObservations).toHaveLength(perConversationBaseline + 1);
		expect(perConversationObservations.at(-1)).toMatchObject({
			event: 'turn-started',
			turnKey: 'run-0',
		});

		const crossConversationObservations: unknown[] = [];
		const captureAcrossConversations = captureObservations(crossConversationObservations);
		const deliverAcrossConversations = cline.createClineRuntimeEventDelivery();
		for (let index = 0; index <= conversationCount; index += 1) {
			deliverAcrossConversations(
				clineCompletion(`session-${index}`, 'run'),
				captureAcrossConversations,
			);
		}
		const crossConversationBaseline = crossConversationObservations.length;
		deliverAcrossConversations(clineStart('session-0', 'run'), captureAcrossConversations);
		deliverAcrossConversations(
			clineStart(`session-${conversationCount}`, 'run'),
			captureAcrossConversations,
		);
		expect(crossConversationObservations).toHaveLength(crossConversationBaseline + 1);
		expect(crossConversationObservations.at(-1)).toMatchObject({
			event: 'turn-started',
			sessionKey: 'session-0',
		});
	});

	it('maps OpenCode event objects without retaining questions, permissions, or tool data', async () => {
		const openCode = await import(openCodeUrl);
		expect(
			openCode.sanitizeOpenCodeEvent({
				id: 'event-1',
				properties: {
					id: 'question-1',
					questions: [{ question: 'PRIVATE QUESTION' }],
					sessionID: 'opencode-session-1',
				},
				type: 'question.asked',
			}),
		).toEqual({
			activity: null,
			agentKey: null,
			event: 'question-requested',
			operationKey: 'question-1',
			sessionKey: 'opencode-session-1',
			turnKey: null,
			version: 1,
		});
		expect(
			openCode.sanitizeOpenCodeEvent({
				properties: {
					callID: 'call-1',
					input: { path: '/private/file' },
					sessionID: 'opencode-session-1',
					tool: 'edit',
				},
				type: 'session.next.tool.called',
			}),
		).toMatchObject({ activity: 'editing', event: 'tool-started', operationKey: 'call-1' });
	});

	it('uses Pi settled and UI-prompt events without claiming a successful outcome', async () => {
		const pi = await import(piUrl);
		const observations: Record<string, unknown>[] = [];
		const handlers = extensionHandlers((api) =>
			pi.registerPiAgentObservation(api, (value: Record<string, unknown>) =>
				observations.push(value),
			),
		);
		const context = extensionContext('pi-session-1');

		invoke(handlers, 'session_start', {}, context);
		invoke(handlers, 'agent_start', {}, context);
		invoke(handlers, 'ui_prompt_start', { kind: 'select', title: 'PRIVATE' }, context);
		invoke(handlers, 'ui_prompt_end', { kind: 'select', title: 'PRIVATE' }, context);
		invoke(
			handlers,
			'tool_execution_start',
			{ args: { command: 'PRIVATE' }, toolCallId: 'tool-1', toolName: 'bash' },
			context,
		);
		invoke(
			handlers,
			'tool_execution_end',
			{ isError: false, result: 'PRIVATE', toolCallId: 'tool-1', toolName: 'bash' },
			context,
		);
		invoke(handlers, 'agent_settled', {}, context);

		expect(observations.map(({ event }) => event)).toEqual([
			'session-started',
			'turn-started',
			'question-requested',
			'attention-resolved',
			'tool-started',
			'tool-finished',
			'session-idle',
		]);
		expect(observations).not.toContainEqual(expect.objectContaining({ event: 'turn-stopped' }));
		expect(JSON.stringify(observations)).not.toContain('PRIVATE');
	});

	it('exposes Pi and OMP through the default factories required by their loaders', async () => {
		const piEntry = await import(piEntryUrl);
		const ompEntry = await import(ompEntryUrl);
		const piHandlers = new Map<string, unknown>();
		const ompHandlers = new Map<string, unknown>();

		await piEntry.default({
			on: (event: string, handler: unknown) => piHandlers.set(event, handler),
		});
		await ompEntry.default({
			on: (event: string, handler: unknown) => ompHandlers.set(event, handler),
		});

		expect(piHandlers.has('agent_settled')).toBe(true);
		expect(ompHandlers.has('tool_approval_requested')).toBe(true);
	});

	it('treats OMP approvals, Ask, continuation, and final settlement as distinct facts', async () => {
		const omp = await import(ompUrl);
		const observations: Record<string, unknown>[] = [];
		const handlers = extensionHandlers((api) =>
			omp.registerOmpAgentObservation(api, (value: Record<string, unknown>) =>
				observations.push(value),
			),
		);
		const context = extensionContext('omp-session-1');

		invoke(handlers, 'session_start', {}, context);
		invoke(handlers, 'agent_start', {}, context);
		invoke(
			handlers,
			'tool_approval_requested',
			{ reason: 'PRIVATE', toolCallId: 'tool-1', toolName: 'bash' },
			context,
		);
		invoke(
			handlers,
			'tool_approval_resolved',
			{ approved: true, toolCallId: 'tool-1', toolName: 'bash' },
			context,
		);
		invoke(
			handlers,
			'tool_execution_start',
			{ args: { question: 'PRIVATE' }, toolCallId: 'ask-1', toolName: 'Ask' },
			context,
		);
		invoke(
			handlers,
			'tool_execution_end',
			{ isError: false, result: 'PRIVATE', toolCallId: 'ask-1', toolName: 'Ask' },
			context,
		);
		invoke(handlers, 'agent_end', { willContinue: true }, context);
		invoke(handlers, 'agent_end', { willContinue: false }, context);

		expect(observations.map(({ event }) => event)).toEqual([
			'session-started',
			'turn-started',
			'approval-requested',
			'attention-resolved',
			'question-requested',
			'attention-resolved',
			'session-idle',
		]);
		expect(JSON.stringify(observations)).not.toContain('PRIVATE');
	});

	it('resolves hidden tmux clients and conservative Zellij client visibility', async () => {
		const transport = await import(transportUrl);
		expect(
			transport.parseTmuxClientOutput(
				'/dev/pts/4\twork\t%7\n/dev/pts/8\twork\t%2\n/dev/pts/9\tother\t%7\n/dev/../tmp/victim\twork\t%7\n',
				'work',
				'%7',
			),
		).toEqual([
			{ tty: '/dev/pts/4', visibility: 'unknown' },
			{ tty: '/dev/pts/8', visibility: 'hidden' },
		]);
		expect(
			transport.parseZellijClientOutput(
				'CLIENT_ID ZELLIJ_PANE_ID RUNNING_COMMAND\n1 terminal_0 zsh\n4 terminal_7 agent\n',
			),
		).toEqual([
			{ clientId: '1', paneId: 'terminal_0' },
			{ clientId: '4', paneId: 'terminal_7' },
		]);
		expect(
			transport.parseZellijClientProcesses(
				'501 100 pts/4 zellij a work\n501 101 pts/8 /usr/local/bin/zellij attach work\n502 102 pts/9 zellij a work\n501 103 pts/10 zellij --server work\n501 104 pts/11 zellij attach other --cwd work\n501 105 pts/12 zellij --session=work\n',
				501,
				'work',
			),
		).toEqual(['/dev/pts/4', '/dev/pts/8', '/dev/pts/12']);
	});

	it.runIf(process.platform !== 'win32')(
		'resolves direct, tmux, and Zellij client TTYs without mux passthrough',
		async () => {
			const transport = await import(transportUrl);
			const userId = process.getuid?.() ?? 501;
			await expect(transport.resolveObservationTargets({ SSH_TTY: '/dev/pts/2' })).resolves.toEqual(
				[{ tty: '/dev/pts/2', visibility: 'unknown' }],
			);
			await expect(
				transport.resolveObservationTargets(
					{ TMUX_PANE: '%7' },
					{
						execute: async (_file: string, arguments_: readonly string[]) =>
							arguments_[0] === 'display-message'
								? 'work\n'
								: '/dev/pts/4\twork\t%7\n/dev/pts/8\twork\t%2\n',
					},
				),
			).resolves.toEqual([
				{ tty: '/dev/pts/4', visibility: 'unknown' },
				{ tty: '/dev/pts/8', visibility: 'hidden' },
			]);
			await expect(
				transport.resolveObservationTargets(
					{ ZELLIJ_PANE_ID: 'terminal_7', ZELLIJ_SESSION_NAME: 'work' },
					{
						execute: async (file: string) =>
							file === 'zellij'
								? 'CLIENT_ID ZELLIJ_PANE_ID RUNNING_COMMAND\n1 terminal_0 zsh\n'
								: `${userId} 100 pts/4 zellij attach work\n`,
					},
				),
			).resolves.toEqual([{ tty: '/dev/pts/4', visibility: 'hidden' }]);
		},
	);

	it('isolates resolver failures from every Agent adapter', async () => {
		const transport = await import(transportUrl);
		const sendEnvelope = vi.fn<() => void>();
		await expect(
			transport.forwardSanitizedObservation(
				'codex',
				{
					activity: null,
					agentKey: null,
					event: 'session-started',
					operationKey: null,
					sessionKey: 'codex-session-1',
					turnKey: null,
					version: 1,
				},
				{
					discoverSockets: async () => ['/tmp/unreachable.sock'],
					resolveTargets: async () => {
						throw new Error('mux unavailable');
					},
					sendEnvelope,
				},
			),
		).resolves.toBeUndefined();
		expect(sendEnvelope).not.toHaveBeenCalled();
	});
});

type ExtensionContextFixture = {
	readonly sessionManager: { readonly getSessionId: () => string };
};
type ExtensionHandler = (event: Record<string, unknown>, context: ExtensionContextFixture) => void;

function extensionHandlers(
	register: (api: { readonly on: (event: string, handler: ExtensionHandler) => void }) => void,
): ReadonlyMap<string, ExtensionHandler> {
	const handlers = new Map<string, ExtensionHandler>();
	register({ on: (event, handler) => handlers.set(event, handler) });
	return handlers;
}

function extensionContext(sessionId: string): ExtensionContextFixture {
	return { sessionManager: { getSessionId: () => sessionId } };
}

function invoke(
	handlers: ReadonlyMap<string, ExtensionHandler>,
	event: string,
	value: Record<string, unknown>,
	context: ExtensionContextFixture,
): void {
	const handler = handlers.get(event);
	if (handler === undefined) throw new Error(`Missing ${event} extension handler.`);
	handler(value, context);
}

function resourceUrl(relativePath: string): string {
	return pathToFileURL(path.join(resourceRoot, relativePath)).href;
}

interface CommandHook {
	readonly args?: readonly string[];
	readonly command: string;
}

function commandHooks(value: unknown): CommandHook[] {
	if (Array.isArray(value)) return value.flatMap(commandHooks);
	if (typeof value !== 'object' || value === null) return [];
	const record = value as Record<string, unknown>;
	const executable = typeof record['exec'] === 'string' ? record['exec'] : record['command'];
	const own =
		typeof executable === 'string'
			? [
					{
						...(Array.isArray(record['args']) &&
						record['args'].every((argument) => typeof argument === 'string')
							? { args: record['args'] as string[] }
							: {}),
						command: executable,
					},
				]
			: [];
	return [...own, ...Object.values(record).flatMap(commandHooks)];
}

function guardedInvocation(hook: CommandHook): {
	readonly arguments: readonly string[];
	readonly command: string;
} {
	const replaceRoot = (value: string): string =>
		value.replace(/\$\{[^}]+\}/gu, '/definitely/missing');
	if (hook.args !== undefined) {
		return { arguments: hook.args.map(replaceRoot), command: hook.command };
	}
	return { arguments: ['-c', replaceRoot(hook.command)], command: '/bin/sh' };
}

function spawnErrorCode(error: Error | undefined): string | null {
	return error !== undefined && 'code' in error && typeof error.code === 'string'
		? error.code
		: null;
}

function captureObservations(target: unknown[]): (observation: unknown) => void {
	return (observation) => target.push(observation);
}

function clineCompletion(conversationId: string, runId: string) {
	return {
		result: { status: 'completed' },
		snapshot: { conversationId, runId },
		type: 'run-finished',
	};
}

function clineStart(conversationId: string, runId: string) {
	return { snapshot: { conversationId, runId }, type: 'run-started' };
}

function normalizedObservation(): Record<string, unknown> {
	return {
		activity: null,
		agentKey: null,
		event: 'session-started',
		operationKey: null,
		sessionKey: 'codex-session-1',
		turnKey: null,
		version: 1,
	};
}

async function withAckServer(
	respond: (socket: Socket) => void,
	send: (socketPath: string, envelope: unknown) => Promise<boolean>,
): Promise<boolean> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-ack-'));
	const socketPath =
		process.platform === 'win32'
			? String.raw`\\.\pipe\orchbee-adapter-ack-${randomUUID()}`
			: path.join(directory, 'ack.sock');
	const connections = new Set<Socket>();
	const server = createServer({ allowHalfOpen: true }, (socket) => {
		connections.add(socket);
		socket.once('close', () => connections.delete(socket));
		// Windows named pipes do not deliver Unix-style half-close semantics.
		// This fixture tests ACK admission; answer after receiving its request.
		socket.once('data', () => respond(socket));
	});
	try {
		await new Promise<void>((resolve, reject) => {
			server.once('error', reject);
			server.listen(socketPath, resolve);
		});
		return await send(socketPath, { version: 1 });
	} finally {
		for (const socket of connections) socket.destroy();
		await new Promise<void>((resolve) => server.close(() => resolve()));
		await rm(directory, { force: true, recursive: true });
	}
}
