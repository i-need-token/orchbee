import { describe, expect, it } from 'vitest';

import { MultiplexDecoder, encodeMultiplexMessage } from '../workbench-multiplex-codec';
import type { MultiplexMessage } from '../workbench-multiplex-codec';
import {
	type ServerCarrier,
	ServerConnection,
	ServerConnectionEndedError,
	ServerExchangeTimeoutError,
	ServerStreamRefusedError,
} from '../server-connection';

class FakeCarrier implements ServerCarrier {
	readonly sent: MultiplexMessage[] = [];
	readonly #decoder = new MultiplexDecoder();
	#closed: (() => void) | null = null;
	#receive: ((chunk: Uint8Array) => void) | null = null;

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

	it('cancels at its deadline and ignores the result that crosses the cancellation', async () => {
		const carrier = new FakeCarrier();
		const connection = new ServerConnection(carrier);

		await expect(connection.exchange(REQUEST, { timeoutMilliseconds: 1 })).rejects.toBeInstanceOf(
			ServerExchangeTimeoutError,
		);
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
});
