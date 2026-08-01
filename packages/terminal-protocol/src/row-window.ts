import { z } from 'zod';

import { BinaryReader, BinaryWriter, checkedAdd, checkedMultiply } from './binary';
import {
	AttachmentIdSchema,
	HostEpochSchema,
	IDENTIFIER_BYTE_LENGTH,
	identifierBytes,
	identifierText,
	parseAttachmentId,
	parseHostEpoch,
} from './identifiers';
import type { AttachmentId, HostEpoch } from './identifiers';
import {
	decodeTerminalFrame,
	encodeTerminalFrame,
	RowPromptClass,
	TerminalFrameKind,
	TerminalFrameSchema,
	TerminalRowFlag,
	TerminalRowPromptClassCode,
} from './frame';
import type { DecodedTerminalFrame, RowPromptClass as RowPromptClassValue } from './frame';
import { PROTOCOL_LIMITS } from './limits';
import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';

export const TERMINAL_ROW_WINDOW_HEADER_BYTES = 112;
export const TERMINAL_ROW_WINDOW_MAGIC = new Uint8Array([0x4f, 0x42, 0x52, 0x57]);
export const TERMINAL_ROW_WINDOW_FORMAT = { major: 1, minor: 1 } as const;

export const TerminalTextRowFlag = {
	identityAscii: 0x04,
} as const;

const MAX_UINT32 = 0xffff_ffff;
const MAX_UINT64 = 0xffff_ffff_ffff_ffffn;
const TEXT_ROW_HEADER_BYTES = 12;
const TEXT_MAPPING_BYTES = 12;
const ROW_FLAGS_MASK = TerminalRowFlag.wrap | TerminalRowFlag.wrapContinuation;

export const TerminalRowWindowDetail = {
	render: 'render',
	text: 'text',
} as const;

export type TerminalRowWindowDetail =
	(typeof TerminalRowWindowDetail)[keyof typeof TerminalRowWindowDetail];

const uint32Schema = z.number().int().min(0).max(MAX_UINT32);
const uint64Schema = z.bigint().min(0n).max(MAX_UINT64);
const positiveUint64Schema = uint64Schema.refine((value) => value > 0n, {
	message: 'Value must be greater than zero',
});
const columnSchema = z
	.number()
	.int()
	.min(0)
	.max(PROTOCOL_LIMITS.maxFrameColumns - 1);
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { fatal: true });

export const TerminalTextCellMappingSchema = z.strictObject({
	byteOffset: uint32Schema,
	column: columnSchema,
	utf16Offset: uint32Schema,
	width: z.number().int().min(1).max(2),
});

export const TerminalTextRowSchema = z.strictObject({
	index: uint64Schema,
	mappings: z.array(TerminalTextCellMappingSchema).max(PROTOCOL_LIMITS.maxFrameColumns),
	promptClass: z.enum(RowPromptClass),
	text: z.string(),
	wrap: z.boolean(),
	wrapContinuation: z.boolean(),
});

const rowWindowBaseShape = {
	attachmentId: AttachmentIdSchema,
	columns: z.number().int().min(1).max(PROTOCOL_LIMITS.maxFrameColumns),
	generation: positiveUint64Schema,
	hostEpoch: HostEpochSchema,
	revision: uint64Schema,
	sequence: positiveUint64Schema,
	startRow: uint64Schema,
	totalRows: uint64Schema,
} as const;

export const TerminalRenderRowWindowSchema = z
	.strictObject({
		...rowWindowBaseShape,
		detail: z.literal(TerminalRowWindowDetail.render),
		frame: TerminalFrameSchema,
	})
	.superRefine((window, context) => {
		if (
			window.frame.kind !== TerminalFrameKind.snapshot ||
			window.frame.attachmentId !== window.attachmentId ||
			window.frame.hostEpoch !== window.hostEpoch ||
			window.frame.sequence !== window.sequence ||
			window.frame.revision !== window.revision ||
			window.frame.generation !== window.generation ||
			window.frame.columns !== window.columns
		) {
			context.addIssue({
				code: 'custom',
				message: 'Render row-window metadata must match its nested snapshot frame',
				path: ['frame'],
			});
		}
		if (window.startRow + BigInt(window.frame.rows) > window.totalRows) {
			context.addIssue({
				code: 'custom',
				message: 'Render row window exceeds the retained row range',
				path: ['startRow'],
			});
		}
	});

