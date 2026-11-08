import { z } from 'zod';

import {
	BinaryReader,
	BinaryWriter,
	checkedAdd,
	checkedMultiply,
	copyBytesToFixedArrayBuffer,
	isSharedArrayBuffer,
} from './binary';
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
	RowPromptClass,
	TerminalFrameKind,
	TerminalFrameSchema,
	TerminalRowFlag,
	TerminalRowPromptClassCode,
} from './frame';
import type { RowPromptClass as RowPromptClassValue } from './frame';
import { PROTOCOL_LIMITS } from './limits';
import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';
import { containsUnpairedSurrogate } from './containsUnpairedSurrogate';
import {
	decodeTerminalProjectionBuffer,
	encodeTerminalProjectionBufferWithin,
	viewTerminalProjectionBuffer,
} from './projection-buffer';
import type { DecodedTerminalProjectionBuffer } from './projection-buffer';
import {
	decodeTerminalGraphics,
	encodeTerminalGraphics,
	TERMINAL_GRAPHICS_HEADER_BYTES,
	TerminalGraphicsUpdateKind,
} from './terminal-graphics';
import type { TerminalGraphics } from './terminal-graphics';
import { utf8ByteLength } from './utf8';

export const TERMINAL_ROW_WINDOW_HEADER_BYTES = 112;
const TERMINAL_ROW_WINDOW_MAGIC = new Uint8Array([0x4f, 0x42, 0x52, 0x57]);
// Keep this synchronized with schemas/terminal/v1/constants.json, Rust and Zig. Released changes (>= 1.0.0) also update
// schemas/terminal/docs/version-history.md in the same change.
export const TERMINAL_ROW_WINDOW_FORMAT = { major: 1, minor: 0, patch: 0 } as const;

