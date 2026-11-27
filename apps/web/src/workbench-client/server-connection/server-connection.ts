/**
 * One admitted Client connection to a Server, carried as OBWP streams and lanes.
 *
 * The Server runs one connection's requests serially in arrival order, so a stream is not a unit
 * of concurrency: requests sent in order complete in order. A lane only names a resource scope.
 * Closing one releases the viewers, controllers, and subscriptions opened on it and nothing else.
 *
 * A handler or listener that throws fails only its own scope: a subscription handler ends that
 * subscription, which the Server and its owner are told of, and a listener loses only its call.
 * The connection and every other stream keep serving, and the fault is reported with its site.
 *
 * A subscription the Server refuses for `capacity` before its first frame never ran: it opens again
 * under a fresh stream ID after a short pause, a bounded number of times, before its owner hears the
 * close. A request refused that way fails alone at once, since its caller may already have sent
 * others behind it and a reopened request would run after them.
 */
import {
	ContainedClientFault,
	type ContainedClientFaultReporter,
	type ContainedClientFaultSite,
	reportContainedClientFault,
} from '../contained-client-fault';
import { isFilesRecord } from './files-record-codec';
import {
	MAXIMUM_MULTIPLEX_LANES,
	MAXIMUM_MULTIPLEX_STREAMS,
	MULTIPLEX_ROOT_LANE,
	type MultiplexCapabilities,
	type MultiplexCloseReason,
	MultiplexDecoder,
	type MultiplexDisconnectReason,
	type MultiplexFrameKind,
	type MultiplexMessage,
	type MultiplexStreamMode,
	encodeMultiplexMessage,
} from './workbench-multiplex-codec';

/** A duplex byte carrier that has already finished local admission. */
export interface ServerCarrier {
	/** What the carrier's handshake negotiated beyond the base OBWP contract; absent, nothing. */
	readonly capabilities?: MultiplexCapabilities;
	close(): void;
	send(bytes: Uint8Array): void;
	subscribe(receive: (chunk: Uint8Array) => void, closed: () => void): () => void;
}

export type ServerConnectionEnd =
	| { readonly kind: 'carrier-closed' }
	| { readonly kind: 'closed' }
	| { readonly kind: 'disconnected'; readonly reason: MultiplexDisconnectReason }
	| { readonly detail: string; readonly kind: 'protocol-violation' };

/** The connection ended; a request it held has no known outcome. */
export class ServerConnectionEndedError extends Error {
	readonly end: ServerConnectionEnd;

	constructor(end: ServerConnectionEnd) {
		super(`Server connection ended: ${end.kind}.`);
		this.name = 'ServerConnectionEndedError';
		this.end = end;
	}
}

/** The Server refused a stream before executing its request. */
export class ServerStreamRefusedError extends Error {
	readonly reason: MultiplexCloseReason;
	/** The connection keeps serving, and the same request may be sent again once it has room. */
	readonly retryable: boolean;

	constructor(reason: MultiplexCloseReason) {
		super(`Server closed the stream: ${reason}.`);
		this.name = 'ServerStreamRefusedError';
		this.reason = reason;
		this.retryable = reason === 'capacity';
	}
}

/** The Server ran the request and cannot report how it ended; the connection keeps serving. */
class ServerOutcomeUnknownError extends Error {
	constructor() {
		super('Server ran the request and cannot report its outcome.');
		this.name = 'ServerOutcomeUnknownError';
	}
}

/** The deadline passed without a result; the request may still have taken effect. */
class ServerExchangeTimeoutError extends Error {
	constructor() {
		super('Server exchange timed out.');
		this.name = 'ServerExchangeTimeoutError';
	}
}

class ServerConnectionCapacityError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'ServerConnectionCapacityError';
	}
}

interface ServerSubscriptionHandlers {
	onClosed(reason: MultiplexCloseReason): void;
	onFrame(kind: MultiplexFrameKind, record: Uint8Array): void;
	onGap(through: bigint): void;
}

