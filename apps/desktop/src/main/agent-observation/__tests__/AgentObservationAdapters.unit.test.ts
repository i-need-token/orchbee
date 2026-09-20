import { spawnSync } from 'node:child_process';
import { closeSync, openSync } from 'node:fs';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createServer, type Socket } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { describe, expect, it, vi } from 'vitest';

const resourceRoot = path.resolve(process.cwd(), 'resources/agent-observation-adapters/bundle');

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
			discoverSockets: async () => ['/tmp/orchbee-agent-v3-forwarded.sock'],
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
					version: 3,
				},
				tty: '/dev/pts/4',
			},
			{
				hint: {
					correlationId: 'identity_0000000000000003',
					eventId: 'identity_0000000000000001',
					kind: 'correlation',
					version: 3,
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
				version: 3,
			},
		]);
		const serialized = JSON.stringify({ envelopes, hints });
		expect(serialized).not.toContain('SUPER_SECRET');
		expect(serialized).not.toContain('/private/secret');
		expect(serialized).not.toContain('id_ed25519');
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

	it('uses only the environment-addressed socket for a local Agent process', async () => {
		const transport = await import(transportUrl);
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-local-socket-'));
		const socketPath = path.join(directory, 'exact.sock');
		const server = createServer();
		await new Promise<void>((resolve, reject) => {
			server.once('error', reject);
			server.listen(socketPath, resolve);
		});
		try {
			await expect(
				transport.discoverObservationSockets({
					ORCHBEE_AGENT_OBSERVATION_SOCKET_V3: socketPath,
				}),
			).resolves.toEqual([socketPath]);
			await expect(
				transport.discoverObservationSockets({
					ORCHBEE_AGENT_OBSERVATION_SOCKET_V3: path.join(directory, 'missing.sock'),
				}),
			).resolves.toEqual([]);
		} finally {
			await new Promise<void>((resolve) => server.close(() => resolve()));
			await rm(directory, { force: true, recursive: true });
		}
	});

	it('discovers forwarded candidates only from an explicit remote SSH environment', async () => {
		const transport = await import(transportUrl);
		const socketPath = path.join('/tmp', `orchbee-agent-v3-test_${process.pid}_${Date.now()}.sock`);
		const server = createServer();
		await new Promise<void>((resolve, reject) => {
			server.once('error', reject);
			server.listen(socketPath, resolve);
		});
		try {
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
	});

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
			withAckServer((socket) => socket.end('{"accepted":true,"version":3}\n'), send),
		).resolves.toBe(true);
		await expect(
			withAckServer((socket) => {
				socket.write('{"accepted":');
				socket.end('true,"version":3}\n');
			}, send),
		).resolves.toBe(true);
		await expect(withAckServer((socket) => socket.end('{"accepted":true'), send)).resolves.toBe(
			false,
		);
		await expect(
			withAckServer((socket) => socket.end('{"accepted":true,"version":3}\nextra'), send),
		).resolves.toBe(false);
		await expect(withAckServer((socket) => socket.destroy(), send)).resolves.toBe(false);
		await expect(withAckServer(() => undefined, send)).resolves.toBe(false);
	});

	it(
		'keeps every persisted command-hook entry silent when the runtime or Capsule script is absent',
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
				'qwen-code/hooks/hooks.json',
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
								expect({
									label,
									status: result.status,
									signal: result.signal,
									hasError: result.error !== undefined,
									errorCode: spawnErrorCode(result.error),
									stderrBytes: result.stderr.length,
									stdoutBytes: result.stdout.length,
								}).toEqual({
									label,
									status: 0,
									signal: null,
									hasError: false,
									errorCode: null,
									stderrBytes: 0,
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

	it('resolves direct, tmux, and Zellij client TTYs without mux passthrough', async () => {
		const transport = await import(transportUrl);
		const userId = process.getuid?.() ?? 501;
		await expect(transport.resolveObservationTargets({ SSH_TTY: '/dev/pts/2' })).resolves.toEqual([
			{ tty: '/dev/pts/2', visibility: 'unknown' },
		]);
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
	});

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
	const own =
		typeof record['command'] === 'string'
			? [
					{
						...(Array.isArray(record['args']) &&
						record['args'].every((argument) => typeof argument === 'string')
							? { args: record['args'] as string[] }
							: {}),
						command: record['command'],
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
	const socketPath = path.join(directory, 'ack.sock');
	const connections = new Set<Socket>();
	const server = createServer({ allowHalfOpen: true }, (socket) => {
		connections.add(socket);
		socket.once('close', () => connections.delete(socket));
		socket.resume();
		socket.once('end', () => respond(socket));
	});
	await new Promise<void>((resolve, reject) => {
		server.once('error', reject);
		server.listen(socketPath, resolve);
	});
	try {
		return await send(socketPath, { version: 3 });
	} finally {
		for (const socket of connections) socket.destroy();
		await new Promise<void>((resolve) => server.close(() => resolve()));
		await rm(directory, { force: true, recursive: true });
	}
}