export const TerminalTextRowWindowSchema = z
	.strictObject({
		...rowWindowBaseShape,
		detail: z.literal(TerminalRowWindowDetail.text),
		rows: z.array(TerminalTextRowSchema).min(1).max(PROTOCOL_LIMITS.maxFrameRows),
	})
	.superRefine((window, context) => {
		if (window.startRow + BigInt(window.rows.length) > window.totalRows) {
			context.addIssue({
				code: 'custom',
				message: 'Text row window exceeds the retained row range',
				path: ['startRow'],
			});
		}
		let mappingCount = 0;
		for (const [rowOffset, row] of window.rows.entries()) {
			if (row.index !== window.startRow + BigInt(rowOffset)) {
				context.addIssue({
					code: 'custom',
					message: 'Text rows must be contiguous from startRow',
					path: ['rows', rowOffset, 'index'],
				});
			}
			const textBytes = textEncoder.encode(row.text);
			let previousByteOffset = -1;
			let previousUtf16Offset = -1;
			let previousColumn = -1;
			for (const [mappingOffset, mapping] of row.mappings.entries()) {
				mappingCount += 1;
				if (
					mapping.byteOffset > textBytes.byteLength ||
					mapping.utf16Offset > row.text.length ||
					mapping.column >= window.columns ||
					mapping.byteOffset <= previousByteOffset ||
					mapping.utf16Offset <= previousUtf16Offset ||
					mapping.column <= previousColumn
				) {
					context.addIssue({
						code: 'custom',
						message: 'Text cell mappings must be bounded and strictly ordered',
						path: ['rows', rowOffset, 'mappings', mappingOffset],
					});
				}
				previousByteOffset = mapping.byteOffset;
				previousUtf16Offset = mapping.utf16Offset;
				previousColumn = mapping.column;
			}
		}
		if (mappingCount > PROTOCOL_LIMITS.maxFrameCells) {
			context.addIssue({
				code: 'custom',
				message: `Text row window exceeds ${PROTOCOL_LIMITS.maxFrameCells} mappings`,
				path: ['rows'],
			});
		}
	});

export const TerminalRowWindowSchema = z.discriminatedUnion('detail', [
	TerminalRenderRowWindowSchema,
	TerminalTextRowWindowSchema,
]);

export type TerminalRenderRowWindow = z.infer<typeof TerminalRenderRowWindowSchema>;
export type TerminalRowWindow = z.infer<typeof TerminalRowWindowSchema>;
export type TerminalTextCellMapping = z.infer<typeof TerminalTextCellMappingSchema>;
export type TerminalTextRow = z.infer<typeof TerminalTextRowSchema>;
export type TerminalTextRowWindow = z.infer<typeof TerminalTextRowWindowSchema>;

export interface DecodedTerminalTextRow {
	readonly byteOffsets: Uint32Array;
	readonly columns: Uint16Array;
	/** The UTF-8 byte, UTF-16 code-unit, and terminal-column offsets are all the character index. */
	readonly identityAscii: boolean;
	readonly index: bigint;
	readonly promptClass: RowPromptClassValue;
	readonly text: string;
	readonly utf16Offsets: Uint32Array;
	readonly widths: Uint8Array;
	readonly wrap: boolean;
	readonly wrapContinuation: boolean;
}

interface DecodedTerminalRowWindowBase {
	readonly attachmentId: AttachmentId;
	readonly columns: number;
	readonly generation: bigint;
	readonly hostEpoch: HostEpoch;
	readonly ignoredOptionalFlags: number;
	readonly revision: bigint;
	readonly sequence: bigint;
	readonly startRow: bigint;
	readonly totalRows: bigint;
}

export interface DecodedTerminalRenderRowWindow extends DecodedTerminalRowWindowBase {
	readonly detail: 'render';
	readonly frame: DecodedTerminalFrame;
	readonly frameBytes: Uint8Array;
}

export interface DecodedTerminalTextRowWindow extends DecodedTerminalRowWindowBase {
	readonly detail: 'text';
	readonly rows: readonly DecodedTerminalTextRow[];
}

export type DecodedTerminalRowWindow =
	| DecodedTerminalRenderRowWindow
	| DecodedTerminalTextRowWindow;

interface RowWindowHeader extends DecodedTerminalRowWindowBase {
	readonly bodyLength: number;
	readonly detail: TerminalRowWindowDetail;
	readonly entryCount: number;
	readonly formatMinor: number;
	readonly rowCount: number;
}

function detailCode(detail: TerminalRowWindowDetail): number {
	return detail === TerminalRowWindowDetail.render ? 1 : 2;
}