const TerminalTextRowFlag = {
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
const EMPTY_UINT8_ARRAY = new Uint8Array(0);
const EMPTY_UINT16_ARRAY = new Uint16Array(0);
const EMPTY_UINT32_ARRAY = new Uint32Array(0);
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

const TerminalTextCellMappingSchema = z.strictObject({
	byteOffset: uint32Schema,
	column: columnSchema,
	utf16Offset: uint32Schema,
	width: z.number().int().min(1).max(2),
});

const TerminalTextRowSchema = z.strictObject({
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

const TerminalRenderRowWindowSchema = z
	.strictObject({
		...rowWindowBaseShape,
		detail: z.literal(TerminalRowWindowDetail.render),
		frame: TerminalFrameSchema,
		graphics: z.custom<TerminalGraphics>(),
	})
	.superRefine((window, context) => {
		if (window.graphics.kind !== TerminalGraphicsUpdateKind.snapshot) {
			context.addIssue({
				code: 'custom',
				message: 'Render row windows require a graphics resource snapshot',
				path: ['graphics'],
			});
		}
		if (
			window.frame.kind !== TerminalFrameKind.snapshot ||
			window.frame.attachmentId !== window.attachmentId ||
			window.frame.hostEpoch !== window.hostEpoch ||
			window.frame.sequence !== window.sequence ||
			window.frame.revision !== window.revision ||
			window.frame.generation !== window.generation ||
			window.frame.columns !== window.columns ||
			window.frame.scrollbackRows !== window.startRow
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

const TerminalTextRowWindowSchema = z
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
			if (containsUnpairedSurrogate(row.text)) {
				context.addIssue({
					code: 'custom',
					message: 'Row text contains an unpaired UTF-16 surrogate',
					path: ['rows', rowOffset, 'text'],
				});
				continue;
			}
			const priorRow = window.rows[rowOffset - 1];
			if (priorRow !== undefined && priorRow.wrap !== row.wrapContinuation) {
				context.addIssue({
					code: 'custom',
					message: 'Adjacent row wrap flags must agree',
					path: ['rows', rowOffset, 'wrapContinuation'],
				});
			}
			if (row.index !== window.startRow + BigInt(rowOffset)) {
				context.addIssue({
					code: 'custom',
					message: 'Text rows must be contiguous from startRow',
					path: ['rows', rowOffset, 'index'],
				});
			}
			const textByteLength = utf8ByteLength(row.text);
			if (textByteLength > 0 && row.mappings.length === 0) {
				context.addIssue({
					code: 'custom',
					message: 'A non-empty text row requires a mapping',
					path: ['rows', rowOffset, 'mappings'],
				});
			}
			let previousByteOffset = -1;
			let previousUtf16Offset = -1;
			let previousColumnEnd = 0;
			const textPosition = { byteOffset: 0, utf16Offset: 0 };
			for (const [mappingOffset, mapping] of row.mappings.entries()) {
				mappingCount += 1;
				if (
					mapping.byteOffset >= textByteLength ||
					mapping.utf16Offset >= row.text.length ||
					mapping.column + mapping.width > window.columns ||
					(mappingOffset === 0 && (mapping.byteOffset !== 0 || mapping.utf16Offset !== 0)) ||
					!matchesTextPosition(row.text, mapping.byteOffset, mapping.utf16Offset, textPosition) ||
					mapping.byteOffset <= previousByteOffset ||
					mapping.utf16Offset <= previousUtf16Offset ||
					mapping.column < previousColumnEnd
				) {
					context.addIssue({
						code: 'custom',
						message: 'Text cell mappings must be bounded and strictly ordered',
						path: ['rows', rowOffset, 'mappings', mappingOffset],
					});
				}
				if (
					previousByteOffset >= 0 &&
					mapping.byteOffset - previousByteOffset > PROTOCOL_LIMITS.maxStringBytes
				) {
					context.addIssue({
						code: 'custom',
						message: 'Text cell mapping spans a byte range above the per-cell limit',
						path: ['rows', rowOffset, 'mappings', mappingOffset],
					});
				}
				previousByteOffset = mapping.byteOffset;
				previousUtf16Offset = mapping.utf16Offset;
				previousColumnEnd = mapping.column + mapping.width;
			}
			if (
				row.mappings.length > 0 &&
				textByteLength - previousByteOffset > PROTOCOL_LIMITS.maxStringBytes
			) {
				context.addIssue({
					code: 'custom',
					message: 'Text cell mapping spans a byte range above the per-cell limit',
					path: ['rows', rowOffset, 'mappings', row.mappings.length - 1],
				});
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

interface Utf8Position {
	byteOffset: number;
	utf16Offset: number;
}

function matchesTextPosition(
	text: string,
	targetByteOffset: number,
	targetUtf16Offset: number,
	position: Utf8Position,
): boolean {
	while (position.byteOffset < targetByteOffset && position.utf16Offset < text.length) {
		const codeUnit = text.charCodeAt(position.utf16Offset);
		let encodedLength = 3;
		let utf16Length = 1;
		if (codeUnit <= 0x7f) encodedLength = 1;
		else if (codeUnit <= 0x7ff) encodedLength = 2;
		else if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
			encodedLength = 4;
			utf16Length = 2;
		}
		position.byteOffset += encodedLength;
		position.utf16Offset += utf16Length;
	}
	return position.byteOffset === targetByteOffset && position.utf16Offset === targetUtf16Offset;
}

function isAscii(bytes: Uint8Array): boolean {
	for (const byte of bytes) {
		if (byte > 0x7f) return false;
	}
	return true;
}

/** Advances over already-validated UTF-8 without materializing every scalar boundary. */
function matchesUtf8Position(
	bytes: Uint8Array,
	targetByteOffset: number,
	targetUtf16Offset: number,
	position: Utf8Position,
): boolean {
	while (position.byteOffset < targetByteOffset) {
		const leadingByte = bytes[position.byteOffset];
		if (leadingByte === undefined) return false;
		let encodedLength: number;
		if (leadingByte <= 0x7f) encodedLength = 1;
		else if (leadingByte >= 0xc2 && leadingByte <= 0xdf) encodedLength = 2;
		else if (leadingByte >= 0xe0 && leadingByte <= 0xef) encodedLength = 3;
		else if (leadingByte >= 0xf0 && leadingByte <= 0xf4) encodedLength = 4;
		else return false;
		position.byteOffset += encodedLength;
		position.utf16Offset += encodedLength === 4 ? 2 : 1;
	}
	return position.byteOffset === targetByteOffset && position.utf16Offset === targetUtf16Offset;
}

const TerminalRowWindowSchema = z.discriminatedUnion('detail', [
	TerminalRenderRowWindowSchema,
	TerminalTextRowWindowSchema,
]);

type TerminalRowWindow = z.infer<typeof TerminalRowWindowSchema>;

type TerminalTextRow = z.infer<typeof TerminalTextRowSchema>;
type TerminalTextRowWindow = z.infer<typeof TerminalTextRowWindowSchema>;

interface DecodedTerminalTextRow {
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

interface DecodedTerminalRenderRowWindow extends DecodedTerminalRowWindowBase {
	readonly detail: 'render';
	readonly frame: DecodedTerminalProjectionBuffer;
	readonly graphics: TerminalGraphics;
}

interface DecodedTerminalTextRowWindow extends DecodedTerminalRowWindowBase {
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
	readonly graphicsLength: number;
	readonly projectionLength: number;
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
	projectionLength: number,
	graphicsLength: number,
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
	writer.writeUint8(TERMINAL_ROW_WINDOW_FORMAT.patch);
	writer.writeUint32(projectionLength);
	writer.writeUint32(graphicsLength);
	writer.writeBytes(new Uint8Array(3));
	writer.writeBytes(body);
	return protocolSuccess(writer.bytes());
}

function encodeTextBody(window: TerminalTextRowWindow): ProtocolResult<Uint8Array> {
	const textByteLengths: number[] = [];
	const identityRows: boolean[] = [];
	const maximumBodyBytes = PROTOCOL_LIMITS.maxRowWindowBytes - TERMINAL_ROW_WINDOW_HEADER_BYTES;
	let bodyLength = 0;
	for (const row of window.rows) {
		const textByteLength = utf8ByteLength(row.text);
		const identityAscii = usesIdentityAsciiMapping(row);
		textByteLengths.push(textByteLength);
		identityRows.push(identityAscii);
		const serializedMappingCount = identityAscii ? 0 : row.mappings.length;
		const mappingsLength = checkedMultiply(serializedMappingCount, TEXT_MAPPING_BYTES);
		if (!mappingsLength.ok) return mappingsLength;
		const rowLength = checkedAdd(TEXT_ROW_HEADER_BYTES, textByteLength, mappingsLength.value);
		if (!rowLength.ok) return rowLength;
		const nextLength = checkedAdd(bodyLength, rowLength.value);
		if (!nextLength.ok) return nextLength;
		bodyLength = nextLength.value;
		if (bodyLength > maximumBodyBytes) {
			return protocolFailure(ProtocolErrorCode.limitExceeded, 'Text row body exceeds its limit');
		}
	}

	const writer = new BinaryWriter(bodyLength);
	for (const [rowOffset, row] of window.rows.entries()) {
		const text = textEncoder.encode(row.text);
		if (text.byteLength !== textByteLengths[rowOffset]) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				'Text row encoding is inconsistent with its bounded preflight',
			);
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

function usesIdentityAsciiMapping(row: TerminalTextRow): boolean {
	if (row.mappings.length !== row.text.length) return false;
	for (let index = 0; index < row.text.length; index += 1) {
		const codeUnit = row.text.charCodeAt(index);
		const mapping = row.mappings[index];
		if (
			codeUnit > 0x7f ||
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
		const encoded = encodeTerminalProjectionBufferWithin(
			window.frame,
			PROTOCOL_LIMITS.maxFrameBytes,
		);
		if (!encoded.ok) return encoded;
		const graphics = encodeTerminalGraphics(window.graphics);
		if (!graphics.ok) return graphics;
		const bodyLength = checkedAdd(encoded.value.byteLength, graphics.value.byteLength);
		if (!bodyLength.ok) return bodyLength;
		const body = new Uint8Array(bodyLength.value);
		body.set(encoded.value, 0);
		body.set(graphics.value, encoded.value.byteLength);
		return encodeHeader(
			window,
			body,
			window.frame.rows * window.frame.columns,
			window.frame.rows,
			encoded.value.byteLength,
			graphics.value.byteLength,
		);
	}
	const body = encodeTextBody(window);
	if (!body.ok) return body;
	const mappingCount = window.rows.reduce((total, row) => total + row.mappings.length, 0);
	return encodeHeader(window, body.value, mappingCount, window.rows.length, 0, 0);
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
	const formatPatch = reader.readUint8();
	const projectionLength = reader.readUint32();
	const graphicsLength = reader.readUint32();
	const reservedTail = reader.readBytes(3);
	if (reader.failure) return reader.failure;

	if (
		formatMajor !== TERMINAL_ROW_WINDOW_FORMAT.major ||
		formatMinor !== TERMINAL_ROW_WINDOW_FORMAT.minor ||
		formatPatch !== TERMINAL_ROW_WINDOW_FORMAT.patch ||
		headerLength !== TERMINAL_ROW_WINDOW_HEADER_BYTES
	) {
		return protocolFailure(
			ProtocolErrorCode.unsupportedVersion,
			`Unsupported terminal row-window format ${formatMajor}.${formatMinor}.${formatPatch}`,
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
	if (
		(detail.value === TerminalRowWindowDetail.render &&
			(projectionLength === 0 ||
				graphicsLength < TERMINAL_GRAPHICS_HEADER_BYTES ||
				projectionLength + graphicsLength !== bodyLength)) ||
		(detail.value === TerminalRowWindowDetail.text &&
			(projectionLength !== 0 || graphicsLength !== 0))
	) {
		return protocolFailure(
			ProtocolErrorCode.lengthMismatch,
			'Terminal row-window body sections disagree with the header',
		);
	}
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
		generation,
		graphicsLength,
		hostEpoch: hostEpoch.value,
		ignoredOptionalFlags: flags & 0x7f,
		revision,
		rowCount,
		projectionLength,
		sequence,
		startRow,
		totalRows,
	});
}

function decodeRenderWindow(
	header: RowWindowHeader,
	body: Uint8Array,
	decodeProjection: (bytes: Uint8Array) => ProtocolResult<DecodedTerminalProjectionBuffer>,
): ProtocolResult<DecodedTerminalRenderRowWindow> {
	const decoded = decodeProjection(body.subarray(0, header.projectionLength));
	if (!decoded.ok) return decoded;
	const decodedGraphics = decodeTerminalGraphics(
		body.subarray(header.projectionLength, header.projectionLength + header.graphicsLength),
	);
	if (!decodedGraphics.ok) return decodedGraphics;
	if (decodedGraphics.value.kind !== TerminalGraphicsUpdateKind.snapshot) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Render row-window graphics must be a resource snapshot',
		);
	}
	const frame = decoded.value;
	if (
		frame.kind !== TerminalFrameKind.snapshot ||
		frame.attachmentId !== header.attachmentId ||
		frame.hostEpoch !== header.hostEpoch ||
		frame.sequence !== header.sequence ||
		frame.revision !== header.revision ||
		frame.generation !== header.generation ||
		frame.columns !== header.columns ||
		frame.scrollbackRows !== header.startRow ||
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
		generation: header.generation,
		graphics: decodedGraphics.value,
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
		const supportedRowFlags = ROW_FLAGS_MASK | TerminalTextRowFlag.identityAscii;
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
			(mappingCount !== 0 || encodedText.byteLength > header.columns || !isAscii(encodedText))
		) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				'Implicit text mapping must be bounded narrow ASCII',
			);
		}
		const logicalMappingCount = identityAscii ? encodedText.byteLength : mappingCount;
		if (!identityAscii && encodedText.byteLength > 0 && mappingCount === 0) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				'A non-empty text row requires a mapping',
			);
		}
		if (logicalMappingCount > PROTOCOL_LIMITS.maxFrameCells - decodedMappings) {
			return protocolFailure(
				ProtocolErrorCode.limitExceeded,
				'Text row mapping table exceeds a limit',
			);
		}
		const byteOffsets = mappingCount === 0 ? EMPTY_UINT32_ARRAY : new Uint32Array(mappingCount);
		const utf16Offsets = mappingCount === 0 ? EMPTY_UINT32_ARRAY : new Uint32Array(mappingCount);
		const columns = mappingCount === 0 ? EMPTY_UINT16_ARRAY : new Uint16Array(mappingCount);
		const widths = mappingCount === 0 ? EMPTY_UINT8_ARRAY : new Uint8Array(mappingCount);
		let previousByteOffset = -1;
		let previousUtf16Offset = -1;
		let previousColumnEnd = 0;
		const textPosition = { byteOffset: 0, utf16Offset: 0 };
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
				byteOffset >= encodedText.byteLength ||
				utf16Offset >= text.length ||
				column + width > header.columns ||
				(width !== 1 && width !== 2) ||
				(mappingOffset === 0 && (byteOffset !== 0 || utf16Offset !== 0)) ||
				!matchesUtf8Position(encodedText, byteOffset, utf16Offset, textPosition) ||
				byteOffset <= previousByteOffset ||
				utf16Offset <= previousUtf16Offset ||
				column < previousColumnEnd
			) {
				return protocolFailure(ProtocolErrorCode.invalidField, 'Text mapping is not canonical');
			}
			if (
				previousByteOffset >= 0 &&
				byteOffset - previousByteOffset > PROTOCOL_LIMITS.maxStringBytes
			) {
				return protocolFailure(
					ProtocolErrorCode.limitExceeded,
					'Text cell mapping spans a byte range above its limit',
				);
			}
			byteOffsets[mappingOffset] = byteOffset;
			utf16Offsets[mappingOffset] = utf16Offset;
			columns[mappingOffset] = column;
			widths[mappingOffset] = width;
			previousByteOffset = byteOffset;
			previousUtf16Offset = utf16Offset;
			previousColumnEnd = column + width;
		}
		if (
			mappingCount > 0 &&
			encodedText.byteLength - (byteOffsets[mappingCount - 1] ?? 0) > PROTOCOL_LIMITS.maxStringBytes
		) {
			return protocolFailure(
				ProtocolErrorCode.limitExceeded,
				'Text cell mapping spans a byte range above its limit',
			);
		}
		decodedMappings += logicalMappingCount;
		const promptClass = promptClassFromCode(promptClassValue);
		if (!promptClass.ok) return promptClass;
		const wrap = (rowFlags & TerminalRowFlag.wrap) !== 0;
		const wrapContinuation = (rowFlags & TerminalRowFlag.wrapContinuation) !== 0;
		if (rows.at(-1)?.wrap !== undefined && rows.at(-1)?.wrap !== wrapContinuation) {
			return protocolFailure(ProtocolErrorCode.invalidField, 'Adjacent row wrap flags disagree');
		}
		rows.push({
			byteOffsets,
			columns,
			identityAscii,
			index: header.startRow + BigInt(rowOffset),
			promptClass: promptClass.value,
			text,
			utf16Offsets,
			widths,
			wrap,
			wrapContinuation,
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
	const hasSharedBacking = isSharedArrayBuffer(bytes.buffer);
	let stableBytes = bytes;
	if (hasSharedBacking) {
		const copied = copyBytesToFixedArrayBuffer(
			bytes,
			PROTOCOL_LIMITS.maxRowWindowBytes,
			'Terminal row window',
		);
		if (!copied.ok) return copied;
		stableBytes = copied.value;
	} else if (bytes.byteLength > PROTOCOL_LIMITS.maxRowWindowBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			'Terminal row window exceeds its limit',
		);
	}
	const header = decodeHeader(stableBytes);
	if (!header.ok) return header;
	const body = stableBytes.subarray(TERMINAL_ROW_WINDOW_HEADER_BYTES);
	return header.value.detail === TerminalRowWindowDetail.render
		? decodeRenderWindow(header.value, body, decodeTerminalProjectionBuffer)
		: decodeTextWindow(header.value, body);
}

/**
 * Returns direct typed views for a capability-scoped local Host render window. Text windows and
 * all fixed routing metadata remain strictly decoded; external input must use decodeTerminalRowWindow.
 */
export function viewTerminalRowWindow(bytes: Uint8Array): ProtocolResult<DecodedTerminalRowWindow> {
	if (isSharedArrayBuffer(bytes.buffer)) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Trusted row-window views do not accept shared memory',
		);
	}
	const header = decodeHeader(bytes);
	if (!header.ok) return header;
	const body = bytes.subarray(TERMINAL_ROW_WINDOW_HEADER_BYTES);
	return header.value.detail === TerminalRowWindowDetail.render
		? decodeRenderWindow(header.value, body, viewTerminalProjectionBuffer)
		: decodeTextWindow(header.value, body);
}