export interface ServerSubscription {
	close(): void;
	resnapshot(): void;
}

/** The Server's limit minus the stream a Client keeps for its progress probe. */
const MAXIMUM_APPLICATION_STREAMS = MAXIMUM_MULTIPLEX_STREAMS - 1;
const DEFAULT_EXCHANGE_DEADLINE_MILLISECONDS = 50_000;
/** Pauses before a subscription refused for capacity opens again; after the last it closes. */
const CAPACITY_PAUSES_MILLISECONDS = [10, 20, 40, 80, 160, 320, 640] as const;

interface RequestStream {
	readonly kind: 'request';
	readonly laneId: bigint;
	readonly reject: (error: Error) => void;
	readonly resolve: (record: Uint8Array) => void;
	sequence: bigint;
	timer: ReturnType<typeof setTimeout> | null;
}

interface SubscriptionStream {
	readonly handlers: ServerSubscriptionHandlers;
	/** The current open's stream ID; reopening after a capacity refusal replaces it. */
	id: bigint;
	readonly kind: 'subscription';
	readonly laneId: bigint;
	readonly mode: Exclude<MultiplexStreamMode, 'request'>;
	pause: ReturnType<typeof setTimeout> | null;
	refusals: number;
	readonly request: Uint8Array;
	sequence: bigint;
}

type ActiveStream = RequestStream | SubscriptionStream;

interface WaitingOpen {
	cancelled: boolean;
	readonly laneId: bigint;
	readonly mode: MultiplexStreamMode;
	readonly request: Uint8Array;
	readonly stream: ActiveStream;
	readonly streamId: bigint;
}

export class ServerConnection {
	readonly #carrier: ServerCarrier;
	readonly #decoder: MultiplexDecoder;
	readonly #endListeners = new Set<(end: ServerConnectionEnd) => void>();
	readonly #lanes = new Set<bigint>([MULTIPLEX_ROOT_LANE]);
	readonly #reportFault: ContainedClientFaultReporter;
	readonly #streams = new Map<bigint, ActiveStream>();
	/** Subscriptions waiting out a capacity pause, under the stream ID the Server refused. */
	readonly #paused = new Map<bigint, SubscriptionStream>();
	readonly #viewerReadyListeners = new Map<bigint, () => void>();
	readonly #waiting: WaitingOpen[] = [];
	#end: ServerConnectionEnd | null = null;
	#nextLaneId = MULTIPLEX_ROOT_LANE + 1n;
	#nextStreamId = 1n;
	#unsubscribe: (() => void) | null = null;
	#viewerHintsSeen = false;