function detailFromCode(code: number): ProtocolResult<TerminalRowWindowDetail> {
	if (code === 1) return protocolSuccess(TerminalRowWindowDetail.render);
	if (code === 2) return protocolSuccess(TerminalRowWindowDetail.text);
	return protocolFailure(ProtocolErrorCode.invalidField, `Unknown row-window detail ${code}`);
}

function promptClassCode(value: RowPromptClassValue): number {
	if (value === RowPromptClass.none) return TerminalRowPromptClassCode.none;
	if (value === RowPromptClass.prompt) return TerminalRowPromptClassCode.prompt;
	return TerminalRowPromptClassCode.promptContinuation;
}

function promptClassFromCode(code: number): ProtocolResult<RowPromptClassValue> {
	if (code === TerminalRowPromptClassCode.none) return protocolSuccess(RowPromptClass.none);
	if (code === TerminalRowPromptClassCode.prompt) return protocolSuccess(RowPromptClass.prompt);
	if (code === TerminalRowPromptClassCode.promptContinuation) {
		return protocolSuccess(RowPromptClass.promptContinuation);
	}
	return protocolFailure(ProtocolErrorCode.invalidField, `Unknown row prompt class ${code}`);
}

function encodeHeader(
	window: TerminalRowWindow,
	body: Uint8Array,
	entryCount: number,
	rowCount: number,
): ProtocolResult<Uint8Array> {
	const totalLength = checkedAdd(TERMINAL_ROW_WINDOW_HEADER_BYTES, body.byteLength);
	if (!totalLength.ok) return totalLength;
	if (totalLength.value > PROTOCOL_LIMITS.maxRowWindowBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			`Terminal row window exceeds ${PROTOCOL_LIMITS.maxRowWindowBytes} bytes`,
		);
	}
	const writer = new BinaryWriter(totalLength.value);
	writer.writeBytes(TERMINAL_ROW_WINDOW_MAGIC);
	writer.writeUint8(TERMINAL_ROW_WINDOW_FORMAT.major);
	writer.writeUint8(TERMINAL_ROW_WINDOW_FORMAT.minor);
	writer.writeUint8(detailCode(window.detail));
	writer.writeUint8(0);
	writer.writeUint32(totalLength.value);
	writer.writeUint16(TERMINAL_ROW_WINDOW_HEADER_BYTES);
	writer.writeUint16(window.columns);
	writer.writeBytes(identifierBytes(window.hostEpoch));
	writer.writeBytes(identifierBytes(window.attachmentId));
	writer.writeUint64(window.sequence);
	writer.writeUint64(window.revision);
	writer.writeUint64(window.generation);
	writer.writeUint64(window.totalRows);
	writer.writeUint64(window.startRow);
	writer.writeUint16(rowCount);
	writer.writeUint16(0);
	writer.writeUint32(entryCount);
	writer.writeUint32(body.byteLength);
	writer.writeBytes(new Uint8Array(12));
	writer.writeBytes(body);
	return protocolSuccess(writer.bytes());
}

function encodeTextBody(window: TerminalTextRowWindow): ProtocolResult<Uint8Array> {
	const encodedRows = window.rows.map((row) => textEncoder.encode(row.text));
	const identityRows = window.rows.map((row, rowOffset) =>
		usesIdentityAsciiMapping(row, encodedRows[rowOffset] ?? new Uint8Array()),
	);
	let bodyLength = 0;
	for (const [rowOffset, row] of window.rows.entries()) {
		const serializedMappingCount = identityRows[rowOffset] ? 0 : row.mappings.length;
		const mappingsLength = checkedMultiply(serializedMappingCount, TEXT_MAPPING_BYTES);
		if (!mappingsLength.ok) return mappingsLength;
		const rowLength = checkedAdd(
			TEXT_ROW_HEADER_BYTES,
			encodedRows[rowOffset]?.byteLength ?? 0,
			mappingsLength.value,
		);
		if (!rowLength.ok) return rowLength;
		const nextLength = checkedAdd(bodyLength, rowLength.value);
		if (!nextLength.ok) return nextLength;
		bodyLength = nextLength.value;
	}
	if (bodyLength > PROTOCOL_LIMITS.maxRowWindowBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'Text row body exceeds its limit');
	}

	const writer = new BinaryWriter(bodyLength);
	for (const [rowOffset, row] of window.rows.entries()) {
		const text = encodedRows[rowOffset];
		if (!text) {
			return protocolFailure(ProtocolErrorCode.invalidField, 'Text row encoding is missing');
		}
		writer.writeUint8(
			(row.wrap ? TerminalRowFlag.wrap : 0) |
				(row.wrapContinuation ? TerminalRowFlag.wrapContinuation : 0) |
				(identityRows[rowOffset] ? TerminalTextRowFlag.identityAscii : 0),
		);
		writer.writeUint8(promptClassCode(row.promptClass));
		writer.writeUint16(0);
		writer.writeUint32(text.byteLength);
		writer.writeUint32(identityRows[rowOffset] ? 0 : row.mappings.length);
		writer.writeBytes(text);
		for (const mapping of identityRows[rowOffset] ? [] : row.mappings) {
			writer.writeUint32(mapping.byteOffset);
			writer.writeUint32(mapping.utf16Offset);
			writer.writeUint16(mapping.column);
			writer.writeUint8(mapping.width);
			writer.writeUint8(0);
		}
	}
	return protocolSuccess(writer.bytes());
}

