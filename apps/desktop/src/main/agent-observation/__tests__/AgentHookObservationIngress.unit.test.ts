import { mkdtemp, lstat, rm } from 'node:fs/promises';
import { createConnection } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { AgentRuntime, type AgentSemanticEvent } from '@orchbee/agent';
import { CreationIdSchema, TerminalApplicationSignalSchema } from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import type {
	AgentHookAdapterId,
	AgentHookRichObservationEnvelope,
} from '../agent-hook-observation-contract';
import { AgentHookObservationIngress } from '../AgentHookObservationIngress';

// Desktop composition supplies no observation socket on Windows. The native
// Windows cohort has its own named-pipe ingress and process witnesses.
describe.runIf(process.platform !== 'win32')('AgentHookObservationIngress POSIX socket', () => {
	let ingress: AgentHookObservationIngress | undefined;
	let runtimeDirectory: string | undefined;

	afterEach(async () => {
		await ingress?.dispose();
		ingress = undefined;
		vi.useRealTimers();
		if (runtimeDirectory !== undefined)
			await rm(runtimeDirectory, { force: true, recursive: true });
		runtimeDirectory = undefined;
	});

	it.each(['session-identified', 'session-started'])(
		'keeps Cursor identity unknown for %s and repeated identities',
		async (event) => {
			const runtime = new AgentRuntime();
			const setup = await startIngress(runtime);
			const presentation = terminalPresentationId('desktop:presentation:cursor');
			const generation = CreationIdSchema.parse('11'.repeat(16));
			setup.ingress.environmentOverlayFor(presentation, generation);
			for (const sequence of [1, 2]) {
				await expect(
					sendEnvelope(setup.socketPath, envelope(sequence, event, [sequence], 'cursor')),
				).resolves.toBe('{"accepted":true,"version":1}\n');
				setup.ingress.observeTerminalApplicationSignal(
					signal(sequence, sequence, generation),
					presentation,
				);
				expect(runtime.getSnapshot().sessions).toHaveLength(1);
				expect(runtime.getSnapshot().sessions[0]).toMatchObject({
					turn: { kind: 'unknown' },
					pendingRequests: [],
					lastOutcome: null,
				});
			}
		},
	);

	it('admits rich state only after an exact socket and terminal correlation join', async () => {
		const runtime = new AgentRuntime();
		const semanticEvents: AgentSemanticEvent[] = [];
		runtime.subscribeSemanticEvents((event) => semanticEvents.push(event));
		const setup = await startIngress(runtime);
		const presentationId = terminalPresentationId('desktop:presentation:claude-test');
		const creationId = CreationIdSchema.parse('11'.repeat(16));
		const overlay = setup.ingress.environmentOverlayFor(presentationId, creationId);
		expect(overlay).toEqual([
			{ name: 'ORCHBEE_AGENT_OBSERVATION_SOCKET', value: setup.socketPath },
		]);
		expect((await lstat(setup.socketPath)).mode & 0o777).toBe(0o600);

		await expect(sendEnvelope(setup.socketPath, envelope(1, 'session-started'))).resolves.toBe(
			'{"accepted":true,"version":1}\n',
		);
		expect(runtime.getSnapshot().sessions).toEqual([]);
		setup.ingress.observeTerminalApplicationSignal(signal(1, 1, creationId), presentationId);
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			observation: { fidelity: 'rich', source: 'hook' },
			presentationAttachments: [{ generation: creationId, presentationId, visibility: 'hidden' }],
			providerId: 'claude-code',
			turn: { kind: 'idle' },
		});

		setup.ingress.observeTerminalApplicationSignal(signal(2, 2, creationId), presentationId);
		await sendEnvelope(setup.socketPath, envelope(2, 'turn-started'));
		setup.ingress.observeTerminalApplicationSignal(signal(3, 3, creationId), presentationId);
		await sendEnvelope(setup.socketPath, {
			...envelope(3, 'question-requested'),
			observation: {
				...observation('question-requested'),
				operationKey: 'tool-use-1',
			},
		});
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			pendingRequests: [{ kind: 'question' }],
			turn: { activity: 'thinking', kind: 'active' },
		});

		setup.ingress.observeTerminalApplicationSignal(signal(4, 4, creationId), presentationId);
		await sendEnvelope(setup.socketPath, {
			...envelope(4, 'tool-started'),
			observation: {
				...observation('tool-started'),
				activity: 'command',
				operationKey: 'tool-use-1',
			},
		});
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			pendingRequests: [],
			turn: { activity: 'command', kind: 'active' },
		});

		setup.ingress.observeTerminalApplicationSignal(signal(5, 5, creationId), presentationId);
		await sendEnvelope(setup.socketPath, envelope(5, 'turn-stopped'));
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			lastOutcome: { kind: 'succeeded' },
			turn: { kind: 'idle' },
		});
		expect(semanticEvents.filter((event) => event.type === 'turn-outcome')).toHaveLength(1);
	});

	it('fans one event out to every matched presentation and retires each generation independently', async () => {
		const runtime = new AgentRuntime();
		const semanticEvents: AgentSemanticEvent[] = [];
		runtime.subscribeSemanticEvents((event) => semanticEvents.push(event));
		const setup = await startIngress(runtime);
		const firstPresentation = terminalPresentationId('desktop:presentation:first');
		const secondPresentation = terminalPresentationId('desktop:presentation:second');
		const firstCreation = CreationIdSchema.parse('22'.repeat(16));
		const secondCreation = CreationIdSchema.parse('33'.repeat(16));
		setup.ingress.environmentOverlayFor(firstPresentation, firstCreation);
		setup.ingress.environmentOverlayFor(secondPresentation, secondCreation);

		await sendEnvelope(
			setup.socketPath,
			envelope(1, 'turn-started', [1, 2], 'claude-code', ['hidden', 'visible']),
		);
		setup.ingress.observeTerminalApplicationSignal(signal(1, 1, firstCreation), firstPresentation);
		setup.ingress.observeTerminalApplicationSignal(
			signal(2, 1, secondCreation),
			secondPresentation,
		);
		expect(runtime.getSnapshot().sessions).toHaveLength(1);
		expect(runtime.getSnapshot().sessions[0]?.presentationAttachments).toEqual([
			expect.objectContaining({ presentationId: firstPresentation, visibility: 'hidden' }),
			expect.objectContaining({ presentationId: secondPresentation, visibility: 'visible' }),
		]);

		setup.ingress.observeTerminalApplicationSignal(signal(3, 2, firstCreation), firstPresentation);
		await sendEnvelope(setup.socketPath, envelope(2, 'turn-failed', [3]));
		await sendEnvelope(setup.socketPath, envelope(2, 'turn-failed', [3]));
		expect(semanticEvents.filter((event) => event.type === 'turn-outcome')).toHaveLength(1);

		setup.ingress.retirePresentation(firstPresentation);
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			observation: { freshness: 'fresh' },
			presentationAttachments: [
				{ navigable: false, presentationId: firstPresentation },
				{ navigable: true, presentationId: secondPresentation },
			],
		});
		setup.ingress.retirePresentation(secondPresentation);
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			observation: { freshness: 'disconnected' },
		});
	});

	it('accepts hints before rich ingress and conservatively combines nested-mux visibility', async () => {
		const runtime = new AgentRuntime();
		const setup = await startIngress(runtime);
		const presentationId = terminalPresentationId('desktop:presentation:nested');
		const creationId = CreationIdSchema.parse('55'.repeat(16));
		setup.ingress.environmentOverlayFor(presentationId, creationId);

		setup.ingress.observeTerminalApplicationSignal(signal(1, 1, creationId), presentationId);
		setup.ingress.observeTerminalApplicationSignal(signal(2, 1, creationId), presentationId);
		await sendEnvelope(
			setup.socketPath,
			envelope(1, 'turn-started', [1, 2], 'claude-code', ['visible', 'hidden']),
		);

		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			presentationAttachments: [{ presentationId, visibility: 'hidden' }],
			turn: { kind: 'active' },
		});
	});

	it('does not apply a partially joined event after its settlement deadline outlives expiry', async () => {
		let now = 0;
		const runtime = new AgentRuntime();
		const setup = await startIngress(runtime, () => now, 25);
		const presentationId = terminalPresentationId('desktop:presentation:expired-settlement');
		const creationId = CreationIdSchema.parse('99'.repeat(16));
		setup.ingress.environmentOverlayFor(presentationId, creationId);

		await sendEnvelope(setup.socketPath, envelope(1, 'turn-started', [1, 2]));
		vi.useFakeTimers();
		setup.ingress.observeTerminalApplicationSignal(signal(1, 1, creationId), presentationId);
		now = 10_000;
		await vi.advanceTimersByTimeAsync(25);

		expect(runtime.getSnapshot().sessions).toEqual([]);
	});

	it('does not retarget a remaining correlation after its presentation generation changes', async () => {
		const runtime = new AgentRuntime();
		const setup = await startIngress(runtime, undefined, 1_000);
		const presentationId = terminalPresentationId('desktop:presentation:generation-fence');
		const firstCreation = CreationIdSchema.parse('aa'.repeat(16));
		const secondCreation = CreationIdSchema.parse('bb'.repeat(16));
		setup.ingress.environmentOverlayFor(presentationId, firstCreation);

		await sendEnvelope(setup.socketPath, envelope(1, 'turn-started', [1, 2]));
		setup.ingress.observeTerminalApplicationSignal(signal(1, 1, firstCreation), presentationId);
		setup.ingress.environmentOverlayFor(presentationId, secondCreation);
		setup.ingress.observeTerminalApplicationSignal(signal(2, 1, secondCreation), presentationId);

		expect(runtime.getSnapshot().sessions).toEqual([]);
	});

	it('expires unmatched halves and rejects malformed or content-bearing envelopes', async () => {
		let now = 0;
		const runtime = new AgentRuntime();
		const setup = await startIngress(runtime, () => now);
		const presentationId = terminalPresentationId('desktop:presentation:expiry');
		const creationId = CreationIdSchema.parse('44'.repeat(16));
		setup.ingress.environmentOverlayFor(presentationId, creationId);

		await expect(
			sendEnvelope(setup.socketPath, {
				...envelope(1, 'session-started'),
				observation: { ...observation('session-started'), prompt: 'forbidden' },
			}),
		).resolves.toBe('');
		setup.ingress.observeTerminalApplicationSignal(signal(1, 1, creationId), presentationId);
		expect(runtime.getSnapshot().sessions).toEqual([]);

		await expect(sendEnvelope(setup.socketPath, envelope(2, 'session-started'))).resolves.toBe(
			'{"accepted":true,"version":1}\n',
		);
		await expect(sendEnvelope(setup.socketPath, envelope(3, 'session-started', [2]))).resolves.toBe(
			'',
		);
		now = 20_000;
		setup.ingress.observeTerminalApplicationSignal(signal(2, 2, creationId), presentationId);
		expect(runtime.getSnapshot().sessions).toEqual([]);
	});

	it('uses registry-granted capabilities and namespaces equal provider identities', async () => {
		const runtime = new AgentRuntime();
		const setup = await startIngress(runtime);
		const presentationId = terminalPresentationId('desktop:presentation:providers');
		const creationId = CreationIdSchema.parse('66'.repeat(16));
		setup.ingress.environmentOverlayFor(presentationId, creationId);

		await sendEnvelope(setup.socketPath, envelope(1, 'session-started'));
		setup.ingress.observeTerminalApplicationSignal(signal(1, 1, creationId), presentationId);
		await sendEnvelope(setup.socketPath, envelope(1, 'session-started', [2], 'codex'));
		setup.ingress.observeTerminalApplicationSignal(signal(2, 1, creationId), presentationId);

		expect(
			runtime
				.getSnapshot()
				.sessions.map((session) => session.providerId)
				.sort(),
		).toEqual(['claude-code', 'codex']);
		expect(
			runtime.getSnapshot().sessions.find((session) => session.providerId === 'codex'),
		).toMatchObject({
			capabilities: expect.arrayContaining(['questions', 'approvals']),
		});

		await sendEnvelope(setup.socketPath, envelope(3, 'question-requested', [3], 'grok'));
		setup.ingress.observeTerminalApplicationSignal(signal(3, 3, creationId), presentationId);
		expect(runtime.getSnapshot().sessions.map((session) => session.providerId)).not.toContain(
			'grok',
		);
	});

	it('fences delayed keyed outcomes from a newer Grok turn', async () => {
		const runtime = new AgentRuntime();
		const semanticEvents: AgentSemanticEvent[] = [];
		runtime.subscribeSemanticEvents((event) => semanticEvents.push(event));
		const setup = await startIngress(runtime);
		const presentationId = terminalPresentationId('desktop:presentation:grok-turn-fence');
		const creationId = CreationIdSchema.parse('77'.repeat(16));
		setup.ingress.environmentOverlayFor(presentationId, creationId);

		await sendEnvelope(setup.socketPath, keyedEnvelope(1, 'turn-started', 'grok-turn-1'));
		setup.ingress.observeTerminalApplicationSignal(signal(1, 1, creationId), presentationId);
		await sendEnvelope(setup.socketPath, keyedEnvelope(2, 'turn-started', 'grok-turn-2'));
		setup.ingress.observeTerminalApplicationSignal(signal(2, 2, creationId), presentationId);

		const activeTurn = runtime.getSnapshot().sessions[0]?.turn;
		expect(activeTurn).toMatchObject({ kind: 'active' });
		expect(semanticEvents.filter((event) => event.type === 'turn-outcome')).toHaveLength(1);

		await sendEnvelope(setup.socketPath, keyedEnvelope(3, 'turn-canceled', 'grok-turn-1'));
		setup.ingress.observeTerminalApplicationSignal(signal(3, 3, creationId), presentationId);

		expect(runtime.getSnapshot().sessions[0]?.turn).toEqual(activeTurn);
		expect(semanticEvents.filter((event) => event.type === 'turn-outcome')).toHaveLength(1);
	});

	it('coalesces unkeyed busy events and settles idle without inventing success', async () => {
		const runtime = new AgentRuntime();
		const semanticEvents: AgentSemanticEvent[] = [];
		runtime.subscribeSemanticEvents((event) => semanticEvents.push(event));
		const setup = await startIngress(runtime);
		const presentationId = terminalPresentationId('desktop:presentation:opencode-unkeyed');
		const creationId = CreationIdSchema.parse('88'.repeat(16));
		setup.ingress.environmentOverlayFor(presentationId, creationId);

		await sendEnvelope(setup.socketPath, envelope(1, 'turn-started', [1], 'opencode'));
		setup.ingress.observeTerminalApplicationSignal(signal(1, 1, creationId), presentationId);
		const firstTurn = runtime.getSnapshot().sessions[0]?.turn;

		await sendEnvelope(setup.socketPath, envelope(2, 'turn-started', [2], 'opencode'));
		setup.ingress.observeTerminalApplicationSignal(signal(2, 2, creationId), presentationId);
		expect(runtime.getSnapshot().sessions[0]?.turn).toEqual(firstTurn);
		expect(semanticEvents.filter((event) => event.type === 'turn-outcome')).toEqual([]);

		await sendEnvelope(setup.socketPath, envelope(3, 'session-idle', [3], 'opencode'));
		setup.ingress.observeTerminalApplicationSignal(signal(3, 3, creationId), presentationId);
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			lastOutcome: null,
			turn: { kind: 'idle' },
		});
		expect(semanticEvents.filter((event) => event.type === 'turn-outcome')).toEqual([]);
	});

	async function startIngress(
		runtime: AgentRuntime,
		now?: () => number,
		joinSettleMilliseconds = 0,
	) {
		let identifier = 0;
		runtimeDirectory = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-ingress-'));
		const socketPath = path.join(runtimeDirectory, 'agent-observation-v1.sock');
		ingress = new AgentHookObservationIngress(runtime, {
			allocateIdentifier: () => `${++identifier}`,
			joinSettleMilliseconds,
			...(now === undefined ? {} : { now }),
			socketPath,
		});
		await ingress.start();
		return { ingress, socketPath };
	}
});

