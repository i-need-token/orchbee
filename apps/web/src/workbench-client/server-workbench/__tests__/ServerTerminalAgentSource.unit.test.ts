import {
	parseWorkbenchRequest,
	parseWorkbenchResponse,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { ServerConnection } from '../../server-connection';
import type { ServerWorkbenchEnd } from '../ServerWorkbenchSession';
import { ServerTerminalAgentSource } from '../ServerTerminalAgentSource';

type Handlers = Parameters<ServerConnection['subscribe']>[2];
type AgentState = 'blocked' | 'done' | 'error' | 'idle' | 'opening' | 'unknown' | 'working';

const SESSION_ID = hex(0x1);
const WORKER_ID = hex(0x3);

function hex(value: number): string {
	return value.toString(16).padStart(32, '0');
}

function presentation(terminal: number): string {
	return hex(0x1000 + terminal);
}

function range(start: number, end: number): number[] {
	return Array.from({ length: end - start }, (_, index) => start + index);
}

interface FakeStream {
	closed: boolean;
	readonly handlers: Handlers;
	readonly presentationIds: readonly string[];
	readonly requestId: string;
	readonly sessionId: string;
}

class FakeConnection {
	readonly streams: FakeStream[] = [];

	subscribe(
		mode: string,
		request: Uint8Array,
		handlers: Handlers,
	): { close(): void; resnapshot(): void } {
		const parsed = parseWorkbenchRequest(
			JSON.parse(new TextDecoder().decode(request.subarray(12))),
		);
		if (
			mode !== 'state' ||
			parsed.kind !== 'workbench.panel-agent' ||
			parsed.action.kind !== 'states'
		) {
			throw new Error('Expected a state stream of terminal Agent states.');
		}
		const stream: FakeStream = {
			closed: false,
			handlers,
			presentationIds: parsed.action.presentationIds,
			requestId: parsed.requestId,
			sessionId: parsed.sessionId,
		};
		this.streams.push(stream);
		return {
			close: () => {
				stream.closed = true;
			},
			resnapshot: () => {
				throw new Error('Every state frame is complete, so none is asked for again.');
			},
		};
	}
}

class FakeSession {
	readonly #endListeners = new Set<(end: ServerWorkbenchEnd) => void>();
	readonly #listeners = new Set<(snapshot: WorkbenchSnapshot) => void>();
	#end: ServerWorkbenchEnd | null = null;
	#snapshot: WorkbenchSnapshot;

	constructor(snapshot: WorkbenchSnapshot) {
		this.#snapshot = snapshot;
	}

	get snapshot(): WorkbenchSnapshot {
		return this.#snapshot;
	}

	subscribe(listener: (snapshot: WorkbenchSnapshot) => void): () => void {
		this.#listeners.add(listener);
		return () => {
			this.#listeners.delete(listener);
		};
	}

	onEnded(listener: (end: ServerWorkbenchEnd) => void): () => void {
		if (this.#end !== null) {
			listener(this.#end);
			return () => undefined;
		}
		this.#endListeners.add(listener);
		return () => {
			this.#endListeners.delete(listener);
		};
	}

	publish(snapshot: WorkbenchSnapshot): void {
		this.#snapshot = snapshot;
		for (const listener of this.#listeners) listener(snapshot);
	}

	end(): void {
		const end: ServerWorkbenchEnd = { kind: 'connection-ended' };
		this.#end = end;
		for (const listener of this.#endListeners) listener(end);
	}
}

/** One Workspace whose Tabs each hold one of these terminals, in this order. */
function sessionSnapshot(terminals: readonly number[]): WorkbenchSnapshot {
	const response = parseWorkbenchResponse({
		kind: 'workbench.snapshot.result',
		outcome: {
			kind: 'available',
			snapshot: {
				kind: 'workbench.snapshot',
				navigation: {
					directories: [{ directoryId: hex(0x2), path: 'C:\\work', workerId: WORKER_ID }],
					projects: [],
					workspaces: [
						{
							label: 'Workspace',
							primaryDirectoryId: hex(0x2),
							projectId: null,
							terminalContextId: hex(0x4),
							workspaceId: hex(0x5),
						},
					],
				},
				protocolVersion: 1,
				revision: '1',
				sessionId: SESSION_ID,
				terminalContexts: [
					{
						groups: [],
						tabs: terminals.map((terminal) => ({
							groupId: null,
							label: `Tab ${terminal}`,
							pinned: false,
							root: {
								childPresentations: [],
								kind: 'panel',
								panelId: hex(0x3000 + terminal),
								primaryPresentation: {
									binding: { terminalId: hex(0x4000 + terminal), workerId: WORKER_ID },
									customLabel: null,
									label: `Terminal ${terminal}`,
									presentationId: presentation(terminal),
								},
							},
							tabId: hex(0x2000 + terminal),
						})),
						terminalContextId: hex(0x4),
					},
				],
			},
		},
		protocolVersion: 1,
		requestId: hex(0xf00),
	});
	if (response.kind !== 'workbench.snapshot.result' || response.outcome.kind !== 'available') {
		throw new Error('The fixture is not an available snapshot.');
	}
	return response.outcome.snapshot;
}

function deliver(stream: FakeStream | undefined, outcome: unknown): void {
	if (stream === undefined) throw new Error('No stream reached the Server.');
	const payload = new TextEncoder().encode(
		JSON.stringify({
			kind: 'workbench.panel-agent.result',
			outcome,
			protocolVersion: 1,
			requestId: stream.requestId,
			sessionId: stream.sessionId,
		}),
	);
	const record = new Uint8Array(12 + payload.byteLength);
	const view = new DataView(record.buffer);
	record.set([0x4f, 0x42, 0x57, 0x50]);
	view.setUint16(4, 1);
	view.setUint16(6, 36);
	view.setUint32(8, payload.byteLength);
	record.set(payload, 12);
	stream.handlers.onFrame('update', record);
}

interface Reported {
	readonly completion?: 'none' | 'awaiting-input' | 'acknowledged';
	readonly agent?: string | null;
	readonly sequence?: number;
	readonly state?: AgentState;
}

/** The Server reports these terminals' Agents; it observes none in the others. */
function answer(
	stream: FakeStream | undefined,
	reported: Readonly<Record<string, Reported>> = {},
	order: readonly string[] | undefined = stream?.presentationIds,
): void {
	deliver(stream, {
		kind: 'states',
		presentations: (order ?? []).map((presentationId) => {
			const row = reported[presentationId];
			return {
				presentationId,
				snapshot:
					row === undefined
						? null
						: {
								agent: row.agent === undefined ? 'codex' : row.agent,
								binding: {
									generation: hex(0x6001),
									hostEpoch: hex(0x7001),
									hostTerminalId: hex(0x8001),
									presentationId,
								},
								sequence: `${row.sequence ?? 1}`,
								state: row.state ?? 'idle',
								completion: row.completion ?? 'none',
							},
			};
		}),
		reportRevision: '1',
		workbenchRevision: '1',
	});
}

function states(source: ServerTerminalAgentSource): ReadonlyMap<string, AgentState> {
	return new Map(
		[...source.getSnapshot().presentations].map(([presentationId, { state }]) => [
			presentationId,
			state,
		]),
	);
}

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
});

describe('ServerTerminalAgentSource', () => {
	it('answers more than 64 terminals across streams and starts its baseline once all answered', () => {
		const connection = new FakeConnection();
		const source = new ServerTerminalAgentSource(
			connection,
			new FakeSession(sessionSnapshot(range(0, 70))),
		);
		const notified = vi.fn<() => void>();
		source.subscribe(notified);
		const [first, second] = connection.streams;
		// One stream answers at most 64 terminals, named in topology order.
		expect(connection.streams.map((stream) => stream.presentationIds)).toEqual([
			range(0, 64).map(presentation),
			range(64, 70).map(presentation),
		]);
		answer(first, { [presentation(1)]: { state: 'working' } });
		expect(source.getSnapshot().complete).toBe(false);
		expect(states(source)).toEqual(new Map([[presentation(1), 'working']]));
		// A terminal whose Agent has no name shows none.
		const secondAnswer: Readonly<Record<string, Reported>> = {
			[presentation(64)]: { state: 'blocked' },
			[presentation(65)]: { agent: null },
		};
		answer(second, secondAnswer);
		const baseline = source.getSnapshot();
		expect(baseline.complete).toBe(true);
		expect(states(source)).toEqual(
			new Map([
				[presentation(1), 'working'],
				[presentation(64), 'blocked'],
			]),
		);
		// The same answer again is the same snapshot.
		const notifications = notified.mock.calls.length;
		answer(second, secondAnswer);
		expect(source.getSnapshot()).toBe(baseline);
		expect(notified).toHaveBeenCalledTimes(notifications);
		source.dispose();
	});

	it('reopens only the stream whose terminals changed and keeps the Agents of terminals that stay', () => {
		const connection = new FakeConnection();
		const session = new FakeSession(sessionSnapshot(range(0, 70)));
		const source = new ServerTerminalAgentSource(connection, session);
		const [first, second] = connection.streams;
		answer(first, { [presentation(1)]: { state: 'working' } });
		answer(second, {
			[presentation(64)]: { state: 'blocked' },
			[presentation(65)]: { state: 'done' },
		});
		const kept = source.getSnapshot().presentations.get(presentation(64));
		// Terminal 65 closes and terminal 70 opens; both belong to the second stream.
		const terminals = [...range(0, 65), ...range(66, 71)];
		session.publish(sessionSnapshot(terminals));
		expect(first?.closed).toBe(false);
		expect(second?.closed).toBe(true);
		const reopened = connection.streams[2];
		expect(reopened?.presentationIds).toEqual(terminals.slice(64).map(presentation));
		// Until the new stream answers, terminal 64 keeps its Agent and the closed terminal has none.
		expect(source.getSnapshot().presentations.get(presentation(64))).toBe(kept);
		expect(states(source)).toEqual(
			new Map([
				[presentation(1), 'working'],
				[presentation(64), 'blocked'],
			]),
		);
		expect(source.getSnapshot().complete).toBe(true);
		answer(reopened, {
			[presentation(64)]: { state: 'blocked' },
			[presentation(70)]: { state: 'idle' },
		});
		expect(source.getSnapshot().presentations.get(presentation(64))).toBe(kept);
		expect(states(source)).toEqual(
			new Map([
				[presentation(1), 'working'],
				[presentation(64), 'blocked'],
				[presentation(70), 'idle'],
			]),
		);
		source.dispose();
	});

	it('keeps the last Agents through a failure for ten seconds and retries a capacity close after five', () => {
		const connection = new FakeConnection();
		const source = new ServerTerminalAgentSource(connection, new FakeSession(sessionSnapshot([1])));
		const [stream] = connection.streams;
		answer(stream, { [presentation(1)]: { state: 'working' } });
		// The Server fails every stream while its Worker restarts.
		deliver(stream, { kind: 'failed', reason: 'unavailable' });
		vi.advanceTimersByTime(9999);
		expect(states(source)).toEqual(new Map([[presentation(1), 'working']]));
		vi.advanceTimersByTime(1);
		expect(states(source)).toEqual(new Map());
		expect(source.getSnapshot().complete).toBe(true);
		answer(stream, { [presentation(1)]: { sequence: 2, state: 'done' } });
		expect(states(source)).toEqual(new Map([[presentation(1), 'done']]));
		// A stream closed for capacity opens again as a new request.
		stream?.handlers.onClosed('capacity');
		vi.advanceTimersByTime(4999);
		expect(connection.streams).toHaveLength(1);
		vi.advanceTimersByTime(1);
		const retried = connection.streams[1];
		expect(retried?.requestId).not.toBe(stream?.requestId);
		answer(retried, { [presentation(1)]: { sequence: 2, state: 'done' } });
		expect(states(source)).toEqual(new Map([[presentation(1), 'done']]));
		// Any other close waits for the terminals to change, and its Agents expire meanwhile.
		retried?.handlers.onClosed('invalid-request');
		vi.advanceTimersByTime(60_000);
		expect(connection.streams).toHaveLength(2);
		expect(states(source)).toEqual(new Map());
		source.dispose();
	});

	it('stops observing when a frame breaks the protocol or the session ends', () => {
		const unobserved = { complete: false, presentations: new Map() };
		const connection = new FakeConnection();
		const source = new ServerTerminalAgentSource(
			connection,
			new FakeSession(sessionSnapshot([1, 2])),
		);
		const [stream] = connection.streams;
		answer(stream, { [presentation(1)]: { state: 'working' } });
		const notified = vi.fn<() => void>();
		source.subscribe(notified);
		// The Server answers exactly the terminals it was asked for, in that order.
		answer(stream, {}, [presentation(2), presentation(1)]);
		expect(stream?.closed).toBe(true);
		expect(source.getSnapshot()).toEqual(unobserved);
		expect(notified).toHaveBeenCalledOnce();

		const endingConnection = new FakeConnection();
		const session = new FakeSession(sessionSnapshot([1]));
		const ending = new ServerTerminalAgentSource(endingConnection, session);
		answer(endingConnection.streams[0], { [presentation(1)]: { state: 'working' } });
		session.end();
		expect(endingConnection.streams[0]?.closed).toBe(true);
		expect(ending.getSnapshot()).toEqual(unobserved);
		// A session that has already ended opens no stream.
		const lateConnection = new FakeConnection();
		const late = new ServerTerminalAgentSource(lateConnection, session);
		expect(lateConnection.streams).toEqual([]);
		expect(late.getSnapshot()).toEqual(unobserved);
	});
});

it('publishes completion-only changes without inventing a new provider sequence', () => {
	const connection = new FakeConnection();
	const source = new ServerTerminalAgentSource(
		connection,
		new FakeSession(sessionSnapshot(range(0, 1))),
	);
	try {
		const stream = connection.streams[0];
		answer(stream, { [presentation(0)]: { state: 'idle', completion: 'awaiting-input' } });
		const before = source.getSnapshot().presentations.get(presentation(0));
		expect(before).toMatchObject({ state: 'idle', completion: 'awaiting-input' });
		answer(stream, { [presentation(0)]: { state: 'idle', completion: 'acknowledged' } });
		const after = source.getSnapshot().presentations.get(presentation(0));
		expect(after).toMatchObject({
			state: 'idle',
			completion: 'acknowledged',
			sequence: before?.sequence,
		});
		expect(after).not.toBe(before);
	} finally {
		source.dispose();
	}
});
