import { mkdtemp, lstat, rm } from 'node:fs/promises';
import { createConnection } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { AgentRuntime, type AgentSemanticEvent } from '@orchbee/agent';
import { CreationIdSchema, TerminalApplicationSignalSchema } from '@orchbee/terminal-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
	AgentHookAdapterId,
	AgentHookRichObservationEnvelope,
} from '../agent-hook-observation-contract';
import { AgentHookObservationIngress } from '../AgentHookObservationIngress';

describe('AgentHookObservationIngress', () => {
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

		await sendEnvelope(setup.socketPath, envelope(1, 'session-started'));
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

		await sendEnvelope(setup.socketPath, envelope(1, 'turn-started', [1, 2]));
		setup.ingress.observeTerminalApplicationSignal(
			signal(1, 1, firstCreation, 'hidden'),
			firstPresentation,
		);
		setup.ingress.observeTerminalApplicationSignal(
			signal(2, 1, secondCreation, 'visible'),
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

		setup.ingress.observeTerminalApplicationSignal(
			signal(1, 1, creationId, 'visible'),
			presentationId,
		);
		setup.ingress.observeTerminalApplicationSignal(
			signal(2, 1, creationId, 'hidden'),
			presentationId,
		);
		await sendEnvelope(setup.socketPath, envelope(1, 'turn-started', [1, 2]));

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

		await sendEnvelope(setup.socketPath, {
			...envelope(1, 'session-started'),
			observation: { ...observation('session-started'), prompt: 'forbidden' },
		});
		setup.ingress.observeTerminalApplicationSignal(signal(1, 1, creationId), presentationId);
		expect(runtime.getSnapshot().sessions).toEqual([]);

		await sendEnvelope(setup.socketPath, envelope(2, 'session-started'));
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
		const socketPath = path.join(runtimeDirectory, 'agent-observation-v2.sock');
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
): AgentHookRichObservationEnvelope {
	return {
		adapterId,
		correlationIds: correlations.map(correlationId),
		eventId: eventId(sequence),
		observation: observation(event) as AgentHookRichObservationEnvelope['observation'],
		version: 2,
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
	visibility: 'hidden' | 'unknown' | 'visible' = 'hidden',
) {
	return TerminalApplicationSignalSchema.parse({
		channel: 'orchbee.agent.v1',
		identity: {
			creationId,
			hostEpoch: '00112233445566778899aabbccddeeff',
			terminalId: 'ffeeddccbbaa99887766554433221100',
		},
		payload: JSON.stringify({
			activity: null,
			correlationId: correlationId(sequence),
			eventId: eventId(eventSequence),
			version: 2,
			visibility,
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

async function sendEnvelope(socketPath: string, body: unknown): Promise<void> {
	await new Promise<void>((resolve, reject) => {
		const socket = createConnection({ path: socketPath });
		socket.once('error', reject);
		socket.once('close', () => resolve());
		socket.once('connect', () => socket.end(JSON.stringify(body)));
	});
}
