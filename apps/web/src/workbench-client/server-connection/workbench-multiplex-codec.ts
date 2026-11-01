/**
 * OBWP version 1 connection-local streams. `crates/workbench-protocol/src/multiplex.rs` is the
 * peer codec and the shared corpus in `schemas/workbench` keeps the two aligned. Inner records
 * keep their own semantic codecs; this layer only frames them.
 */

const MAGIC = [0x4f, 0x42, 0x57, 0x50] as const;
const FRAME_VERSION = 1;
const HEADER_BYTES = 12;

/** Active streams one carrier admits. */
export const MAXIMUM_MULTIPLEX_STREAMS = 64;
/** Explicit lanes one carrier admits, besides the root lane. */
export const MAXIMUM_MULTIPLEX_LANES = 64;
/** One inner record, including its own 12-byte header. */
const MAXIMUM_STREAM_RECORD_BYTES = 24 * 1024 * 1024 + 12;
/** The lane every connection owns without opening it. */
export const MULTIPLEX_ROOT_LANE = 1n;

const STREAM_MODES = ['request', 'state', 'events', 'invalidations'] as const;
const FRAME_KINDS = ['result', 'snapshot', 'update'] as const;
const CLOSE_REASONS = [
	'complete',
	'cancelled',
	'unsupported',
	'capacity',
	'invalid-request',
] as const;
const DISCONNECT_REASONS = ['slow-consumer', 'capacity', 'invalid-stream', 'unavailable'] as const;

export type MultiplexStreamMode = (typeof STREAM_MODES)[number];
export type MultiplexFrameKind = (typeof FRAME_KINDS)[number];
export type MultiplexCloseReason = (typeof CLOSE_REASONS)[number];
export type MultiplexDisconnectReason = (typeof DISCONNECT_REASONS)[number];

export type MultiplexMessage =
	| {
			readonly kind: 'stream-open';
			readonly laneId: bigint;
			readonly mode: MultiplexStreamMode;
			readonly request: Uint8Array;
			readonly streamId: bigint;
	  }
	| {
			readonly frameKind: MultiplexFrameKind;
			readonly kind: 'stream-frame';
			readonly record: Uint8Array;
			readonly sequence: bigint;
			readonly streamId: bigint;
	  }
	| {
			readonly kind: 'stream-close';
			readonly reason: MultiplexCloseReason;
			readonly streamId: bigint;
	  }
	| { readonly kind: 'gap'; readonly streamId: bigint; readonly through: bigint }
	| { readonly kind: 'disconnect'; readonly reason: MultiplexDisconnectReason }
	| { readonly kind: 'resnapshot'; readonly streamId: bigint }
	| { readonly kind: 'lane-open'; readonly laneId: bigint }
	| { readonly kind: 'lane-close'; readonly laneId: bigint };

/** A violated framing rule. The carrier cannot be resynchronized after one. */
export class MultiplexProtocolError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'MultiplexProtocolError';
	}
}

function positive(view: DataView, offset: number): bigint {
	const value = view.getBigUint64(offset);
	if (value === 0n) throw new MultiplexProtocolError('Multiplex identity must be nonzero.');
	return value;
}

function member<const Values extends readonly string[]>(
	values: Values,
	code: number | undefined,
	label: string,
): Values[number] {
	const value = code === undefined ? undefined : values[code];
	if (value === undefined) throw new MultiplexProtocolError(`Unknown multiplex ${label}.`);
	return value;
}

function codeOf(values: readonly string[], value: string): number {
	return values.indexOf(value);
}

function encodedBodyLength(message: MultiplexMessage): number {
	switch (message.kind) {
		case 'stream-open':
			return 17 + checkedRecordLength(message.request);
		case 'stream-frame':
			return 17 + checkedRecordLength(message.record);
		case 'stream-close':
			return 9;
		case 'gap':
			return 16;
		case 'disconnect':
			return 1;
		case 'resnapshot':
		case 'lane-open':
		case 'lane-close':
			return 8;
	}
}

function checkedRecordLength(record: Uint8Array): number {
	if (record.byteLength === 0 || record.byteLength > MAXIMUM_STREAM_RECORD_BYTES) {
		throw new MultiplexProtocolError('Multiplex record length is out of range.');
	}
	return record.byteLength;
}

const MESSAGE_KIND_CODES: Readonly<Record<MultiplexMessage['kind'], number>> = {
	'stream-open': 64,
	'stream-frame': 65,
	'stream-close': 66,
	gap: 67,
	disconnect: 68,
	resnapshot: 69,
	'lane-open': 70,
	'lane-close': 71,
};