it('leaves observation disabled when composition supplies no endpoint', async () => {
	const runtime = new AgentRuntime();
	const ingress = new AgentHookObservationIngress(runtime, {
		allocateIdentifier: () => 'disabled',
		socketPath: null,
	});
	try {
		await ingress.start();
		await ingress.start();
		const presentation = terminalPresentationId('desktop:disabled');
		const generation = CreationIdSchema.parse('11'.repeat(16));
		expect(ingress.environmentOverlayFor(presentation, generation)).toEqual([]);
		ingress.observeTerminalApplicationSignal(signal(1, 1, generation), presentation);
		expect(runtime.getSnapshot().sessions).toEqual([]);
	} finally {
		await ingress.dispose();
	}
});

function observation(event: string): Record<string, unknown> {
	return {
		activity: event === 'turn-started' ? 'thinking' : null,
		agentKey: null,
		event,
		operationKey: null,
		sessionKey: 'claude-session-1',
		turnKey: null,
		version: 1,
	};
}

function envelope(
	sequence: number,
	event: string,
	correlations: readonly number[] = [sequence],
	adapterId: AgentHookAdapterId = 'claude-code',
	visibilities: readonly ('hidden' | 'unknown' | 'visible')[] = correlations.map(() => 'hidden'),
): AgentHookRichObservationEnvelope {
	return {
		adapterId,
		correlations: correlations.map((value, index) => ({
			correlationId: correlationId(value),
			visibility: visibilities[index] ?? 'unknown',
		})),
		eventId: eventId(sequence),
		observation: observation(event) as AgentHookRichObservationEnvelope['observation'],
		version: 1,
	};
}

