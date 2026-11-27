/**
 * OBFB, the records File Bar streams carry under the `files` capability. `schemas/files` owns the
 * contract and the corpus that keeps this codec aligned with `crates/files-protocol`. This layer
 * checks a record's frame where it travels, frames the Client's requests and takes apart what a
 * Server sends; `files-contract.ts` reads the JSON inside.
 */

const MAGIC = [0x4f, 0x42, 0x46, 0x42] as const;
const RECORD_VERSION = 1;
const HEADER_BYTES = 12;
const MAXIMUM_REQUEST_BYTES = 16 * 1024;
const MAXIMUM_REPLY_BYTES = 128 * 1024;
const MAXIMUM_DATA_BYTES = 128 * 1024;
const PUT_PREFIX_BYTES = 17;
const CHUNK_PREFIX_BYTES = 18;
const DIGEST_BYTES = 32;

const REQUEST = 1;
const RESULT = 2;
const NOTICE = 3;
const PUT = 4;
const CHUNK = 5;

const LAST = 1;
const COMMIT = 2;
const CHANGED = 2;
const DIGEST = 4;
const PUT_FLAGS: ReadonlySet<number> = new Set([0, LAST, LAST | COMMIT]);
/** A final chunk's line ending, by its code less one. */
const LINE_ENDINGS = ['none', 'lf', 'crlf'] as const;

/** Where a record travels, which decides the kinds it may be. */
export type FilesRecordPlace = 'events-open' | 'notice-frame' | 'request-open' | 'result-frame';

const PLACED_KINDS: Readonly<Record<FilesRecordPlace, ReadonlySet<number>>> = {
	'events-open': new Set([REQUEST]),
	'notice-frame': new Set([NOTICE]),
	'request-open': new Set([REQUEST, PUT]),
	'result-frame': new Set([RESULT, CHUNK]),
};

/** The facts a read's final chunk reports. */
export interface FilesChunkEnd {
	/** The file's modification time or size moved while it was read. */
	readonly changed: boolean;
	/** For a text read, the SHA-256 of the file bytes it consumed, which a save sends back. */
	readonly digest: Uint8Array | null;
	/** The text's line ending by VS Code's buffer rule; `none` for a byte read. */
	readonly lineEnding: (typeof LINE_ENDINGS)[number];
}

/** One piece of a read's output, at its offset there. */
export interface FilesChunk {
	readonly data: Uint8Array;
	/** Present on the read's final chunk only. */
	readonly end: FilesChunkEnd | null;
	readonly offset: bigint;
	readonly transfer: bigint;
}

/** A record a Server sends: a JSON body, or a piece of a read. */
export type FilesServerRecord =
	| { readonly body: Uint8Array; readonly kind: 'notice' | 'result' }
	| { readonly chunk: FilesChunk; readonly kind: 'chunk' };

/** Whether `record` claims to be OBFB, which only a carrier that negotiated `files` admits. */
export function isFilesRecord(record: Uint8Array): boolean {
	return MAGIC.every((value, index) => record[index] === value);
}

/**
 * Whether `record` is one complete OBFB record that may travel at `place`: a known header, a kind
 * that belongs there, the length its header states within the kind's bound, and a binary prefix in
 * its domain.
 */
export function checkFilesRecord(record: Uint8Array, place: FilesRecordPlace): boolean {
	if (record.byteLength < HEADER_BYTES || !isFilesRecord(record)) return false;
	const view = new DataView(record.buffer, record.byteOffset, record.byteLength);
	const kind = view.getUint16(6);
	const length = view.getUint32(8);
	if (
		view.getUint16(4) !== RECORD_VERSION ||
		!PLACED_KINDS[place].has(kind) ||
		length !== record.byteLength - HEADER_BYTES
	) {
		return false;
	}
	switch (kind) {
		case REQUEST:
			return length >= 1 && length <= MAXIMUM_REQUEST_BYTES;
		case RESULT:
		case NOTICE:
			return length >= 1 && length <= MAXIMUM_REPLY_BYTES;
		case PUT:
			return (
				length >= PUT_PREFIX_BYTES &&
				length <= PUT_PREFIX_BYTES + MAXIMUM_DATA_BYTES &&
				view.getBigUint64(HEADER_BYTES) !== 0n &&
				PUT_FLAGS.has(view.getUint8(HEADER_BYTES + 16))
			);
		default:
			return checkChunk(view, length);
	}
}

/** Frames a request's JSON body as an OBFB Request; null when the body is empty or beyond 16 KiB. */
export function encodeFilesRequestRecord(body: Uint8Array): Uint8Array | null {
	if (body.byteLength < 1 || body.byteLength > MAXIMUM_REQUEST_BYTES) return null;
	const record = new Uint8Array(HEADER_BYTES + body.byteLength);
	const view = new DataView(record.buffer);
	record.set(MAGIC);
	view.setUint16(4, RECORD_VERSION);
	view.setUint16(6, REQUEST);
	view.setUint32(8, body.byteLength);
	record.set(body, HEADER_BYTES);
	return record;
}

/** Takes apart a record a Server sent at `place`; null unless `checkFilesRecord` admits it there. */
export function decodeFilesRecord(
	record: Uint8Array,
	place: 'notice-frame' | 'result-frame',
): FilesServerRecord | null {
	if (!checkFilesRecord(record, place)) return null;
	const view = new DataView(record.buffer, record.byteOffset, record.byteLength);
	const kind = view.getUint16(6);
	if (kind === NOTICE) return { body: record.subarray(HEADER_BYTES), kind: 'notice' };
	if (kind === RESULT) return { body: record.subarray(HEADER_BYTES), kind: 'result' };
	const flags = view.getUint8(HEADER_BYTES + 16);
	let data = HEADER_BYTES + CHUNK_PREFIX_BYTES;
	let end: FilesChunkEnd | null = null;
	if ((flags & LAST) !== 0) {
		const lineEnding = LINE_ENDINGS[view.getUint8(HEADER_BYTES + 17) - 1];
		if (lineEnding === undefined) return null;
		let digest: Uint8Array | null = null;
		if ((flags & DIGEST) !== 0) {
			digest = record.slice(data, data + DIGEST_BYTES);
			data += DIGEST_BYTES;
		}
		end = { changed: (flags & CHANGED) !== 0, digest, lineEnding };
	}
	return {
		chunk: {
			data: record.subarray(data),
			end,
			offset: view.getBigUint64(HEADER_BYTES + 8),
			transfer: view.getBigUint64(HEADER_BYTES),
		},
		kind: 'chunk',
	};
}

function checkChunk(view: DataView, length: number): boolean {
	if (length < CHUNK_PREFIX_BYTES || view.getBigUint64(HEADER_BYTES) === 0n) return false;
	const flags = view.getUint8(HEADER_BYTES + 16);
	const lineEnding = view.getUint8(HEADER_BYTES + 17);
	if ((flags & ~(LAST | CHANGED | DIGEST)) !== 0) return false;
	let data = length - CHUNK_PREFIX_BYTES;
	if ((flags & LAST) === 0) {
		// Only a final chunk reports its read's facts.
		if (flags !== 0 || lineEnding !== 0) return false;
	} else {
		if (lineEnding < 1 || lineEnding > 3) return false;
		if ((flags & DIGEST) !== 0) {
			if (data < DIGEST_BYTES) return false;
			data -= DIGEST_BYTES;
		}
	}
	return data <= MAXIMUM_DATA_BYTES;
}