export function encodeMultiplexMessage(message: MultiplexMessage): Uint8Array {
	const bodyLength = encodedBodyLength(message);
	const bytes = new Uint8Array(HEADER_BYTES + bodyLength);
	const view = new DataView(bytes.buffer);
	bytes.set(MAGIC);
	view.setUint16(4, FRAME_VERSION);
	view.setUint16(6, MESSAGE_KIND_CODES[message.kind]);
	view.setUint32(8, bodyLength);
	const body = HEADER_BYTES;
	switch (message.kind) {
		case 'stream-open':
			view.setBigUint64(body, message.streamId);
			view.setBigUint64(body + 8, message.laneId);
			view.setUint8(body + 16, codeOf(STREAM_MODES, message.mode));
			bytes.set(message.request, body + 17);
			break;
		case 'stream-frame':
			view.setBigUint64(body, message.streamId);
			view.setBigUint64(body + 8, message.sequence);
			view.setUint8(body + 16, codeOf(FRAME_KINDS, message.frameKind));
			bytes.set(message.record, body + 17);
			break;
		case 'stream-close':
			view.setBigUint64(body, message.streamId);
			view.setUint8(body + 8, codeOf(CLOSE_REASONS, message.reason));
			break;
		case 'gap':
			view.setBigUint64(body, message.streamId);
			view.setBigUint64(body + 8, message.through);
			break;
		case 'disconnect':
			view.setUint8(body, codeOf(DISCONNECT_REASONS, message.reason));
			break;
		case 'resnapshot':
			view.setBigUint64(body, message.streamId);
			break;
		case 'lane-open':
		case 'lane-close':
			view.setBigUint64(body, message.laneId);
			break;
	}
	return bytes;
}

function validBodyLength(kind: number, length: number): boolean {
	switch (kind) {
		case 64:
		case 65:
			return length >= 18 && length <= MAXIMUM_STREAM_RECORD_BYTES + 17;
		case 66:
			return length === 9;
		case 67:
			return length === 16;
		case 68:
			return length === 1;
		case 69:
		case 70:
		case 71:
			return length === 8;
		default:
			return false;
	}
}

function decodeBody(kind: number, body: Uint8Array): MultiplexMessage {
	const view = new DataView(body.buffer, body.byteOffset, body.byteLength);
	switch (kind) {
		case 64:
			return {
				kind: 'stream-open',
				laneId: positive(view, 8),
				mode: member(STREAM_MODES, body[16], 'stream mode'),
				request: body.subarray(17),
				streamId: positive(view, 0),
			};
		case 65:
			return {
				frameKind: member(FRAME_KINDS, body[16], 'frame kind'),
				kind: 'stream-frame',
				record: body.subarray(17),
				sequence: positive(view, 8),
				streamId: positive(view, 0),
			};
		case 66:
			return {
				kind: 'stream-close',
				reason: member(CLOSE_REASONS, body[8], 'close reason'),
				streamId: positive(view, 0),
			};
		case 67:
			return { kind: 'gap', streamId: positive(view, 0), through: positive(view, 8) };
		case 68:
			return {
				kind: 'disconnect',
				reason: member(DISCONNECT_REASONS, body[0], 'disconnect reason'),
			};
		case 69:
			return { kind: 'resnapshot', streamId: positive(view, 0) };
		case 70:
			return { kind: 'lane-open', laneId: positive(view, 0) };
		case 71:
			return { kind: 'lane-close', laneId: positive(view, 0) };
		default:
			throw new MultiplexProtocolError('Unknown multiplex message kind.');
	}
}

/**
 * Reassembles messages from arbitrary carrier chunks. Each message owns one allocation sized from
 * its validated header, so a large record costs one copy and its record bytes can be handed on
 * without another.
 */
export class MultiplexDecoder {
	readonly #header = new Uint8Array(HEADER_BYTES);
	#headerFill = 0;
	#body: Uint8Array | null = null;
	#bodyFill = 0;
	#kind = 0;

	/** Whether a message has started and not finished; the Client bounds how long this may last. */
	get partial(): boolean {
		return this.#headerFill > 0;
	}

	/** Consumes one chunk and returns the messages it completed, in order. */
	push(chunk: Uint8Array): MultiplexMessage[] {
		const messages: MultiplexMessage[] = [];
		let offset = 0;
		while (offset < chunk.byteLength) {
			if (this.#body === null) {
				const copied = Math.min(HEADER_BYTES - this.#headerFill, chunk.byteLength - offset);
				this.#header.set(chunk.subarray(offset, offset + copied), this.#headerFill);
				this.#headerFill += copied;
				offset += copied;
				if (this.#headerFill < HEADER_BYTES) break;
				this.#acceptHeader();
				continue;
			}
			const copied = Math.min(this.#body.byteLength - this.#bodyFill, chunk.byteLength - offset);
			this.#body.set(chunk.subarray(offset, offset + copied), this.#bodyFill);
			this.#bodyFill += copied;
			offset += copied;
			if (this.#bodyFill === this.#body.byteLength) {
				messages.push(decodeBody(this.#kind, this.#body));
				this.#body = null;
				this.#bodyFill = 0;
				this.#headerFill = 0;
			}
		}
		return messages;
	}

	#acceptHeader(): void {
		const view = new DataView(this.#header.buffer);
		if (
			MAGIC.some((value, index) => this.#header[index] !== value) ||
			view.getUint16(4) !== FRAME_VERSION
		) {
			throw new MultiplexProtocolError('Invalid multiplex frame header.');
		}
		const kind = view.getUint16(6);
		const length = view.getUint32(8);
		if (!validBodyLength(kind, length)) {
			throw new MultiplexProtocolError('Invalid multiplex frame length.');
		}
		this.#kind = kind;
		this.#body = new Uint8Array(length);
		this.#bodyFill = 0;
	}
}
