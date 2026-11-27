import { describe, expect, it, vi } from 'vitest';

import type { ContainedClientFault } from '../../contained-client-fault';
import { MultiplexDecoder, encodeMultiplexMessage } from '../workbench-multiplex-codec';
import type { MultiplexCapabilities, MultiplexMessage } from '../workbench-multiplex-codec';
import {
	type ServerCarrier,
	ServerConnection,
	ServerConnectionEndedError,
	ServerStreamRefusedError,
} from '../server-connection';

class FakeCarrier implements ServerCarrier {
	readonly capabilities: MultiplexCapabilities;
	readonly sent: MultiplexMessage[] = [];
	readonly #decoder: MultiplexDecoder;
	#closed: (() => void) | null = null;
	#receive: ((chunk: Uint8Array) => void) | null = null;

	constructor(capabilities: MultiplexCapabilities = { files: false, streamOutcomeUnknown: false }) {
		this.capabilities = capabilities;
		this.#decoder = new MultiplexDecoder(capabilities);
	}

	close(): void {
		this.#closed = null;
	}

	send(bytes: Uint8Array): void {
		this.sent.push(...this.#decoder.push(bytes));
	}

	subscribe(receive: (chunk: Uint8Array) => void, closed: () => void): () => void {
		this.#receive = receive;
		this.#closed = closed;
		return () => {
			this.#receive = null;
		};
	}

	deliver(message: MultiplexMessage): void {
		this.#receive?.(encodeMultiplexMessage(message));
	}

	dropConnection(): void {
		this.#closed?.();
	}

	opened(): Extract<MultiplexMessage, { readonly kind: 'stream-open' }>[] {
		return this.sent.filter(
			(message): message is Extract<MultiplexMessage, { readonly kind: 'stream-open' }> =>
				message.kind === 'stream-open',
		);
	}
}

const REQUEST = new Uint8Array(32).fill(7);

function result(streamId: bigint, record: Uint8Array = new Uint8Array([1])): MultiplexMessage {
	return { frameKind: 'result', kind: 'stream-frame', record, sequence: 1n, streamId };
}

const REQUEST_KIND = 1;
const RESULT_KIND = 2;
const FILES_SUBSCRIBE = '{"op":"fs.subscribe","workspaceId":"0123456789abcdef0123456789abcdef"}';

/** An OBFB record of `kind` around a short JSON body. */
function filesRecord(kind: number, json: string): Uint8Array {
	const body = new TextEncoder().encode(json);
	const record = new Uint8Array(12 + body.byteLength);
	record.set([0x4f, 0x42, 0x46, 0x42, 0, 1, 0, kind, 0, 0, 0, body.byteLength]);
	record.set(body, 12);
	return record;
}

describe('ServerConnection', () => {
	it('holds requests past the Server stream limit and opens them in order as streams free up', async () => {
		const carrier = new FakeCarrier();
		const connection = new ServerConnection(carrier);
		const exchanges = Array.from({ length: 65 }, () => connection.exchange(REQUEST));

		const opened = carrier.opened();
		expect(opened).toHaveLength(63);

		carrier.deliver(result(opened[0]?.streamId ?? 0n));
		await expect(exchanges[0]).resolves.toBeInstanceOf(Uint8Array);
		expect(carrier.opened().map((open) => open.streamId)).toEqual(
			Array.from({ length: 64 }, (_, index) => BigInt(index + 1)),
		);

		connection.close();
		const settled = await Promise.allSettled(exchanges);
		expect(settled[64]).toMatchObject({
			reason: expect.any(ServerConnectionEndedError),
			status: 'rejected',
		});
	});

	it('refuses a request on a closed lane locally because the Server would end the connection', async () => {
		const carrier = new FakeCarrier();
		const connection = new ServerConnection(carrier);
		const lane = connection.openLane();
		connection.closeLane(lane);

		await expect(connection.exchange(REQUEST, { laneId: lane })).rejects.toBeInstanceOf(
			ServerStreamRefusedError,
		);
		expect(carrier.opened()).toHaveLength(0);
		expect(connection.ended).toBeNull();
	});

	it('settles a request already sent on a lane that then closes', async () => {
		const carrier = new FakeCarrier();
		const connection = new ServerConnection(carrier);
		const lane = connection.openLane();
		const exchange = connection.exchange(REQUEST, { laneId: lane });
		connection.closeLane(lane);

		carrier.deliver(result(carrier.opened()[0]?.streamId ?? 0n));

		await expect(exchange).resolves.toBeInstanceOf(Uint8Array);
	});

	it('fails one request as unknown, not refused, only on a carrier that admits that close', async () => {
		const carrier = new FakeCarrier({ files: false, streamOutcomeUnknown: true });
		const connection = new ServerConnection(carrier);
		const unknown = connection.exchange(REQUEST);
		carrier.deliver({ kind: 'stream-close', reason: 'outcome-unknown', streamId: 1n });

		const failure: unknown = await unknown.catch((error: unknown) => error);
		expect(failure).toBeInstanceOf(Error);
		expect(failure).not.toBeInstanceOf(ServerStreamRefusedError);
		expect(connection.ended).toBeNull();
		const next = connection.exchange(REQUEST);
		carrier.deliver(result(2n));
		await expect(next).resolves.toBeInstanceOf(Uint8Array);

		const base = new FakeCarrier();
		const baseConnection = new ServerConnection(base);
		const held = baseConnection.exchange(REQUEST);
		base.deliver({ kind: 'stream-close', reason: 'outcome-unknown', streamId: 1n });
		await expect(held).rejects.toBeInstanceOf(ServerConnectionEndedError);
		expect(baseConnection.ended?.kind).toBe('protocol-violation');
	});

	it('reopens a subscription refused for capacity, boundedly, and fails a refused request alone', async () => {
		vi.useFakeTimers();
		try {
			const carrier = new FakeCarrier();
			const connection = new ServerConnection(carrier);
			const closed: string[] = [];
			const frames: string[] = [];
			const handlers = {
				onClosed: (reason: string) => closed.push(reason),
				onFrame: (kind: string) => frames.push(kind),
				onGap: () => undefined,
			};
			const subscription = connection.subscribe('state', REQUEST, handlers);
			const refused = connection.exchange(REQUEST);
			carrier.deliver({ kind: 'stream-close', reason: 'capacity', streamId: 1n });
			carrier.deliver({ kind: 'stream-close', reason: 'capacity', streamId: 2n });

			const failure: unknown = await refused.catch((error: unknown) => error);
			expect(failure).toBeInstanceOf(ServerStreamRefusedError);
			expect(failure).toMatchObject({ reason: 'capacity', retryable: true });
			expect(carrier.opened()).toHaveLength(2);

			vi.runOnlyPendingTimers();
			const reopened = carrier.opened().at(-1);
			expect(reopened).toMatchObject({ mode: 'state', request: REQUEST });
			const streamId = reopened?.streamId ?? 0n;
			expect(streamId).toBeGreaterThan(2n);
			carrier.deliver({
				frameKind: 'snapshot',
				kind: 'stream-frame',
				record: new Uint8Array([1]),
				sequence: 1n,
				streamId,
			});
			subscription.close();
			expect(frames).toEqual(['snapshot']);
			expect(closed).toEqual([]);
			expect(carrier.sent.at(-1)).toEqual({ kind: 'stream-close', reason: 'cancelled', streamId });

			connection.subscribe('state', REQUEST, handlers);
			for (let refusals = 0; closed.length === 0; refusals += 1) {
				expect(refusals).toBeLessThan(32);
				const open = carrier.opened().at(-1);
				carrier.deliver({
					kind: 'stream-close',
					reason: 'capacity',
					streamId: open?.streamId ?? 0n,
				});
				vi.runOnlyPendingTimers();
			}
			expect(closed).toEqual(['capacity']);
			expect(connection.ended).toBeNull();
		} finally {
			vi.useRealTimers();
		}
	});

	it('cancels at its deadline and ignores the result that crosses the cancellation', async () => {
		const carrier = new FakeCarrier();
		const connection = new ServerConnection(carrier);

		await expect(connection.exchange(REQUEST, { timeoutMilliseconds: 1 })).rejects.toMatchObject({
			name: 'ServerExchangeTimeoutError',
		});
		expect(carrier.sent.at(-1)).toEqual({
			kind: 'stream-close',
			reason: 'cancelled',
			streamId: 1n,
		});

		carrier.deliver(result(1n));
		expect(connection.ended).toBeNull();
	});

	it('ends the connection on a frame kind its stream cannot carry and fails what it held', async () => {
		const carrier = new FakeCarrier();
		const connection = new ServerConnection(carrier);
		const exchange = connection.exchange(REQUEST);

		carrier.deliver({
			frameKind: 'snapshot',
			kind: 'stream-frame',
			record: new Uint8Array([1]),
			sequence: 1n,
			streamId: 1n,
		});

		await expect(exchange).rejects.toBeInstanceOf(ServerConnectionEndedError);
		expect(connection.ended?.kind).toBe('protocol-violation');
	});

	it('reports the reason the Server gave for disconnecting', async () => {
		const carrier = new FakeCarrier();
		const connection = new ServerConnection(carrier);
		const ended: unknown[] = [];
		connection.onEnded((end) => ended.push(end));
		const exchange = connection.exchange(REQUEST);

		carrier.deliver({ kind: 'disconnect', reason: 'slow-consumer' });

		await expect(exchange).rejects.toBeInstanceOf(ServerConnectionEndedError);
		expect(ended).toEqual([{ kind: 'disconnected', reason: 'slow-consumer' }]);
	});

	it('delivers subscription frames in order and treats a dropped carrier as the end', () => {
		const carrier = new FakeCarrier();
		const connection = new ServerConnection(carrier);
		const frames: string[] = [];
		connection.subscribe('state', REQUEST, {
			onClosed: () => undefined,
			onFrame: (kind) => frames.push(kind),
			onGap: () => undefined,
		});

		for (const [sequence, frameKind] of [
			[1n, 'snapshot'],
			[2n, 'update'],
		] as const) {
			carrier.deliver({
				frameKind,
				kind: 'stream-frame',
				record: new Uint8Array([1]),
				sequence,
				streamId: 1n,
			});
		}
		carrier.dropConnection();

		expect(frames).toEqual(['snapshot', 'update']);
		expect(connection.ended).toEqual({ kind: 'carrier-closed' });
	});

	it('hands a File Bar subscription its failure result while a result ends any other', () => {
		const carrier = new FakeCarrier({ files: true, streamOutcomeUnknown: false });
		const connection = new ServerConnection(carrier);
		const frames: string[] = [];
		const closes: string[] = [];
		connection.subscribe('events', filesRecord(REQUEST_KIND, FILES_SUBSCRIBE), {
			onClosed: (reason) => closes.push(reason),
			onFrame: (kind) => frames.push(kind),
			onGap: () => undefined,
		});
		connection.subscribe('state', REQUEST, {
			onClosed: () => undefined,
			onFrame: () => undefined,
			onGap: () => undefined,
		});

		carrier.deliver(result(1n, filesRecord(RESULT_KIND, '{"error":{"code":"denied"}}')));
		carrier.deliver({ kind: 'stream-close', reason: 'complete', streamId: 1n });

		expect(frames).toEqual(['result']);
		expect(closes).toEqual(['complete']);
		expect(connection.ended).toBeNull();

		carrier.deliver(result(2n));

		expect(connection.ended?.kind).toBe('protocol-violation');
	});

	it('ends only the subscription whose handler throws and keeps serving everything else', async () => {
		const carrier = new FakeCarrier();
		const faults: ContainedClientFault[] = [];
		const connection = new ServerConnection(carrier, (fault) => faults.push(fault));
		const closed: string[] = [];
		const healthy: string[] = [];
		connection.subscribe('state', REQUEST, {
			onClosed: (reason) => closed.push(reason),
			onFrame: () => {
				throw new Error('handler defect');
			},
			onGap: () => undefined,
		});
		connection.subscribe('state', REQUEST, {
			onClosed: () => undefined,
			onFrame: (kind) => healthy.push(kind),
			onGap: () => undefined,
		});
		connection.watchViewerReady(9n, () => {
			throw new Error('listener defect');
		});
		const exchange = connection.exchange(REQUEST);
		const frame = (streamId: bigint, sequence: bigint): MultiplexMessage => ({
			frameKind: 'snapshot',
			kind: 'stream-frame',
			record: new Uint8Array([1]),
			sequence,
			streamId,
		});

		carrier.deliver(frame(1n, 1n));
		carrier.deliver({ kind: 'viewer-record-ready', viewerId: 9n });
		carrier.deliver(frame(2n, 1n));
		carrier.deliver(frame(1n, 2n));
		carrier.deliver(result(3n));

		await expect(exchange).resolves.toBeInstanceOf(Uint8Array);
		expect(connection.ended).toBeNull();
		expect(healthy).toEqual(['snapshot']);
		expect(closed).toEqual(['cancelled']);
		expect(carrier.sent).toContainEqual({
			kind: 'stream-close',
			reason: 'cancelled',
			streamId: 1n,
		});
		expect(faults.map((fault) => fault.site)).toEqual([
			'subscription-frame',
			'viewer-ready-listener',
		]);
	});
});