function keyedEnvelope(
	sequence: number,
	event: string,
	turnKey: string,
): AgentHookRichObservationEnvelope {
	const value = envelope(sequence, event, [sequence], 'grok');
	return {
		...value,
		observation: {
			...value.observation,
			sessionKey: 'grok-session-1',
			turnKey,
		},
	};
}

function signal(
	sequence: number,
	eventSequence: number,
	creationId: ReturnType<typeof CreationIdSchema.parse>,
) {
	return TerminalApplicationSignalSchema.parse({
		channel: 'orchbee.agent.v1',
		identity: {
			creationId,
			hostEpoch: '00112233445566778899aabbccddeeff',
			terminalId: 'ffeeddccbbaa99887766554433221100',
		},
		payload: JSON.stringify({
			correlationId: correlationId(sequence),
			eventId: eventId(eventSequence),
			kind: 'correlation',
			version: 1,
		}),
		sequence: String(sequence),
	});
}

function eventId(sequence: number): string {
	return `event-${String(sequence).padStart(16, '0')}`;
}

function correlationId(sequence: number): string {
	return `correlation-${String(sequence).padStart(16, '0')}`;
}

async function sendEnvelope(socketPath: string, body: unknown): Promise<string> {
	return new Promise<string>((resolve, reject) => {
		const socket = createConnection({ path: socketPath });
		const chunks: Buffer[] = [];
		socket.once('error', reject);
		socket.on('data', (chunk: Buffer) => chunks.push(chunk));
		socket.once('close', () => resolve(Buffer.concat(chunks).toString('utf8')));
		socket.once('connect', () => socket.end(JSON.stringify(body)));
	});
}
