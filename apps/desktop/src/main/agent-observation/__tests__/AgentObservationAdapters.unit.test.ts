import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { describe, expect, it, vi } from 'vitest';

const resourceRoot = path.resolve(process.cwd(), 'resources/agent-observation-adapters/bundle');
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
			discoverSockets: async () => ['/tmp/orchbee-agent-v2-forwarded.sock'],
			environment: {},
			resolveTargets: async () => [
				{ tty: '/dev/pts/4', visibility: 'hidden' },
				{ tty: '/dev/pts/7', visibility: 'visible' },
			],
			sendEnvelope: async (_socketPath: string, envelope: unknown) => {
				envelopes.push(envelope);
			},
			writeHint: async (tty: string, hint: unknown) => {
				hints.push({ hint, tty });
			},
		});

		expect(hints).toEqual([
			{
				hint: {
					activity: 'command',
					correlationId: 'identity_0000000000000002',
					eventId: 'identity_0000000000000001',
					version: 2,
					visibility: 'hidden',
				},
				tty: '/dev/pts/4',
			},
			{
				hint: {
					activity: 'command',
					correlationId: 'identity_0000000000000003',
					eventId: 'identity_0000000000000001',
					version: 2,
					visibility: 'visible',
				},
				tty: '/dev/pts/7',
			},
		]);
		expect(envelopes).toEqual([
			{
				adapterId: 'claude-code',
				correlationIds: ['identity_0000000000000002', 'identity_0000000000000003'],
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
				version: 2,
			},
		]);
		const serialized = JSON.stringify({ envelopes, hints });
		expect(serialized).not.toContain('SUPER_SECRET');
		expect(serialized).not.toContain('/private/secret');
		expect(serialized).not.toContain('id_ed25519');
	});

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
				'/dev/pts/4\twork\t%7\n/dev/pts/8\twork\t%2\n/dev/pts/9\tother\t%7\n',
				'work',
				'%7',
			),
		).toEqual([
			{ tty: '/dev/pts/4', visibility: 'visible' },
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
				'501 100 pts/4 zellij a work\n501 101 pts/8 /usr/local/bin/zellij attach work\n502 102 pts/9 zellij a work\n501 103 ?? zellij --server work\n',
				501,
				'work',
			),
		).toEqual(['/dev/pts/4', '/dev/pts/8']);
	});

	it('resolves direct, tmux, and Zellij client TTYs without mux passthrough', async () => {
		const transport = await import(transportUrl);
		const userId = process.getuid?.() ?? 501;
		await expect(transport.resolveObservationTargets({ SSH_TTY: '/dev/pts/2' })).resolves.toEqual([
			{ tty: '/dev/pts/2', visibility: 'visible' },
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
			{ tty: '/dev/pts/4', visibility: 'visible' },
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