	constructor(
		carrier: ServerCarrier,
		reportFault: ContainedClientFaultReporter = reportContainedClientFault,
	) {
		this.#carrier = carrier;
		this.#decoder = new MultiplexDecoder(carrier.capabilities);
		this.#reportFault = reportFault;
		const unsubscribe = carrier.subscribe(
			(chunk) => this.#receive(chunk),
			() => this.#finish({ kind: 'carrier-closed' }),
		);
		// A carrier that was already closed reports it during subscribe.
		if (this.#end === null) this.#unsubscribe = unsubscribe;
		else unsubscribe();
	}

	get ended(): ServerConnectionEnd | null {
		return this.#end;
	}

	/** Only an observed hint proves this connection's Server sends them. */
	get viewerHintsSeen(): boolean {
		return this.#viewerHintsSeen;
	}

	/**
	 * Calls `listener` when the Server reports that this viewer holds something to Poll. A hint moves
	 * no record, and one that reaches no listener is dropped; that viewer finds its record on a later
	 * Poll.
	 */
	watchViewerReady(viewerId: bigint, listener: () => void): () => void {
		this.#viewerReadyListeners.set(viewerId, listener);
		return () => this.#viewerReadyListeners.delete(viewerId);
	}

	/** Runs one request and resolves with its complete result record. */
	exchange(
		request: Uint8Array,
		options: { readonly laneId?: bigint; readonly timeoutMilliseconds?: number } = {},
	): Promise<Uint8Array> {
		return new Promise((resolve, reject) => {
			const laneId = options.laneId ?? MULTIPLEX_ROOT_LANE;
			const refusal = this.#openRefusal(laneId);
			if (refusal !== null) {
				reject(refusal);
				return;
			}
			const streamId = this.#nextStreamId++;
			const stream: RequestStream = {
				kind: 'request',
				laneId,
				reject,
				resolve,
				sequence: 0n,
				timer: setTimeout(() => {
					this.#abandon(streamId);
					reject(new ServerExchangeTimeoutError());
				}, options.timeoutMilliseconds ?? DEFAULT_EXCHANGE_DEADLINE_MILLISECONDS),
			};
			this.#open({ cancelled: false, laneId, mode: 'request', request, stream, streamId });
		});
	}

	/**
	 * Opens a state, event, or invalidation stream; its snapshot arrives as the first frame. A File
	 * Bar stream that fails receives a `result` frame instead, then its close.
	 */
	subscribe(
		mode: Exclude<MultiplexStreamMode, 'request'>,
		request: Uint8Array,
		handlers: ServerSubscriptionHandlers,
		laneId: bigint = MULTIPLEX_ROOT_LANE,
	): ServerSubscription {
		const refusal = this.#openRefusal(laneId);
		if (refusal !== null) throw refusal;
		const stream: SubscriptionStream = {
			handlers,
			id: this.#nextStreamId++,
			kind: 'subscription',
			laneId,
			mode,
			pause: null,
			refusals: 0,
			request,
			sequence: 0n,
		};
		this.#open({ cancelled: false, laneId, mode, request, stream, streamId: stream.id });
		return {
			close: () => this.#abandon(stream.id),
			resnapshot: () => {
				if (this.#streams.get(stream.id) === stream) {
					this.#send({ kind: 'resnapshot', streamId: stream.id });
				}
			},
		};
	}

	openLane(): bigint {
		if (this.#end !== null) throw new ServerConnectionEndedError(this.#end);
		// The root lane does not count against the explicit lane limit.
		if (this.#lanes.size > MAXIMUM_MULTIPLEX_LANES) {
			throw new ServerConnectionCapacityError('The connection holds its maximum lanes.');
		}
		const laneId = this.#nextLaneId++;
		this.#lanes.add(laneId);
		this.#send({ kind: 'lane-open', laneId });
		return laneId;
	}

	/** Releases the lane's Server resources. A request already sent still settles normally. */
	closeLane(laneId: bigint): void {
		if (laneId === MULTIPLEX_ROOT_LANE || !this.#lanes.delete(laneId)) return;
		for (const open of this.#waiting) {
			if (open.laneId === laneId && !open.cancelled) {
				open.cancelled = true;
				this.#settleUnsent(open);
			}
		}
		for (const [streamId, stream] of this.#streams) {
			if (stream.kind === 'subscription' && stream.laneId === laneId) {
				this.#streams.delete(streamId);
			}
		}
		for (const stream of this.#paused.values()) {
			if (stream.laneId === laneId) this.#unpause(stream);
		}
		if (this.#end === null) this.#send({ kind: 'lane-close', laneId });
		this.#admitWaiting();
	}

	onEnded(listener: (end: ServerConnectionEnd) => void): () => void {
		if (this.#end !== null) {
			listener(this.#end);
			return () => undefined;
		}
		this.#endListeners.add(listener);
		return () => this.#endListeners.delete(listener);
	}

	close(): void {
		this.#finish({ kind: 'closed' });
	}

	#openRefusal(laneId: bigint): Error | null {
		if (this.#end !== null) return new ServerConnectionEndedError(this.#end);
		// Opening a stream on a closed lane would make the Server end the whole connection.
		if (!this.#lanes.has(laneId)) return new ServerStreamRefusedError('invalid-request');
		return null;
	}

	#open(open: WaitingOpen): void {
		if (this.#streams.size >= MAXIMUM_APPLICATION_STREAMS || this.#waiting.length > 0) {
			this.#waiting.push(open);
			return;
		}
		this.#start(open);
	}

	#start(open: WaitingOpen): void {
		this.#streams.set(open.streamId, open.stream);
		this.#send({
			kind: 'stream-open',
			laneId: open.laneId,
			mode: open.mode,
			request: open.request,
			streamId: open.streamId,
		});
	}

	#admitWaiting(): void {
		while (this.#end === null && this.#streams.size < MAXIMUM_APPLICATION_STREAMS) {
			const next = this.#waiting.shift();
			if (next === undefined) return;
			if (!next.cancelled) this.#start(next);
		}
	}

	/** Forgets a stream locally. The Server ignores the close once it has answered a request. */
	#abandon(streamId: bigint): void {
		const paused = this.#paused.get(streamId);
		// The Server already closed the refused stream, so nothing goes on the wire.
		if (paused !== undefined) this.#unpause(paused);
		const waiting = this.#waiting.find((open) => open.streamId === streamId);
		if (waiting !== undefined) waiting.cancelled = true;
		const stream = this.#streams.get(streamId);
		if (stream === undefined) return;
		this.#streams.delete(streamId);
		if (stream.kind === 'request' && stream.timer !== null) clearTimeout(stream.timer);
		if (this.#end === null) {
			this.#send({ kind: 'stream-close', reason: 'cancelled', streamId });
			this.#admitWaiting();
		}
	}

