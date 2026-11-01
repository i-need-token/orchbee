import { parseWorkbenchAction } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import { decodeWorkbenchResponse } from '../../server-connection';
import {
	ServerWorkbenchSession,
	type ServerWorkbenchConnection,
	type ServerWorkbenchSubmission,
} from '../ServerWorkbenchSession';

type ServerSubscriptionHandlers = Parameters<ServerWorkbenchConnection['subscribe']>[2];

const SESSION_ID = '11111111111111111111111111111111';

type EndListener = Parameters<ServerWorkbenchConnection['onEnded']>[0];

interface PendingExchange {
	readonly reject: (error: Error) => void;
	readonly request: Uint8Array;
	readonly resolve: (record: Uint8Array) => void;
}

class FakeConnection implements ServerWorkbenchConnection {
	readonly exchanges: PendingExchange[] = [];
	#ended: EndListener | null = null;
	#handlers: ServerSubscriptionHandlers | null = null;

	exchange(request: Uint8Array): Promise<Uint8Array> {
		return new Promise((resolve, reject) => {
			this.exchanges.push({ reject, request, resolve });
		});
	}

	onEnded(listener: EndListener): () => void {
		this.#ended = listener;
		return () => {
			this.#ended = null;
		};
	}

	subscribe(
		_mode: string,
		_request: Uint8Array,
		handlers: ServerSubscriptionHandlers,
	): { close(): void; resnapshot(): void } {
		this.#handlers = handlers;
		return { close: () => undefined, resnapshot: () => undefined };
	}

	publish(revision: string): void {
		this.#handlers?.onFrame('update', snapshotRecord(revision));
	}

	end(): void {
		const error = new Error('The connection ended.');
		for (const exchange of this.exchanges) exchange.reject(error);
		this.#ended?.({ kind: 'carrier-closed' });
	}
}

function frame(kind: number, value: unknown): Uint8Array {
	const payload = new TextEncoder().encode(JSON.stringify(value));
	const record = new Uint8Array(12 + payload.byteLength);
	const view = new DataView(record.buffer);
	record.set([0x4f, 0x42, 0x57, 0x50]);
	view.setUint16(4, 1);
	view.setUint16(6, kind);
	view.setUint32(8, payload.byteLength);
	record.set(payload, 12);
	return record;
}

function snapshotRecord(revision: string): Uint8Array {
	return frame(3, {
		kind: 'workbench.snapshot.result',
		outcome: {
			kind: 'available',
			snapshot: {
				kind: 'workbench.snapshot',
				navigation: { directories: [], projects: [], workspaces: [] },
				protocolVersion: 1,
				revision,
				sessionId: SESSION_ID,
				terminalContexts: [],
			},
		},
		protocolVersion: 1,
		requestId: '22222222222222222222222222222222',
	});
}

function requestOf(request: Uint8Array): {
	readonly expectedRevision?: unknown;
	requestId: string;
} {
	const value: unknown = JSON.parse(new TextDecoder().decode(request.subarray(12)));
	if (typeof value !== 'object' || value === null || !('requestId' in value)) {
		throw new Error('The request has no identity.');
	}
	const { requestId } = value;
	if (typeof requestId !== 'string') throw new Error('The request has no identity.');
	return { ...value, requestId };
}

function answer(exchange: PendingExchange | undefined, outcome: unknown): void {
	if (exchange === undefined) throw new Error('No request reached the connection.');
	exchange.resolve(
		frame(4, {
			kind: 'workbench.command.result',
			outcome,
			protocolVersion: 1,
			requestId: requestOf(exchange.request).requestId,
		}),
	);
}

const RENAME = parseWorkbenchAction({
	kind: 'project.rename',
	label: 'Renamed',
	projectId: '33333333333333333333333333333333',
});

async function openSession(connection: FakeConnection): Promise<ServerWorkbenchSession> {
	const opening = ServerWorkbenchSession.open(connection);
	connection.publish('5');
	return opening;
}

function settled(
	submission: Promise<ServerWorkbenchSubmission>,
): Promise<ServerWorkbenchSubmission | 'pending'> {
	return Promise.race([
		submission,
		new Promise<'pending'>((resolve) => {
			setTimeout(() => resolve('pending'), 0);
		}),
	]);
}

async function flush(): Promise<void> {
	await new Promise((resolve) => {
		setTimeout(resolve, 0);
	});
}

describe('ServerWorkbenchSession', () => {
	it('settles a commit only after a publication carries its revision', async () => {
		const connection = new FakeConnection();
		const session = await openSession(connection);
		const submission = session.submit(RENAME);
		await flush();
		answer(connection.exchanges[0], { kind: 'renamed', revision: '6' });

		expect(await settled(submission)).toBe('pending');
		expect(session.snapshot.revision).toBe('5');

		connection.publish('6');

		await expect(submission).resolves.toEqual({
			kind: 'settled',
			outcome: { kind: 'renamed', revision: '6' },
		});
		expect(session.snapshot.revision).toBe('6');
	});

	it('decides each queued command from the revision the previous one published', async () => {
		const connection = new FakeConnection();
		const session = await openSession(connection);
		const first = session.submit(RENAME);
		const second = session.submit(RENAME);
		await flush();
		expect(connection.exchanges).toHaveLength(1);

		answer(connection.exchanges[0], { kind: 'renamed', revision: '6' });
		connection.publish('6');
		await first;
		await flush();

		const next = connection.exchanges[1];
		expect(next === undefined ? null : requestOf(next.request).expectedRevision).toBe('6');
		answer(next, { kind: 'rejected', reason: 'revision-mismatch', revision: '7' });
		await expect(second).resolves.toMatchObject({
			kind: 'settled',
			outcome: { reason: 'revision-mismatch' },
		});
	});

	it('reports an unknown outcome and never replays when the connection ends mid-request', async () => {
		const connection = new FakeConnection();
		const session = await openSession(connection);
		const submission = session.submit(RENAME);
		await flush();

		connection.end();

		await expect(submission).resolves.toEqual({ kind: 'outcome-unknown' });
		expect(session.ended).toEqual({ kind: 'connection-ended' });
		await expect(session.submit(RENAME)).resolves.toEqual({ kind: 'not-submitted' });
		expect(connection.exchanges).toHaveLength(1);
	});

	it('refuses a response whose frame kind does not match it', () => {
		expect(() =>
			decodeWorkbenchResponse(
				frame(3, {
					kind: 'workbench.command.result',
					outcome: { kind: 'denied' },
					protocolVersion: 1,
					requestId: '22222222222222222222222222222222',
				}),
			),
		).toThrow('frame kind');
	});
});