function usesIdentityAsciiMapping(row: TerminalTextRow, encodedText: Uint8Array): boolean {
	if (encodedText.byteLength !== row.text.length || row.mappings.length !== row.text.length) {
		return false;
	}
	for (const [index, byte] of encodedText.entries()) {
		const mapping = row.mappings[index];
		if (
			byte > 0x7f ||
			mapping === undefined ||
			mapping.byteOffset !== index ||
			mapping.utf16Offset !== index ||
			mapping.column !== index ||
			mapping.width !== 1
		) {
			return false;
		}
	}
	return true;
}

export function encodeTerminalRowWindow(value: unknown): ProtocolResult<Uint8Array> {
	const parsed = TerminalRowWindowSchema.safeParse(value);
	if (!parsed.success) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`Row-window validation failed: ${parsed.error.issues[0]?.message ?? 'unknown field error'}`,
		);
	}
	const window = parsed.data;
	if (window.detail === TerminalRowWindowDetail.render) {
		const encoded = encodeTerminalFrame(window.frame);
		if (!encoded.ok) return encoded;
		return encodeHeader(
			window,
			encoded.value,
			window.frame.rows * window.frame.columns,
			window.frame.rows,
		);
	}
	const body = encodeTextBody(window);
	if (!body.ok) return body;
	const mappingCount = window.rows.reduce((total, row) => total + row.mappings.length, 0);
	return encodeHeader(window, body.value, mappingCount, window.rows.length);
}

function isMagic(bytes: Uint8Array): boolean {
	return TERMINAL_ROW_WINDOW_MAGIC.every((value, index) => bytes[index] === value);
}

function allZero(bytes: Uint8Array): boolean {
	return bytes.every((value) => value === 0);
}

