import { createHash } from 'node:crypto';

import {
	decodeTerminalProjectionBuffer,
	decodeTerminalRowWindow,
	TerminalFrameKind,
	TerminalIdentitySchema,
	TerminalRowWindowDetail,
	TerminalThemeStateSchema,
	type DecodedTerminalProjectionBuffer,
	type TerminalIdentity,
	type TerminalThemeState,
} from '@orchbee/terminal-protocol';
import {
	TerminalPresentationIdSchema,
	type TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';
import { z } from 'zod';

export const MAX_TERMINAL_ARCHIVE_ROWS = 5_000;
const MAX_TERMINAL_ARCHIVE_BYTES = 10 * 1024 * 1024;
export const MAX_TERMINAL_ARCHIVE_STORE_BYTES = 250 * 1024 * 1024;

const TERMINAL_ARCHIVE_HEADER_BYTES = 12;
const TERMINAL_ARCHIVE_MAGIC = new Uint8Array([0x4f, 0x42, 0x54, 0x41]);
const TERMINAL_ARCHIVE_VERSION = 1;
const textDecoder = new TextDecoder('utf-8', { fatal: true });
const textEncoder = new TextEncoder();

const TerminalArchiveDigestMetadataSchema = z.strictObject({
	chunkLengths: z.array(z.number().int().min(1).max(MAX_TERMINAL_ARCHIVE_BYTES)).max(5_000),
	frameLength: z.number().int().min(1).max(MAX_TERMINAL_ARCHIVE_BYTES),
	identity: TerminalIdentitySchema,
	presentationId: TerminalPresentationIdSchema,
	retainedRows: z.number().int().min(1).max(MAX_TERMINAL_ARCHIVE_ROWS),
	sourceStartRow: z.string().regex(/^(?:0|[1-9][0-9]*)$/u),
	theme: TerminalThemeStateSchema,
	version: z.literal(TERMINAL_ARCHIVE_VERSION),
});
const TerminalArchiveHeaderSchema = TerminalArchiveDigestMetadataSchema.extend({
	contentDigest: z.string().regex(/^[0-9a-f]{64}$/u),
});

type TerminalArchiveDigestMetadata = z.infer<typeof TerminalArchiveDigestMetadataSchema>;

export interface TerminalArchiveWrite {
	readonly frame: Uint8Array;
	readonly identity: TerminalIdentity;
	readonly presentationId: TerminalPresentationId;
	readonly rowWindows: readonly Uint8Array[];
	readonly theme: TerminalThemeState;
}

export interface TerminalArchive {
	readonly encodedByteLength: number;
	readonly frame: DecodedTerminalProjectionBuffer;
	readonly frameBytes: Uint8Array;
	readonly identity: TerminalIdentity;
	readonly presentationId: TerminalPresentationId;
	readonly retainedRows: number;
	readonly rowWindowBytes: readonly Uint8Array[];
	readonly rowWindows: readonly DecodedTerminalProjectionBuffer[];
	readonly sourceStartRow: bigint;
	readonly theme: TerminalThemeState;
}

export type TerminalArchiveCodecResult<Value> =
	| { readonly ok: true; readonly value: Value }
	| { readonly detail: string; readonly ok: false };

export function encodeTerminalArchive(
	value: TerminalArchiveWrite,
): TerminalArchiveCodecResult<Uint8Array> {
	const validated = validateTerminalArchiveParts(value);
	if (!validated.ok) return validated;
	const digestMetadata = TerminalArchiveDigestMetadataSchema.parse({
		chunkLengths: value.rowWindows.map((window) => window.byteLength),
		frameLength: value.frame.byteLength,
		identity: value.identity,
		presentationId: value.presentationId,
		retainedRows: validated.value.retainedRows,
		sourceStartRow: validated.value.sourceStartRow.toString(),
		theme: value.theme,
		version: TERMINAL_ARCHIVE_VERSION,
	});
	const header = TerminalArchiveHeaderSchema.parse({
		...digestMetadata,
		contentDigest: archiveContentDigest(digestMetadata, value.frame, value.rowWindows),
	});
	const headerBytes = textEncoder.encode(JSON.stringify(header));
	const totalBytes =
		TERMINAL_ARCHIVE_HEADER_BYTES +
		headerBytes.byteLength +
		value.frame.byteLength +
		value.rowWindows.reduce((total, window) => total + window.byteLength, 0);
	if (totalBytes > MAX_TERMINAL_ARCHIVE_BYTES) {
		return failure('Terminal archive exceeds its per-presentation byte limit');
	}
	const encoded = new Uint8Array(totalBytes);
	encoded.set(TERMINAL_ARCHIVE_MAGIC, 0);
	const view = new DataView(encoded.buffer);
	view.setUint8(4, TERMINAL_ARCHIVE_VERSION);
	view.setUint32(8, headerBytes.byteLength, true);
	let offset = TERMINAL_ARCHIVE_HEADER_BYTES;
	encoded.set(headerBytes, offset);
	offset += headerBytes.byteLength;
	encoded.set(value.frame, offset);
	offset += value.frame.byteLength;
	for (const window of value.rowWindows) {
		encoded.set(window, offset);
		offset += window.byteLength;
	}
	return { ok: true, value: encoded };
}

export function decodeTerminalArchive(
	encoded: Uint8Array,
): TerminalArchiveCodecResult<TerminalArchive> {
	if (
		encoded.byteLength < TERMINAL_ARCHIVE_HEADER_BYTES ||
		encoded.byteLength > MAX_TERMINAL_ARCHIVE_BYTES ||
		!TERMINAL_ARCHIVE_MAGIC.every((value, index) => encoded[index] === value)
	) {
		return failure('Terminal archive header is invalid');
	}
	const view = new DataView(encoded.buffer, encoded.byteOffset, encoded.byteLength);
	if (
		view.getUint8(4) !== TERMINAL_ARCHIVE_VERSION ||
		view.getUint8(5) !== 0 ||
		view.getUint8(6) !== 0 ||
		view.getUint8(7) !== 0
	) {
		return failure('Terminal archive version or reserved bytes are invalid');
	}
	const headerLength = view.getUint32(8, true);
	if (headerLength === 0 || TERMINAL_ARCHIVE_HEADER_BYTES + headerLength > encoded.byteLength) {
		return failure('Terminal archive header length is invalid');
	}
	let headerValue: unknown;
	try {
		headerValue = JSON.parse(
			textDecoder.decode(
				encoded.subarray(
					TERMINAL_ARCHIVE_HEADER_BYTES,
					TERMINAL_ARCHIVE_HEADER_BYTES + headerLength,
				),
			),
		);
	} catch {
		return failure('Terminal archive header is not canonical UTF-8 JSON');
	}
	const header = TerminalArchiveHeaderSchema.safeParse(headerValue);
	if (!header.success) return failure('Terminal archive header fields are invalid');
	const payloadLength =
		header.data.frameLength + header.data.chunkLengths.reduce((total, length) => total + length, 0);
	if (TERMINAL_ARCHIVE_HEADER_BYTES + headerLength + payloadLength !== encoded.byteLength) {
		return failure('Terminal archive payload length does not match its header');
	}
	let offset = TERMINAL_ARCHIVE_HEADER_BYTES + headerLength;
	const frameBytes = encoded.subarray(offset, offset + header.data.frameLength);
	offset += header.data.frameLength;
	const rowWindowBytes = header.data.chunkLengths.map((length) => {
		const window = encoded.subarray(offset, offset + length);
		offset += length;
		return window;
	});
	const digestMetadata: TerminalArchiveDigestMetadata = {
		chunkLengths: header.data.chunkLengths,
		frameLength: header.data.frameLength,
		identity: header.data.identity,
		presentationId: header.data.presentationId,
		retainedRows: header.data.retainedRows,
		sourceStartRow: header.data.sourceStartRow,
		theme: header.data.theme,
		version: header.data.version,
	};
	if (
		archiveContentDigest(digestMetadata, frameBytes, rowWindowBytes) !== header.data.contentDigest
	) {
		return failure('Terminal archive content digest does not match its header');
	}
	const validated = validateTerminalArchiveParts({
		frame: frameBytes,
		identity: header.data.identity,
		presentationId: header.data.presentationId,
		rowWindows: rowWindowBytes,
		theme: header.data.theme,
	});
	if (!validated.ok) return validated;
	if (
		validated.value.retainedRows !== header.data.retainedRows ||
		validated.value.sourceStartRow !== BigInt(header.data.sourceStartRow)
	) {
		return failure('Terminal archive retained range does not match its header');
	}
	return {
		ok: true,
		value: {
			encodedByteLength: encoded.byteLength,
			frame: validated.value.frame,
			frameBytes,
			identity: header.data.identity,
			presentationId: header.data.presentationId,
			retainedRows: header.data.retainedRows,
			rowWindowBytes,
			rowWindows: validated.value.rowWindows,
			sourceStartRow: validated.value.sourceStartRow,
			theme: header.data.theme,
		},
	};
}

function validateTerminalArchiveParts(value: TerminalArchiveWrite): TerminalArchiveCodecResult<{
	readonly frame: DecodedTerminalProjectionBuffer;
	readonly retainedRows: number;
	readonly rowWindows: readonly DecodedTerminalProjectionBuffer[];
	readonly sourceStartRow: bigint;
}> {
	const presentationId = TerminalPresentationIdSchema.safeParse(value.presentationId);
	const identity = TerminalIdentitySchema.safeParse(value.identity);
	const theme = TerminalThemeStateSchema.safeParse(value.theme);
	if (!presentationId.success || !identity.success || !theme.success) {
		return failure('Terminal archive identity, presentation, or theme is invalid');
	}
	const decodedFrame = decodeTerminalProjectionBuffer(value.frame);
	if (!decodedFrame.ok || decodedFrame.value.kind !== TerminalFrameKind.snapshot) {
		return failure('Terminal archive frame is not a valid packed snapshot');
	}
	const frame = decodedFrame.value;
	if (
		frame.hostEpoch !== identity.data.hostEpoch ||
		frame.themeRevision !== BigInt(theme.data.revision)
	) {
		return failure('Terminal archive frame identity or theme revision is inconsistent');
	}
	const decodedWindows: DecodedTerminalProjectionBuffer[] = [];
	for (const encodedWindow of value.rowWindows) {
		const decoded = decodeTerminalRowWindow(encodedWindow);
		if (!decoded.ok || decoded.value.detail !== TerminalRowWindowDetail.render) {
			return failure('Terminal archive contains an invalid render row window');
		}
		const window = decoded.value.frame;
		if (
			decoded.value.totalRows !== frame.scrollbackRows + BigInt(frame.rows) ||
			window.attachmentId !== frame.attachmentId ||
			window.columns !== frame.columns ||
			window.generation !== frame.generation ||
			window.hostEpoch !== frame.hostEpoch ||
			window.revision !== frame.revision ||
			window.themeRevision !== frame.themeRevision
		) {
			return failure('Terminal archive row-window metadata is inconsistent');
		}
		decodedWindows.push(window);
	}
	let expectedStart = decodedWindows[0]?.scrollbackRows ?? frame.scrollbackRows;
	for (const window of decodedWindows) {
		if (window.scrollbackRows !== expectedStart) {
			return failure('Terminal archive row windows are not contiguous');
		}
		expectedStart += BigInt(window.rows);
	}
	if (expectedStart !== frame.scrollbackRows) {
		return failure('Terminal archive history does not meet the final viewport');
	}
	const sourceStartRow = decodedWindows[0]?.scrollbackRows ?? frame.scrollbackRows;
	const retainedRows = Number(frame.scrollbackRows + BigInt(frame.rows) - sourceStartRow);
	if (retainedRows < 1 || retainedRows > MAX_TERMINAL_ARCHIVE_ROWS) {
		return failure('Terminal archive retained row count is outside its limit');
	}
	return { ok: true, value: { frame, retainedRows, rowWindows: decodedWindows, sourceStartRow } };
}

function failure(detail: string): TerminalArchiveCodecResult<never> {
	return { detail, ok: false };
}

function archiveContentDigest(
	metadata: TerminalArchiveDigestMetadata,
	frame: Uint8Array,
	rowWindows: readonly Uint8Array[],
): string {
	const hash = createHash('sha256');
	hash.update(JSON.stringify(metadata), 'utf8');
	hash.update(frame);
	for (const window of rowWindows) hash.update(window);
	return hash.digest('hex');
}
