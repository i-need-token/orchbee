/**
 * OBFB, the records File Bar streams carry under the `files` capability. `schemas/files` owns the
 * contract and the corpus that keeps this codec aligned with `crates/files-protocol`. This layer
 * checks a record's frame where it travels; the JSON inside belongs to the File Bar adapters.
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

/** Where a record travels, which decides the kinds it may be. */
export type FilesRecordPlace = 'events-open' | 'notice-frame' | 'request-open' | 'result-frame';

const PLACED_KINDS: Readonly<Record<FilesRecordPlace, ReadonlySet<number>>> = {
	'events-open': new Set([REQUEST]),
	'notice-frame': new Set([NOTICE]),
	'request-open': new Set([REQUEST, PUT]),
	'result-frame': new Set([RESULT, CHUNK]),
};

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