function decodeHeader(bytes: Uint8Array): ProtocolResult<RowWindowHeader> {
	if (bytes.byteLength < TERMINAL_ROW_WINDOW_HEADER_BYTES) {
		return protocolFailure(
			ProtocolErrorCode.truncated,
			`Terminal row window requires at least ${TERMINAL_ROW_WINDOW_HEADER_BYTES} bytes`,
		);
	}
	if (bytes.byteLength > PROTOCOL_LIMITS.maxRowWindowBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			'Terminal row window exceeds its limit',
		);
	}
	if (!isMagic(bytes)) {
		return protocolFailure(ProtocolErrorCode.invalidMagic, 'Terminal row-window magic is not OBRW');
	}

	const reader = new BinaryReader(bytes, TERMINAL_ROW_WINDOW_MAGIC.byteLength);
	const formatMajor = reader.readUint8();
	const formatMinor = reader.readUint8();
	const detailValue = reader.readUint8();
	const flags = reader.readUint8();
	const totalLength = reader.readUint32();
	const headerLength = reader.readUint16();
	const columns = reader.readUint16();
	const hostEpochBytes = reader.readBytes(IDENTIFIER_BYTE_LENGTH);
	const attachmentIdBytes = reader.readBytes(IDENTIFIER_BYTE_LENGTH);
	const sequence = reader.readUint64();
	const revision = reader.readUint64();
	const generation = reader.readUint64();
	const totalRows = reader.readUint64();
	const startRow = reader.readUint64();
	const rowCount = reader.readUint16();
	const reserved = reader.readUint16();
	const entryCount = reader.readUint32();
	const bodyLength = reader.readUint32();
	const reservedTail = reader.readBytes(12);
	if (reader.failure) return reader.failure;

	if (
		formatMajor !== TERMINAL_ROW_WINDOW_FORMAT.major ||
		formatMinor > TERMINAL_ROW_WINDOW_FORMAT.minor ||
		headerLength !== TERMINAL_ROW_WINDOW_HEADER_BYTES
	) {
		return protocolFailure(
			ProtocolErrorCode.unsupportedVersion,
			`Unsupported terminal row-window format ${formatMajor}.${formatMinor}`,
		);
	}
	if (totalLength !== bytes.byteLength || bodyLength !== bytes.byteLength - headerLength) {
		return protocolFailure(ProtocolErrorCode.lengthMismatch, 'Terminal row-window length mismatch');
	}
	if (reserved !== 0 || !allZero(reservedTail) || (flags & 0x80) !== 0) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredFlag,
			'Terminal row window contains a non-zero reserved or required flag',
		);
	}
	if (
		columns === 0 ||
		columns > PROTOCOL_LIMITS.maxFrameColumns ||
		rowCount === 0 ||
		rowCount > PROTOCOL_LIMITS.maxFrameRows ||
		entryCount > PROTOCOL_LIMITS.maxFrameCells ||
		sequence === 0n ||
		generation === 0n ||
		startRow + BigInt(rowCount) > totalRows
	) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Terminal row-window shape is invalid');
	}
	const detail = detailFromCode(detailValue);
	if (!detail.ok) return detail;
	const hostEpochText = identifierText(hostEpochBytes);
	if (!hostEpochText.ok) return hostEpochText;
	const hostEpoch = parseHostEpoch(hostEpochText.value);
	if (!hostEpoch.ok) return hostEpoch;
	const attachmentIdText = identifierText(attachmentIdBytes);
	if (!attachmentIdText.ok) return attachmentIdText;
	const attachmentId = parseAttachmentId(attachmentIdText.value);
	if (!attachmentId.ok) return attachmentId;
	return protocolSuccess({
		attachmentId: attachmentId.value,
		bodyLength,
		columns,
		detail: detail.value,
		entryCount,
		formatMinor,
		generation,
		hostEpoch: hostEpoch.value,
		ignoredOptionalFlags: flags & 0x7f,
		revision,
		rowCount,
		sequence,
		startRow,
		totalRows,
	});
}

function decodeRenderWindow(
	header: RowWindowHeader,
	body: Uint8Array,
): ProtocolResult<DecodedTerminalRenderRowWindow> {
	const decoded = decodeTerminalFrame(body);
	if (!decoded.ok) return decoded;
	const frame = decoded.value;
	if (
		frame.kind !== TerminalFrameKind.snapshot ||
		frame.attachmentId !== header.attachmentId ||
		frame.hostEpoch !== header.hostEpoch ||
		frame.sequence !== header.sequence ||
		frame.revision !== header.revision ||
		frame.generation !== header.generation ||
		frame.columns !== header.columns ||
		frame.rows !== header.rowCount ||
		header.entryCount !== frame.rows * frame.columns
	) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Render row-window metadata does not match its nested snapshot frame',
		);
	}
	return protocolSuccess({
		attachmentId: header.attachmentId,
		columns: header.columns,
		detail: TerminalRowWindowDetail.render,
		frame,
		frameBytes: body.slice(),
		generation: header.generation,
		hostEpoch: header.hostEpoch,
		ignoredOptionalFlags: header.ignoredOptionalFlags,
		revision: header.revision,
		sequence: header.sequence,
		startRow: header.startRow,
		totalRows: header.totalRows,
	});
}