	/** Opens a refused subscription again under a fresh stream ID once its pause has passed. */
	#pause(stream: SubscriptionStream, delay: number): void {
		stream.refusals += 1;
		this.#paused.set(stream.id, stream);
		stream.pause = setTimeout(() => {
			if (this.#paused.get(stream.id) !== stream) return;
			this.#unpause(stream);
			if (this.#end !== null) return;
			stream.id = this.#nextStreamId++;
			this.#open({
				cancelled: false,
				laneId: stream.laneId,
				mode: stream.mode,
				request: stream.request,
				stream,
				streamId: stream.id,
			});
		}, delay);
	}

	#unpause(stream: SubscriptionStream): void {
		this.#paused.delete(stream.id);
		if (stream.pause !== null) clearTimeout(stream.pause);
		stream.pause = null;
	}

	#receive(chunk: Uint8Array): void {
		if (this.#end !== null) return;
		let messages: MultiplexMessage[];
		try {
			messages = this.#decoder.push(chunk);
		} catch (error) {
			this.#violate(error instanceof Error ? error.message : 'Invalid multiplex frame.');
			return;
		}
		for (const message of messages) {
			if (this.#end !== null) return;
			try {
				this.#dispatch(message);
			} catch (error) {
				// A handler the cases below do not contain themselves, such as a closed stream's owner,
				// fails alone; the stream it held is already gone, so only its waiting opens remain.
				this.#contain('message-dispatch', error);
				this.#admitWaiting();
			}
		}
	}

	#contain(site: ContainedClientFaultSite, error: unknown): void {
		this.#reportFault(new ContainedClientFault(site, error));
	}

	/** Runs one subscription handler; a throw ends that subscription alone and tells its owner. */
	#deliver(
		streamId: bigint,
		stream: SubscriptionStream,
		site: ContainedClientFaultSite,
		handle: () => void,
	): void {
		try {
			handle();
		} catch (error) {
			this.#contain(site, error);
			// The owner may have closed the stream before it threw.
			if (this.#streams.get(streamId) !== stream) return;
			this.#abandon(streamId);
			try {
				stream.handlers.onClosed('cancelled');
			} catch (closeError) {
				this.#contain('subscription-closed', closeError);
			}
		}
	}

	#dispatch(message: MultiplexMessage): void {
		switch (message.kind) {
			case 'stream-frame': {
				const stream = this.#streams.get(message.streamId);
				// A frame may cross a local cancellation on the wire.
				if (stream === undefined) return;
				if (message.sequence <= stream.sequence) {
					this.#violate('A stream frame did not advance its sequence.');
					return;
				}
				stream.sequence = message.sequence;
				if (stream.kind === 'request') {
					if (message.frameKind !== 'result') {
						this.#violate('A request stream received a subscription frame.');
						return;
					}
					this.#streams.delete(message.streamId);
					if (stream.timer !== null) clearTimeout(stream.timer);
					stream.resolve(message.record);
					this.#admitWaiting();
					return;
				}
				// A File Bar subscription answers with a Result only to fail, before the Server closes it.
				if (message.frameKind === 'result' && !isFilesRecord(stream.request)) {
					this.#violate('A subscription stream received a request result.');
					return;
				}
				this.#deliver(message.streamId, stream, 'subscription-frame', () =>
					stream.handlers.onFrame(message.frameKind, message.record),
				);
				return;
			}
			case 'stream-close': {
				const stream = this.#streams.get(message.streamId);
				if (stream === undefined) return;
				this.#streams.delete(message.streamId);
				if (stream.kind === 'request') {
					if (stream.timer !== null) clearTimeout(stream.timer);
					stream.reject(
						message.reason === 'outcome-unknown'
							? new ServerOutcomeUnknownError()
							: new ServerStreamRefusedError(message.reason),
					);
				} else {
					const pause = CAPACITY_PAUSES_MILLISECONDS[stream.refusals];
					if (message.reason === 'capacity' && stream.sequence === 0n && pause !== undefined) {
						this.#pause(stream, pause);
					} else {
						stream.handlers.onClosed(message.reason);
					}
				}
				this.#admitWaiting();
				return;
			}
			case 'gap': {
				const stream = this.#streams.get(message.streamId);
				if (stream?.kind === 'subscription') {
					this.#deliver(message.streamId, stream, 'subscription-gap', () =>
						stream.handlers.onGap(message.through),
					);
				}
				return;
			}
			case 'disconnect':
				this.#finish({ kind: 'disconnected', reason: message.reason });
				return;
			case 'viewer-record-ready':
				this.#viewerHintsSeen = true;
				try {
					this.#viewerReadyListeners.get(message.viewerId)?.();
				} catch (error) {
					this.#contain('viewer-ready-listener', error);
				}
				return;
			case 'stream-open':
			case 'resnapshot':
			case 'lane-open':
			case 'lane-close':
				this.#violate('The Server sent a Client-only multiplex message.');
				return;
		}
	}

	#violate(detail: string): void {
		this.#finish({ detail, kind: 'protocol-violation' });
	}

	#send(message: MultiplexMessage): void {
		try {
			this.#carrier.send(encodeMultiplexMessage(message));
		} catch {
			this.#finish({ kind: 'carrier-closed' });
		}
	}

	#settleUnsent(open: WaitingOpen): void {
		if (open.stream.kind !== 'request') return;
		if (open.stream.timer !== null) clearTimeout(open.stream.timer);
		open.stream.reject(new ServerStreamRefusedError('cancelled'));
	}

	#finish(end: ServerConnectionEnd): void {
		if (this.#end !== null) return;
		this.#end = end;
		this.#unsubscribe?.();
		this.#carrier.close();
		for (const stream of this.#paused.values()) this.#unpause(stream);
		const error = new ServerConnectionEndedError(end);
		for (const open of this.#waiting.splice(0)) {
			if (!open.cancelled && open.stream.kind === 'request') {
				if (open.stream.timer !== null) clearTimeout(open.stream.timer);
				open.stream.reject(error);
			}
		}
		const streams = [...this.#streams.values()];
		this.#streams.clear();
		for (const stream of streams) {
			if (stream.kind === 'request') {
				if (stream.timer !== null) clearTimeout(stream.timer);
				stream.reject(error);
			}
		}
		const listeners = [...this.#endListeners];
		this.#endListeners.clear();
		for (const listener of listeners) {
			try {
				listener(end);
			} catch (listenerError) {
				this.#contain('connection-end-listener', listenerError);
			}
		}
	}
}