function decodeTextWindow(
	header: RowWindowHeader,
	body: Uint8Array,
): ProtocolResult<DecodedTerminalTextRowWindow> {
	const reader = new BinaryReader(body);
	const rows: DecodedTerminalTextRow[] = [];
	let decodedMappings = 0;
	for (let rowOffset = 0; rowOffset < header.rowCount; rowOffset += 1) {
		const rowFlags = reader.readUint8();
		const promptClassValue = reader.readUint8();
		const reserved = reader.readUint16();
		const textLength = reader.readUint32();
		const mappingCount = reader.readUint32();
		if (reader.failure) return reader.failure;
		const supportedRowFlags =
			ROW_FLAGS_MASK | (header.formatMinor >= 1 ? TerminalTextRowFlag.identityAscii : 0);
		if (reserved !== 0 || (rowFlags & ~supportedRowFlags) !== 0) {
			return protocolFailure(
				ProtocolErrorCode.unknownRequiredFlag,
				'Text row contains a non-zero reserved or required flag',
			);
		}
		const identityAscii = (rowFlags & TerminalTextRowFlag.identityAscii) !== 0;
		if (mappingCount > header.columns) {
			return protocolFailure(
				ProtocolErrorCode.limitExceeded,
				'Text row mapping table exceeds a limit',
			);
		}
		const encodedText = reader.readBytes(textLength);
		if (reader.failure) return reader.failure;
		let text: string;
		try {
			text = textDecoder.decode(encodedText);
		} catch {
			return protocolFailure(ProtocolErrorCode.invalidUtf8, 'Text row is not valid UTF-8');
		}
		if (
			identityAscii &&
			(mappingCount !== 0 ||
				encodedText.byteLength > header.columns ||
				!encodedText.every((byte) => byte <= 0x7f))
		) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				'Implicit text mapping must be bounded narrow ASCII',
			);
		}
		const logicalMappingCount = identityAscii ? encodedText.byteLength : mappingCount;
		if (logicalMappingCount > PROTOCOL_LIMITS.maxFrameCells - decodedMappings) {
			return protocolFailure(
				ProtocolErrorCode.limitExceeded,
				'Text row mapping table exceeds a limit',
			);
		}
		const byteOffsets = new Uint32Array(mappingCount);
		const utf16Offsets = new Uint32Array(mappingCount);
		const columns = new Uint16Array(mappingCount);
		const widths = new Uint8Array(mappingCount);
		let previousByteOffset = -1;
		let previousUtf16Offset = -1;
		let previousColumn = -1;
		for (let mappingOffset = 0; mappingOffset < mappingCount; mappingOffset += 1) {
			const byteOffset = reader.readUint32();
			const utf16Offset = reader.readUint32();
			const column = reader.readUint16();
			const width = reader.readUint8();
			const mappingReserved = reader.readUint8();
			if (reader.failure) return reader.failure;
			if (mappingReserved !== 0) {
				return protocolFailure(
					ProtocolErrorCode.unknownRequiredFlag,
					'Text mapping contains a non-zero reserved flag',
				);
			}
			if (
				byteOffset > encodedText.byteLength ||
				utf16Offset > text.length ||
				column >= header.columns ||
				(width !== 1 && width !== 2) ||
				byteOffset <= previousByteOffset ||
				utf16Offset <= previousUtf16Offset ||
				column <= previousColumn
			) {
				return protocolFailure(ProtocolErrorCode.invalidField, 'Text mapping is not canonical');
			}
			byteOffsets[mappingOffset] = byteOffset;
			utf16Offsets[mappingOffset] = utf16Offset;
			columns[mappingOffset] = column;
			widths[mappingOffset] = width;
			previousByteOffset = byteOffset;
			previousUtf16Offset = utf16Offset;
			previousColumn = column;
		}
		decodedMappings += logicalMappingCount;
		const promptClass = promptClassFromCode(promptClassValue);
		if (!promptClass.ok) return promptClass;
		rows.push({
			byteOffsets,
			columns,
			identityAscii,
			index: header.startRow + BigInt(rowOffset),
			promptClass: promptClass.value,
			text,
			utf16Offsets,
			widths,
			wrap: (rowFlags & TerminalRowFlag.wrap) !== 0,
			wrapContinuation: (rowFlags & TerminalRowFlag.wrapContinuation) !== 0,
		});
	}
	if (reader.offset !== body.byteLength || decodedMappings !== header.entryCount) {
		return protocolFailure(ProtocolErrorCode.lengthMismatch, 'Text row-window table mismatch');
	}
	return protocolSuccess({
		attachmentId: header.attachmentId,
		columns: header.columns,
		detail: TerminalRowWindowDetail.text,
		generation: header.generation,
		hostEpoch: header.hostEpoch,
		ignoredOptionalFlags: header.ignoredOptionalFlags,
		revision: header.revision,
		rows,
		sequence: header.sequence,
		startRow: header.startRow,
		totalRows: header.totalRows,
	});
}

export function decodeTerminalRowWindow(
	bytes: Uint8Array,
): ProtocolResult<DecodedTerminalRowWindow> {
	const header = decodeHeader(bytes);
	if (!header.ok) return header;
	const body = bytes.subarray(TERMINAL_ROW_WINDOW_HEADER_BYTES);
	return header.value.detail === TerminalRowWindowDetail.render
		? decodeRenderWindow(header.value, body)
		: decodeTextWindow(header.value, body);
}
