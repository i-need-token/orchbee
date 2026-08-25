import {
	checkedAdd,
	checkedMultiply,
	copyBytesToFixedArrayBuffer,
	isSharedArrayBuffer,
} from './binary';
import {
	IDENTIFIER_BYTE_LENGTH,
	identifierBytes,
	identifierText,
	parseAttachmentId,
	parseHostEpoch,
} from './identifiers';
import type { AttachmentId, HostEpoch } from './identifiers';
import {
	ActiveScreen,
	CellWidth,
	CursorShape,
	RowPromptClass,
	SemanticContent,
	TerminalColorEncoding,
	TerminalColorKind,
	TerminalFrameKind,
	TerminalFrameSchema,
	TerminalRowFlag,
	TerminalStyleFlag,
	UnderlineKind,
} from './frame';
import type {
	ActiveScreen as ActiveScreenValue,
	CellWidth as CellWidthValue,
	CursorShape as CursorShapeValue,
	RowPromptClass as RowPromptClassValue,
	SemanticContent as SemanticContentValue,
	TerminalColor,
	TerminalFrame,
	TerminalFrameKind as TerminalFrameKindValue,
	TerminalInteraction,
	TerminalStyle,
	UnderlineKind as UnderlineKindValue,
} from './frame';
import { PROTOCOL_LIMITS } from './limits';
import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolFailure, ProtocolResult } from './protocol-result';
import { isValidUtf8, utf8ByteLength } from './utf8';

export const TERMINAL_PROJECTION_BUFFER_HEADER_BYTES = 320;
export const TERMINAL_PROJECTION_BUFFER_MAGIC = new Uint8Array([0x4f, 0x42, 0x50, 0x42]);
// Keep this synchronized with Zig. Released changes (>= 1.0.0) also update
// docs/version-history.md in the same change.
export const TERMINAL_PROJECTION_BUFFER_FORMAT = { major: 0, minor: 0, patch: 2 } as const;

export const TerminalProjectionSection = {
	backgroundColors: 8,
	foregroundColors: 7,
	graphemeLengths: 4,
	graphemeOffsets: 3,
	hyperlinkLengths: 6,
	hyperlinkOffsets: 5,
	protectedCells: 14,
	rowFlags: 1,
	rowIndices: 0,
	rowPromptClasses: 2,
	semanticContents: 13,
	stringBytes: 15,
	styleFlags: 10,
	underlineColors: 9,
	underlineKinds: 11,
	widths: 12,
} as const;

const DIRECTORY_OFFSET = 160;
const DIRECTORY_ENTRY_BYTES = 8;
const DIRECTORY_ENTRY_COUNT = 16;
const MAX_UINT16 = 0xffff;
const MAX_UINT32 = 0xffff_ffff;
const MAX_UINT64 = 0xffff_ffff_ffff_ffffn;
const COLOR_RGB_MASK = TerminalColorEncoding.rgbMask;
const CURSOR_FLAGS_MASK = 0b111;
const INTERACTION_FLAGS_MASK = 0x0003_ffff;
const ROW_FLAGS_MASK = TerminalRowFlag.wrap | TerminalRowFlag.wrapContinuation;
const STYLE_FLAGS_MASK = 0x00ff;
const SECTION_ELEMENT_BYTES = [2, 1, 1, 4, 2, 4, 2, 4, 4, 4, 2, 1, 1, 1, 1, 1] as const;
const ProjectionStringRangeStatus = {
	invalidBoundary: 4,
	invalidNull: 1,
	invalidRange: 3,
	limitExceeded: 2,
	valid: 0,
} as const;
const textDecoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
const textEncoder = new TextEncoder();

interface ProjectionLayout {
	readonly byteLengths: readonly number[];
	readonly offsets: readonly number[];
	readonly totalLength: number;
}

interface EncodedProjectionString {
	readonly length: number;
	readonly offset: number;
}

interface CanonicalProjectionHyperlink {
	readonly bytes: Uint8Array;
	readonly firstCellOffset: number;
	readonly range: EncodedProjectionString;
}

interface PreparedProjectionHyperlink {
	readonly byteValueKey: string;
	readonly canonical: CanonicalProjectionHyperlink;
}

function projectionByteValueKey(bytes: Uint8Array): string {
	const chunks: string[] = [];
	for (let offset = 0; offset < bytes.byteLength; offset += 4_096) {
		chunks.push(String.fromCharCode(...bytes.subarray(offset, offset + 4_096)));
	}
	return chunks.join('');
}

export interface DecodedTerminalProjectionCells {
	readonly backgroundColors: Uint32Array;
	readonly foregroundColors: Uint32Array;
	readonly graphemeLengths: Uint16Array;
	readonly graphemeOffsets: Uint32Array;
	readonly hyperlinkLengths: Uint16Array;
	readonly hyperlinkOffsets: Uint32Array;
	readonly protected: Uint8Array;
	readonly semanticContents: Uint8Array;
	readonly styleFlags: Uint16Array;
	readonly underlineColors: Uint32Array;
	readonly underlineKinds: Uint8Array;
	readonly widths: Uint8Array;
}

export interface DecodedTerminalProjectionRows {
	readonly flags: Uint8Array;
	readonly indices: Uint16Array;
	readonly promptClasses: Uint8Array;
}

export interface DecodedTerminalProjectionBuffer {
	readonly activeScreen: ActiveScreenValue;
	readonly attachmentId: AttachmentId;
	readonly baseRevision: bigint | null;
	readonly bytes: Uint8Array;
	readonly cellPixelHeight: number;
	readonly cellPixelWidth: number;
	readonly cells: DecodedTerminalProjectionCells;
	readonly columns: number;
	readonly currentDirectory: string | null;
	readonly cursor: {
		readonly blinking: boolean;
		readonly column: number;
		readonly row: number;
		readonly shape: CursorShapeValue;
		readonly visible: boolean;
		readonly wideTail: boolean;
	};
	readonly defaultColors: {
		readonly backgroundRgb: number;
		readonly cursorRgb: number | null;
		readonly foregroundRgb: number;
	};
	readonly generation: bigint;
	readonly hostEpoch: HostEpoch;
	readonly ignoredOptionalFlags: number;
	readonly interaction: TerminalInteraction;
	readonly kind: TerminalFrameKindValue;
	readonly revision: bigint;
	readonly rowData: DecodedTerminalProjectionRows;
	readonly rows: number;
	readonly scrollbackRows: bigint;
	readonly sequence: bigint;
	readonly stringBytes: Uint8Array;
	readonly themeRevision: bigint;
	readonly title: string | null;
}

export interface TerminalProjectionFrameMaterialization {
	readonly attachmentId: AttachmentId;
	readonly revealCursor: boolean;
	readonly rowCount: number;
	readonly scrollbackRows: bigint;
	readonly sequence: bigint;
	readonly sourceStartRow: bigint;
}

interface PreparedProjectionMaterializationString {
	decoded: ProtocolResult<string | null> | null;
	readonly length: number;
	readonly offset: number;
}

interface ProjectionMaterializationStringCache {
	logicalByteLength: number;
	readonly rangesBySource: WeakMap<
		Uint8Array,
		Map<number, PreparedProjectionMaterializationString>
	>;
}

interface PreparedProjectionMaterialization {
	readonly indexedSources: readonly (DecodedTerminalProjectionBuffer | undefined)[];
	readonly stringCache: ProjectionMaterializationStringCache;
}

function align(value: number, alignment: number): ProtocolResult<number> {
	const remainder = value % alignment;
	return remainder === 0 ? protocolSuccess(value) : checkedAdd(value, alignment - remainder);
}

function calculateLayout(
	rowCount: number,
	cellCount: number,
	stringByteLength: number,
	maximumBytes = PROTOCOL_LIMITS.maxFrameBytes,
): ProtocolResult<ProjectionLayout> {
	const offsets: number[] = [];
	const byteLengths: number[] = [];
	let nextOffset = TERMINAL_PROJECTION_BUFFER_HEADER_BYTES;
	for (let section = 0; section < DIRECTORY_ENTRY_COUNT; section += 1) {
		const elementBytes = SECTION_ELEMENT_BYTES[section];
		if (elementBytes === undefined) {
			return protocolFailure(ProtocolErrorCode.invalidField, 'Projection section is missing');
		}
		let count = cellCount;
		if (section <= TerminalProjectionSection.rowPromptClasses) count = rowCount;
		if (section === TerminalProjectionSection.stringBytes) count = stringByteLength;
		const aligned = align(nextOffset, elementBytes);
		if (!aligned.ok) return aligned;
		const byteLength = checkedMultiply(count, elementBytes);
		if (!byteLength.ok) return byteLength;
		offsets.push(aligned.value);
		byteLengths.push(byteLength.value);
		const end = checkedAdd(aligned.value, byteLength.value);
		if (!end.ok) return end;
		nextOffset = end.value;
	}
	if (nextOffset > maximumBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			'Projection buffer exceeds its byte limit',
		);
	}
	return protocolSuccess({ byteLengths, offsets, totalLength: nextOffset });
}

function frameKindCode(kind: TerminalFrameKindValue): number {
	return kind === TerminalFrameKind.snapshot ? 1 : 2;
}

function frameKindFromCode(code: number): ProtocolResult<TerminalFrameKindValue> {
	if (code === 1) return protocolSuccess(TerminalFrameKind.snapshot);
	if (code === 2) return protocolSuccess(TerminalFrameKind.delta);
	return protocolFailure(ProtocolErrorCode.invalidField, `Unknown projection kind ${code}`);
}

function cursorShapeCode(shape: CursorShapeValue): number {
	if (shape === CursorShape.block) return 1;
	if (shape === CursorShape.bar) return 2;
	return 3;
}

function cursorShapeFromCode(code: number): ProtocolResult<CursorShapeValue> {
	if (code === 1) return protocolSuccess(CursorShape.block);
	if (code === 2) return protocolSuccess(CursorShape.bar);
	if (code === 3) return protocolSuccess(CursorShape.underline);
	return protocolFailure(ProtocolErrorCode.invalidField, `Unknown projection cursor shape ${code}`);
}

function screenCode(screen: ActiveScreenValue): number {
	return screen === ActiveScreen.primary ? 1 : 2;
}

function screenFromCode(code: number): ProtocolResult<ActiveScreenValue> {
	if (code === 1) return protocolSuccess(ActiveScreen.primary);
	if (code === 2) return protocolSuccess(ActiveScreen.alternate);
	return protocolFailure(ProtocolErrorCode.invalidField, `Unknown projection screen ${code}`);
}

function colorCode(color: TerminalColor): number {
	if (color.kind === TerminalColorKind.default) return TerminalColorEncoding.default;
	if (color.kind === TerminalColorKind.none) return TerminalColorEncoding.none;
	return color.rgb;
}

function styleFlags(style: TerminalStyle): number {
	return (
		(style.bold ? TerminalStyleFlag.bold : 0) |
		(style.italic ? TerminalStyleFlag.italic : 0) |
		(style.faint ? TerminalStyleFlag.faint : 0) |
		(style.blink ? TerminalStyleFlag.blink : 0) |
		(style.inverse ? TerminalStyleFlag.inverse : 0) |
		(style.invisible ? TerminalStyleFlag.invisible : 0) |
		(style.strikethrough ? TerminalStyleFlag.strikethrough : 0) |
		(style.overline ? TerminalStyleFlag.overline : 0)
	);
}

function underlineKindCode(kind: TerminalStyle['underlineKind']): number {
	if (kind === UnderlineKind.none) return 0;
	if (kind === UnderlineKind.single) return 1;
	if (kind === UnderlineKind.double) return 2;
	if (kind === UnderlineKind.curly) return 3;
	if (kind === UnderlineKind.dotted) return 4;
	return 5;
}

function widthCode(width: string): number {
	if (width === CellWidth.narrow) return 0;
	if (width === CellWidth.wide) return 1;
	if (width === CellWidth.spacerTail) return 2;
	return 3;
}

function semanticCode(semantic: string): number {
	if (semantic === SemanticContent.output) return 0;
	if (semantic === SemanticContent.input) return 1;
	return 2;
}

function promptCode(prompt: string): number {
	if (prompt === RowPromptClass.none) return 0;
	if (prompt === RowPromptClass.prompt) return 1;
	return 2;
}

function interactionFlags(interaction: TerminalInteraction): number {
	return (
		(interaction.applicationCursorKeys ? 1 : 0) |
		(interaction.applicationKeypad ? 1 << 1 : 0) |
		(interaction.bracketedPaste ? 1 << 2 : 0) |
		(interaction.focusReporting ? 1 << 3 : 0) |
		(interaction.mouseX10 ? 1 << 4 : 0) |
		(interaction.mouseNormal ? 1 << 5 : 0) |
		(interaction.mouseButton ? 1 << 6 : 0) |
		(interaction.mouseAny ? 1 << 7 : 0) |
		(interaction.mouseUtf8 ? 1 << 8 : 0) |
		(interaction.mouseSgr ? 1 << 9 : 0) |
		(interaction.mouseUrxvt ? 1 << 10 : 0) |
		(interaction.mouseSgrPixels ? 1 << 11 : 0) |
		(interaction.modifyOtherKeys2 ? 1 << 12 : 0) |
		(interaction.kittyDisambiguateEscapeCodes ? 1 << 13 : 0) |
		(interaction.kittyReportEventTypes ? 1 << 14 : 0) |
		(interaction.kittyReportAlternateKeys ? 1 << 15 : 0) |
		(interaction.kittyReportAllKeysAsEscapeCodes ? 1 << 16 : 0) |
		(interaction.kittyReportAssociatedText ? 1 << 17 : 0)
	);
}

function interactionFromFlags(flags: number): ProtocolResult<TerminalInteraction> {
	if ((flags & ~INTERACTION_FLAGS_MASK) !== 0) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredFlag,
			'Projection interaction state has unknown required bits',
		);
	}
	const mouseModes = [4, 5, 6, 7].filter((bit) => (flags & (1 << bit)) !== 0).length;
	const mouseFormats = [8, 9, 10, 11].filter((bit) => (flags & (1 << bit)) !== 0).length;
	if (mouseModes > 1 || mouseFormats > 1) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Projection mouse modes conflict');
	}
	return protocolSuccess({
		applicationCursorKeys: (flags & 1) !== 0,
		applicationKeypad: (flags & (1 << 1)) !== 0,
		bracketedPaste: (flags & (1 << 2)) !== 0,
		focusReporting: (flags & (1 << 3)) !== 0,
		kittyDisambiguateEscapeCodes: (flags & (1 << 13)) !== 0,
		kittyReportAllKeysAsEscapeCodes: (flags & (1 << 16)) !== 0,
		kittyReportAlternateKeys: (flags & (1 << 15)) !== 0,
		kittyReportAssociatedText: (flags & (1 << 17)) !== 0,
		kittyReportEventTypes: (flags & (1 << 14)) !== 0,
		modifyOtherKeys2: (flags & (1 << 12)) !== 0,
		mouseAny: (flags & (1 << 7)) !== 0,
		mouseButton: (flags & (1 << 6)) !== 0,
		mouseNormal: (flags & (1 << 5)) !== 0,
		mouseSgr: (flags & (1 << 9)) !== 0,
		mouseSgrPixels: (flags & (1 << 11)) !== 0,
		mouseUtf8: (flags & (1 << 8)) !== 0,
		mouseUrxvt: (flags & (1 << 10)) !== 0,
		mouseX10: (flags & (1 << 4)) !== 0,
	});
}

function writeDirectory(view: DataView, layout: ProjectionLayout): void {
	for (let section = 0; section < DIRECTORY_ENTRY_COUNT; section += 1) {
		const entryOffset = DIRECTORY_OFFSET + section * DIRECTORY_ENTRY_BYTES;
		view.setUint32(entryOffset, layout.offsets[section] ?? 0, true);
		view.setUint32(entryOffset + 4, layout.byteLengths[section] ?? 0, true);
	}
}

function sectionView<Column extends Uint8Array | Uint16Array | Uint32Array>(
	bytes: Uint8Array,
	layout: ProjectionLayout,
	section: number,
	ColumnType: { new (buffer: ArrayBufferLike, byteOffset: number, length: number): Column },
): Column {
	const offset = layout.offsets[section] ?? 0;
	const byteLength = layout.byteLengths[section] ?? 0;
	return new ColumnType(
		bytes.buffer,
		bytes.byteOffset + offset,
		byteLength / (SECTION_ELEMENT_BYTES[section] ?? 1),
	);
}

export function encodeTerminalProjectionBuffer(value: unknown): ProtocolResult<Uint8Array> {
	return encodeTerminalProjectionBufferWithin(value, PROTOCOL_LIMITS.maxFrameBytes);
}

export function encodeTerminalProjectionBufferWithin(
	value: unknown,
	maximumBytes: number,
): ProtocolResult<Uint8Array> {
	if (
		!Number.isSafeInteger(maximumBytes) ||
		maximumBytes < TERMINAL_PROJECTION_BUFFER_HEADER_BYTES ||
		maximumBytes > PROTOCOL_LIMITS.maxFrameBytes
	) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Projection buffer byte budget is invalid',
		);
	}
	const parsed = TerminalFrameSchema.safeParse(value);
	if (!parsed.success) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`Projection buffer validation failed: ${parsed.error.issues[0]?.message ?? 'unknown field error'}`,
		);
	}
	const frame = parsed.data;
	if (frame.extensions.length !== 0) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Projection buffer 0.0.2 has no extension section',
		);
	}
	const cellCount = frame.rowData.length * frame.columns;
	const fixedLayout = calculateLayout(frame.rowData.length, cellCount, 0, maximumBytes);
	if (!fixedLayout.ok) return fixedLayout;
	const maximumStringBytes = maximumBytes - fixedLayout.value.totalLength;
	let stringByteLength = 0;
	const accountByteLength = (byteLength: number): boolean => {
		if (byteLength > maximumStringBytes - stringByteLength) return false;
		stringByteLength += byteLength;
		return true;
	};
	const accountString = (text: string | null): boolean => {
		if (text === null) return true;
		return accountByteLength(utf8ByteLength(text));
	};
	if (!accountString(frame.title) || !accountString(frame.currentDirectory)) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			'Projection string section exceeds its byte limit',
		);
	}
	const cellHyperlinks: Array<CanonicalProjectionHyperlink | null> = [];
	const preparedHyperlinksByStyle = new Map<number, PreparedProjectionHyperlink | null>();
	const hyperlinksByByteValue = new Map<string, CanonicalProjectionHyperlink>();
	for (const row of frame.rowData) {
		for (const cell of row.cells) {
			const style = frame.styles[cell.style];
			if (style === undefined) {
				return protocolFailure(ProtocolErrorCode.invalidField, 'Projection cell style is missing');
			}
			if (!accountString(cell.grapheme)) {
				return protocolFailure(
					ProtocolErrorCode.limitExceeded,
					'Projection string section exceeds its byte limit',
				);
			}
			let preparedHyperlink = preparedHyperlinksByStyle.get(cell.style);
			if (!preparedHyperlinksByStyle.has(cell.style)) {
				preparedHyperlink = null;
				if (style.hyperlink !== null) {
					const hyperlinkBytes = textEncoder.encode(style.hyperlink);
					const byteValueKey = projectionByteValueKey(hyperlinkBytes);
					let canonical = hyperlinksByByteValue.get(byteValueKey);
					if (canonical === undefined) {
						const offset = stringByteLength;
						if (!accountByteLength(hyperlinkBytes.byteLength)) {
							return protocolFailure(
								ProtocolErrorCode.limitExceeded,
								'Projection string section exceeds its byte limit',
							);
						}
						canonical = {
							bytes: hyperlinkBytes,
							firstCellOffset: cellHyperlinks.length,
							range: { length: hyperlinkBytes.byteLength, offset },
						};
						hyperlinksByByteValue.set(byteValueKey, canonical);
					}
					preparedHyperlink = { byteValueKey, canonical };
				}
				preparedHyperlinksByStyle.set(cell.style, preparedHyperlink);
			}
			if (preparedHyperlink === null || preparedHyperlink === undefined) {
				cellHyperlinks.push(null);
				continue;
			}
			cellHyperlinks.push(preparedHyperlink.canonical);
		}
	}
	const layout = calculateLayout(frame.rowData.length, cellCount, stringByteLength, maximumBytes);
	if (!layout.ok) return layout;
	const bytes = new Uint8Array(layout.value.totalLength);
	const view = new DataView(bytes.buffer);
	const rowIndices = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.rowIndices,
		Uint16Array,
	);
	const rowFlags = sectionView(bytes, layout.value, TerminalProjectionSection.rowFlags, Uint8Array);
	const promptClasses = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.rowPromptClasses,
		Uint8Array,
	);
	const graphemeOffsets = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.graphemeOffsets,
		Uint32Array,
	);
	const graphemeLengths = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.graphemeLengths,
		Uint16Array,
	);
	const hyperlinkOffsets = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.hyperlinkOffsets,
		Uint32Array,
	);
	const hyperlinkLengths = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.hyperlinkLengths,
		Uint16Array,
	);
	const foregroundColors = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.foregroundColors,
		Uint32Array,
	);
	const backgroundColors = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.backgroundColors,
		Uint32Array,
	);
	const underlineColors = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.underlineColors,
		Uint32Array,
	);
	const encodedStyleFlags = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.styleFlags,
		Uint16Array,
	);
	const underlineKinds = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.underlineKinds,
		Uint8Array,
	);
	const widths = sectionView(bytes, layout.value, TerminalProjectionSection.widths, Uint8Array);
	const semanticContents = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.semanticContents,
		Uint8Array,
	);
	const protectedCells = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.protectedCells,
		Uint8Array,
	);
	const stringBytes = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.stringBytes,
		Uint8Array,
	);
	let stringOffset = 0;
	const appendString = (text: string | null): EncodedProjectionString | null => {
		if (text === null) return { length: 0, offset: MAX_UINT32 };
		const offset = stringOffset;
		const expectedLength = utf8ByteLength(text);
		const encoded = textEncoder.encodeInto(text, stringBytes.subarray(offset));
		if (encoded.read !== text.length || encoded.written !== expectedLength) return null;
		stringOffset += encoded.written;
		return { length: encoded.written, offset };
	};
	const title = appendString(frame.title);
	const currentDirectory = appendString(frame.currentDirectory);
	if (title === null || currentDirectory === null) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Projection metadata string encoding is inconsistent',
		);
	}
	for (const [rowOffset, row] of frame.rowData.entries()) {
		rowIndices[rowOffset] = row.index;
		rowFlags[rowOffset] =
			(row.wrap ? TerminalRowFlag.wrap : 0) |
			(row.wrapContinuation ? TerminalRowFlag.wrapContinuation : 0);
		promptClasses[rowOffset] = promptCode(row.promptClass);
		for (const [column, cell] of row.cells.entries()) {
			const cellOffset = rowOffset * frame.columns + column;
			const style = frame.styles[cell.style];
			if (style === undefined) {
				return protocolFailure(ProtocolErrorCode.invalidField, 'Projection cell style is missing');
			}
			const grapheme = appendString(cell.grapheme);
			const preparedHyperlink = cellHyperlinks[cellOffset];
			if (grapheme === null || preparedHyperlink === undefined) {
				return protocolFailure(
					ProtocolErrorCode.invalidField,
					'Projection cell string encoding is inconsistent',
				);
			}
			let hyperlink: EncodedProjectionString;
			if (preparedHyperlink === null) {
				hyperlink = { length: 0, offset: MAX_UINT32 };
			} else {
				if (preparedHyperlink.firstCellOffset === cellOffset) {
					if (stringOffset !== preparedHyperlink.range.offset) {
						return protocolFailure(
							ProtocolErrorCode.invalidField,
							'Projection hyperlink encoding is inconsistent',
						);
					}
					stringBytes.set(preparedHyperlink.bytes, stringOffset);
					stringOffset += preparedHyperlink.bytes.byteLength;
				}
				hyperlink = preparedHyperlink.range;
			}
			graphemeOffsets[cellOffset] = grapheme.offset;
			graphemeLengths[cellOffset] = grapheme.length;
			hyperlinkOffsets[cellOffset] = hyperlink.offset;
			hyperlinkLengths[cellOffset] = hyperlink.length;
			foregroundColors[cellOffset] = colorCode(style.foreground);
			backgroundColors[cellOffset] = colorCode(style.background);
			underlineColors[cellOffset] = colorCode(style.underline);
			encodedStyleFlags[cellOffset] = styleFlags(style);
			underlineKinds[cellOffset] = underlineKindCode(style.underlineKind);
			widths[cellOffset] = widthCode(cell.width);
			semanticContents[cellOffset] = semanticCode(cell.semanticContent);
			protectedCells[cellOffset] = cell.protected ? 1 : 0;
		}
	}
	if (stringOffset !== stringByteLength) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Projection string encoding is inconsistent',
		);
	}
	bytes.set(TERMINAL_PROJECTION_BUFFER_MAGIC, 0);
	view.setUint8(4, TERMINAL_PROJECTION_BUFFER_FORMAT.major);
	view.setUint8(5, TERMINAL_PROJECTION_BUFFER_FORMAT.minor);
	view.setUint8(6, frameKindCode(frame.kind));
	view.setUint32(8, bytes.byteLength, true);
	view.setUint16(12, TERMINAL_PROJECTION_BUFFER_HEADER_BYTES, true);
	view.setUint16(14, frame.columns, true);
	view.setUint16(16, frame.rows, true);
	view.setUint16(18, frame.rowData.length, true);
	view.setUint32(20, cellCount, true);
	bytes.set(identifierBytes(frame.hostEpoch), 24);
	bytes.set(identifierBytes(frame.attachmentId), 40);
	view.setBigUint64(56, frame.sequence, true);
	view.setBigUint64(64, frame.baseRevision ?? MAX_UINT64, true);
	view.setBigUint64(72, frame.revision, true);
	view.setBigUint64(80, frame.generation, true);
	view.setBigUint64(88, frame.scrollbackRows, true);
	view.setUint16(96, frame.cursor.column, true);
	view.setUint16(98, frame.cursor.row, true);
	view.setUint16(100, frame.cellPixelWidth, true);
	view.setUint16(102, frame.cellPixelHeight, true);
	view.setUint8(104, cursorShapeCode(frame.cursor.shape));
	view.setUint8(
		105,
		(frame.cursor.visible ? 1 : 0) |
			(frame.cursor.blinking ? 1 << 1 : 0) |
			(frame.cursor.wideTail ? 1 << 2 : 0),
	);
	view.setUint8(106, screenCode(frame.activeScreen));
	view.setUint32(108, interactionFlags(frame.interaction), true);
	view.setUint32(112, frame.defaultColors.foregroundRgb, true);
	view.setUint32(116, frame.defaultColors.backgroundRgb, true);
	view.setUint32(120, frame.defaultColors.cursorRgb ?? TerminalColorEncoding.default, true);
	view.setUint32(124, title.offset, true);
	view.setUint32(128, title.length, true);
	view.setUint32(132, currentDirectory.offset, true);
	view.setUint32(136, currentDirectory.length, true);
	view.setUint32(140, stringByteLength, true);
	view.setUint8(144, TERMINAL_PROJECTION_BUFFER_FORMAT.patch);
	view.setBigUint64(145, frame.themeRevision, true);
	writeDirectory(view, layout.value);
	return protocolSuccess(bytes);
}

function allZero(bytes: Uint8Array): boolean {
	return bytes.every((value) => value === 0);
}

function decodeMetadataString(
	stringBytes: Uint8Array,
	offset: number,
	length: number,
	field: string,
): ProtocolResult<string | null> {
	if (offset === MAX_UINT32) {
		return length === 0
			? protocolSuccess(null)
			: protocolFailure(ProtocolErrorCode.invalidField, `${field} null range has bytes`);
	}
	if (length > PROTOCOL_LIMITS.maxStringBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, `${field} exceeds its byte limit`);
	}
	if (offset > stringBytes.byteLength || length > stringBytes.byteLength - offset) {
		return protocolFailure(ProtocolErrorCode.invalidField, `${field} string range is invalid`);
	}
	try {
		return protocolSuccess(textDecoder.decode(stringBytes.subarray(offset, offset + length)));
	} catch {
		return protocolFailure(ProtocolErrorCode.invalidUtf8, `${field} is not valid UTF-8`);
	}
}

/**
 * Checks the fixed local data-plane structure and returns zero-repack typed views. Per-cell values
 * are trusted Host output and are intentionally not rescanned by this capability-scoped boundary.
 */
export function viewTerminalProjectionBuffer(
	bytes: Uint8Array,
): ProtocolResult<DecodedTerminalProjectionBuffer> {
	if (isSharedArrayBuffer(bytes.buffer)) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Trusted projection views do not accept shared memory',
		);
	}
	if (bytes.byteLength < TERMINAL_PROJECTION_BUFFER_HEADER_BYTES) {
		return protocolFailure(ProtocolErrorCode.truncated, 'Projection buffer header is truncated');
	}
	if (bytes.byteLength > PROTOCOL_LIMITS.maxFrameBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'Projection buffer exceeds its limit');
	}
	if (!TERMINAL_PROJECTION_BUFFER_MAGIC.every((value, index) => bytes[index] === value)) {
		return protocolFailure(ProtocolErrorCode.invalidMagic, 'Projection buffer magic is not OBPB');
	}
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const formatMajor = view.getUint8(4);
	const formatMinor = view.getUint8(5);
	const kindCode = view.getUint8(6);
	const flags = view.getUint8(7);
	const totalLength = view.getUint32(8, true);
	const headerLength = view.getUint16(12, true);
	const columns = view.getUint16(14, true);
	const rows = view.getUint16(16, true);
	const rowCount = view.getUint16(18, true);
	const cellCount = view.getUint32(20, true);
	const sequence = view.getBigUint64(56, true);
	const encodedBaseRevision = view.getBigUint64(64, true);
	const revision = view.getBigUint64(72, true);
	const generation = view.getBigUint64(80, true);
	const scrollbackRows = view.getBigUint64(88, true);
	const cursorColumn = view.getUint16(96, true);
	const cursorRow = view.getUint16(98, true);
	const cellPixelWidth = view.getUint16(100, true);
	const cellPixelHeight = view.getUint16(102, true);
	const cursorShapeValue = view.getUint8(104);
	const cursorFlags = view.getUint8(105);
	const screenValue = view.getUint8(106);
	const reservedScreen = view.getUint8(107);
	const encodedInteraction = view.getUint32(108, true);
	const foregroundRgb = view.getUint32(112, true);
	const backgroundRgb = view.getUint32(116, true);
	const encodedCursorRgb = view.getUint32(120, true);
	const titleOffset = view.getUint32(124, true);
	const titleLength = view.getUint32(128, true);
	const directoryOffset = view.getUint32(132, true);
	const directoryLength = view.getUint32(136, true);
	const stringByteLength = view.getUint32(140, true);
	const formatPatch = view.getUint8(144);
	const themeRevision = view.getBigUint64(145, true);
	if (
		formatMajor !== TERMINAL_PROJECTION_BUFFER_FORMAT.major ||
		formatMinor !== TERMINAL_PROJECTION_BUFFER_FORMAT.minor ||
		formatPatch !== TERMINAL_PROJECTION_BUFFER_FORMAT.patch ||
		headerLength !== TERMINAL_PROJECTION_BUFFER_HEADER_BYTES
	) {
		return protocolFailure(
			ProtocolErrorCode.unsupportedVersion,
			`Unsupported projection buffer ${formatMajor}.${formatMinor}.${formatPatch}`,
		);
	}
	if (totalLength !== bytes.byteLength) {
		return protocolFailure(ProtocolErrorCode.lengthMismatch, 'Projection buffer length disagrees');
	}
	if (
		(flags & 0x80) !== 0 ||
		reservedScreen !== 0 ||
		(cursorFlags & ~CURSOR_FLAGS_MASK) !== 0 ||
		!allZero(bytes.subarray(153, 160)) ||
		!allZero(bytes.subarray(288, 320))
	) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredFlag,
			'Projection buffer contains a reserved or required flag',
		);
	}
	const gridCellCount = checkedMultiply(columns, rows);
	if (!gridCellCount.ok) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Projection buffer shape is invalid');
	}
	if (
		columns === 0 ||
		columns > PROTOCOL_LIMITS.maxFrameColumns ||
		rows === 0 ||
		rows > PROTOCOL_LIMITS.maxFrameRows ||
		gridCellCount.value > PROTOCOL_LIMITS.maxFrameCells ||
		rowCount > rows ||
		cellCount !== rowCount * columns ||
		cellCount > PROTOCOL_LIMITS.maxFrameCells ||
		sequence === 0n ||
		generation === 0n ||
		cursorColumn >= columns ||
		cursorRow >= rows ||
		cellPixelWidth === 0 ||
		cellPixelHeight === 0 ||
		cellPixelWidth > PROTOCOL_LIMITS.maxCellPixelDimension ||
		cellPixelHeight > PROTOCOL_LIMITS.maxCellPixelDimension ||
		columns * cellPixelWidth > MAX_UINT16 ||
		rows * cellPixelHeight > MAX_UINT16
	) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Projection buffer shape is invalid');
	}
	const kind = frameKindFromCode(kindCode);
	if (!kind.ok) return kind;
	const baseRevision = encodedBaseRevision === MAX_UINT64 ? null : encodedBaseRevision;
	if (
		(kind.value === TerminalFrameKind.snapshot && (baseRevision !== null || rowCount !== rows)) ||
		(kind.value === TerminalFrameKind.delta && (baseRevision === null || baseRevision >= revision))
	) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Projection kind and base disagree');
	}
	if (
		foregroundRgb > COLOR_RGB_MASK ||
		backgroundRgb > COLOR_RGB_MASK ||
		(encodedCursorRgb !== TerminalColorEncoding.default && encodedCursorRgb > COLOR_RGB_MASK)
	) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Projection default color is invalid');
	}
	const layout = calculateLayout(rowCount, cellCount, stringByteLength);
	if (!layout.ok) return layout;
	if (layout.value.totalLength !== bytes.byteLength) {
		return protocolFailure(
			ProtocolErrorCode.lengthMismatch,
			'Projection sections are not canonical',
		);
	}
	for (let section = 0; section < DIRECTORY_ENTRY_COUNT; section += 1) {
		const entryOffset = DIRECTORY_OFFSET + section * DIRECTORY_ENTRY_BYTES;
		const sectionOffset = layout.value.offsets[section] ?? 0;
		const priorSectionEnd =
			section === 0
				? TERMINAL_PROJECTION_BUFFER_HEADER_BYTES
				: (layout.value.offsets[section - 1] ?? 0) + (layout.value.byteLengths[section - 1] ?? 0);
		if (
			view.getUint32(entryOffset, true) !== sectionOffset ||
			view.getUint32(entryOffset + 4, true) !== layout.value.byteLengths[section]
		) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				'Projection directory is not canonical',
			);
		}
		if (!allZero(bytes.subarray(priorSectionEnd, sectionOffset))) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				'Projection alignment padding is not zero',
			);
		}
		const absoluteOffset = bytes.byteOffset + sectionOffset;
		if (absoluteOffset % (SECTION_ELEMENT_BYTES[section] ?? 1) !== 0) {
			return protocolFailure(ProtocolErrorCode.invalidField, 'Projection view is misaligned');
		}
	}
	const hostEpochText = identifierText(bytes.subarray(24, 24 + IDENTIFIER_BYTE_LENGTH));
	if (!hostEpochText.ok) return hostEpochText;
	const hostEpoch = parseHostEpoch(hostEpochText.value);
	if (!hostEpoch.ok) return hostEpoch;
	const attachmentText = identifierText(bytes.subarray(40, 40 + IDENTIFIER_BYTE_LENGTH));
	if (!attachmentText.ok) return attachmentText;
	const attachmentId = parseAttachmentId(attachmentText.value);
	if (!attachmentId.ok) return attachmentId;
	const shape = cursorShapeFromCode(cursorShapeValue);
	if (!shape.ok) return shape;
	const activeScreen = screenFromCode(screenValue);
	if (!activeScreen.ok) return activeScreen;
	const interaction = interactionFromFlags(encodedInteraction);
	if (!interaction.ok) return interaction;
	const stringBytes = sectionView(
		bytes,
		layout.value,
		TerminalProjectionSection.stringBytes,
		Uint8Array,
	);
	const title = decodeMetadataString(stringBytes, titleOffset, titleLength, 'Projection title');
	if (!title.ok) return title;
	const currentDirectory = decodeMetadataString(
		stringBytes,
		directoryOffset,
		directoryLength,
		'Projection current directory',
	);
	if (!currentDirectory.ok) return currentDirectory;
	return protocolSuccess({
		activeScreen: activeScreen.value,
		attachmentId: attachmentId.value,
		baseRevision,
		bytes,
		cellPixelHeight,
		cellPixelWidth,
		cells: {
			backgroundColors: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.backgroundColors,
				Uint32Array,
			),
			foregroundColors: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.foregroundColors,
				Uint32Array,
			),
			graphemeLengths: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.graphemeLengths,
				Uint16Array,
			),
			graphemeOffsets: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.graphemeOffsets,
				Uint32Array,
			),
			hyperlinkLengths: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.hyperlinkLengths,
				Uint16Array,
			),
			hyperlinkOffsets: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.hyperlinkOffsets,
				Uint32Array,
			),
			protected: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.protectedCells,
				Uint8Array,
			),
			semanticContents: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.semanticContents,
				Uint8Array,
			),
			styleFlags: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.styleFlags,
				Uint16Array,
			),
			underlineColors: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.underlineColors,
				Uint32Array,
			),
			underlineKinds: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.underlineKinds,
				Uint8Array,
			),
			widths: sectionView(bytes, layout.value, TerminalProjectionSection.widths, Uint8Array),
		},
		columns,
		currentDirectory: currentDirectory.value,
		cursor: {
			blinking: (cursorFlags & (1 << 1)) !== 0,
			column: cursorColumn,
			row: cursorRow,
			shape: shape.value,
			visible: (cursorFlags & 1) !== 0,
			wideTail: (cursorFlags & (1 << 2)) !== 0,
		},
		defaultColors: {
			backgroundRgb,
			cursorRgb: encodedCursorRgb === TerminalColorEncoding.default ? null : encodedCursorRgb,
			foregroundRgb,
		},
		generation,
		hostEpoch: hostEpoch.value,
		ignoredOptionalFlags: flags & 0x7f,
		interaction: interaction.value,
		kind: kind.value,
		revision,
		rowData: {
			flags: sectionView(bytes, layout.value, TerminalProjectionSection.rowFlags, Uint8Array),
			indices: sectionView(bytes, layout.value, TerminalProjectionSection.rowIndices, Uint16Array),
			promptClasses: sectionView(
				bytes,
				layout.value,
				TerminalProjectionSection.rowPromptClasses,
				Uint8Array,
			),
		},
		rows,
		scrollbackRows,
		sequence,
		stringBytes,
		themeRevision,
		title: title.value,
	});
}

function validProjectionColor(value: number): boolean {
	return (
		value <= COLOR_RGB_MASK ||
		value === TerminalColorEncoding.default ||
		value === TerminalColorEncoding.none
	);
}

type ProjectionStringRangeStatusValue =
	(typeof ProjectionStringRangeStatus)[keyof typeof ProjectionStringRangeStatus];

function projectionStringRangeStatus(
	stringBytes: Uint8Array,
	offset: number,
	length: number,
	nullable: boolean,
): ProjectionStringRangeStatusValue {
	if (offset === MAX_UINT32) {
		return nullable && length === 0
			? ProjectionStringRangeStatus.valid
			: ProjectionStringRangeStatus.invalidNull;
	}
	if (length > PROTOCOL_LIMITS.maxStringBytes) {
		return ProjectionStringRangeStatus.limitExceeded;
	}
	if (offset > stringBytes.byteLength || length > stringBytes.byteLength - offset) {
		return ProjectionStringRangeStatus.invalidRange;
	}
	const end = offset + length;
	if (!isUtf8Boundary(stringBytes, offset) || !isUtf8Boundary(stringBytes, end)) {
		return ProjectionStringRangeStatus.invalidBoundary;
	}
	return ProjectionStringRangeStatus.valid;
}

function projectionStringRangeFailure(
	status: ProjectionStringRangeStatusValue,
	field: string,
): ProtocolFailure {
	if (status === ProjectionStringRangeStatus.invalidNull) {
		return protocolFailure(ProtocolErrorCode.invalidField, `${field} has an invalid null range`);
	}
	if (status === ProjectionStringRangeStatus.limitExceeded) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, `${field} exceeds its byte limit`);
	}
	if (status === ProjectionStringRangeStatus.invalidRange) {
		return protocolFailure(ProtocolErrorCode.invalidField, `${field} range is invalid`);
	}
	if (status === ProjectionStringRangeStatus.invalidBoundary) {
		return protocolFailure(ProtocolErrorCode.invalidUtf8, `${field} splits a UTF-8 scalar`);
	}
	return protocolFailure(ProtocolErrorCode.invalidField, `${field} validation failed`);
}

function projectionStringRangeKey(offset: number, length: number): number {
	// The complete u32/u16 tuple fits exactly below Number.MAX_SAFE_INTEGER.
	return offset * (MAX_UINT16 + 1) + length;
}

function preflightProjectionStringRange(
	cache: ProjectionMaterializationStringCache,
	source: DecodedTerminalProjectionBuffer,
	offset: number | undefined,
	length: number | undefined,
	nullable: boolean,
	field: string,
): ProtocolResult<undefined> {
	if (offset === undefined || length === undefined) {
		return protocolFailure(ProtocolErrorCode.invalidField, `${field} range is missing`);
	}
	let sourceRanges = cache.rangesBySource.get(source.stringBytes);
	if (sourceRanges === undefined) {
		sourceRanges = new Map();
		cache.rangesBySource.set(source.stringBytes, sourceRanges);
	}
	const key = projectionStringRangeKey(offset, length);
	if (sourceRanges.has(key) && (offset !== MAX_UINT32 || nullable)) {
		return protocolSuccess(undefined);
	}
	const status = projectionStringRangeStatus(source.stringBytes, offset, length, nullable);
	if (status !== ProjectionStringRangeStatus.valid) {
		return projectionStringRangeFailure(status, field);
	}
	const logicalByteLength = checkedAdd(cache.logicalByteLength, length);
	if (!logicalByteLength.ok) return logicalByteLength;
	if (logicalByteLength.value > PROTOCOL_LIMITS.maxFrameBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			'Projection materialization string ranges exceed the frame byte limit',
		);
	}
	sourceRanges.set(key, { decoded: null, length, offset });
	cache.logicalByteLength = logicalByteLength.value;
	return protocolSuccess(undefined);
}

function isUtf8Boundary(bytes: Uint8Array, offset: number): boolean {
	if (offset === 0 || offset === bytes.byteLength) return true;
	const value = bytes[offset];
	return value !== undefined && (value & 0xc0) !== 0x80;
}

function validateProjectionStringSection(bytes: Uint8Array): ProtocolResult<undefined> {
	if (!isValidUtf8(bytes)) {
		return protocolFailure(
			ProtocolErrorCode.invalidUtf8,
			'Projection string section is not valid UTF-8',
		);
	}
	return protocolSuccess(undefined);
}

function nextUnassignedProjectionRow(successors: number[], row: number): number {
	let root = row;
	let parent = successors[root];
	if (parent === undefined)
		throw new Error('Projection materialization source index is incomplete');
	while (parent !== root) {
		root = parent;
		parent = successors[root];
		if (parent === undefined) {
			throw new Error('Projection materialization source index is incomplete');
		}
	}
	let current = row;
	while (current !== root) {
		const next = successors[current];
		if (next === undefined) {
			throw new Error('Projection materialization source index is incomplete');
		}
		successors[current] = root;
		current = next;
	}
	return root;
}

function indexProjectionMaterializationSources(
	sources: readonly DecodedTerminalProjectionBuffer[],
	sourceStartRow: bigint,
	rowCount: number,
): readonly (DecodedTerminalProjectionBuffer | undefined)[] {
	const indexedSources: Array<DecodedTerminalProjectionBuffer | undefined> = Array.from(
		{ length: rowCount },
		() => undefined,
	);
	const successors = Array.from({ length: rowCount + 1 }, (_, index) => index);
	const requestedEndRow = sourceStartRow + BigInt(rowCount);
	let rowsRemaining = rowCount;
	for (const source of sources) {
		if (rowsRemaining === 0) break;
		const sourceEndRow = source.scrollbackRows + BigInt(source.rows);
		const overlapStartRow =
			source.scrollbackRows > sourceStartRow ? source.scrollbackRows : sourceStartRow;
		const overlapEndRow = sourceEndRow < requestedEndRow ? sourceEndRow : requestedEndRow;
		if (overlapStartRow >= overlapEndRow) continue;
		let outputRow = nextUnassignedProjectionRow(
			successors,
			Number(overlapStartRow - sourceStartRow),
		);
		const outputEndRow = Number(overlapEndRow - sourceStartRow);
		while (outputRow < outputEndRow) {
			indexedSources[outputRow] = source;
			rowsRemaining -= 1;
			const nextOutputRow = nextUnassignedProjectionRow(successors, outputRow + 1);
			successors[outputRow] = nextOutputRow;
			outputRow = nextOutputRow;
		}
	}
	return indexedSources;
}

function prepareProjectionMaterialization(
	metadata: DecodedTerminalProjectionBuffer,
	sources: readonly DecodedTerminalProjectionBuffer[],
	options: TerminalProjectionFrameMaterialization,
): ProtocolResult<PreparedProjectionMaterialization> {
	const indexedSources = indexProjectionMaterializationSources(
		sources,
		options.sourceStartRow,
		options.rowCount,
	);
	const stringCache: ProjectionMaterializationStringCache = {
		logicalByteLength: 0,
		rangesBySource: new WeakMap(),
	};
	for (let outputRow = 0; outputRow < options.rowCount; outputRow += 1) {
		const absoluteRow = options.sourceStartRow + BigInt(outputRow);
		const source = indexedSources[outputRow];
		if (source === undefined || !projectionMaterializationSourceMatches(metadata, source)) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				`Projection materialization is missing source row ${absoluteRow}`,
			);
		}
		const sourceRow = Number(absoluteRow - source.scrollbackRows);
		if (
			source.rowData.flags[sourceRow] === undefined ||
			source.rowData.promptClasses[sourceRow] === undefined
		) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				`Projection materialization source row ${absoluteRow} is incomplete`,
			);
		}
		for (let column = 0; column < metadata.columns; column += 1) {
			const cellOffset = sourceRow * metadata.columns + column;
			const grapheme = preflightProjectionStringRange(
				stringCache,
				source,
				source.cells.graphemeOffsets[cellOffset],
				source.cells.graphemeLengths[cellOffset],
				false,
				`Projection grapheme ${cellOffset}`,
			);
			if (!grapheme.ok) return grapheme;
			const hyperlink = preflightProjectionStringRange(
				stringCache,
				source,
				source.cells.hyperlinkOffsets[cellOffset],
				source.cells.hyperlinkLengths[cellOffset],
				true,
				`Projection hyperlink ${cellOffset}`,
			);
			if (!hyperlink.ok) return hyperlink;
		}
	}
	return protocolSuccess({ indexedSources, stringCache });
}

/**
 * Strictly validates an untrusted projection buffer before exposing its zero-repack typed views.
 * Local capability-scoped Host delivery may use {@link viewTerminalProjectionBuffer} instead.
 */
export function decodeTerminalProjectionBuffer(
	bytes: Uint8Array,
): ProtocolResult<DecodedTerminalProjectionBuffer> {
	const hasSharedBacking = isSharedArrayBuffer(bytes.buffer);
	let stableBytes = bytes;
	if (hasSharedBacking || bytes.byteOffset % 4 !== 0) {
		const copied = copyBytesToFixedArrayBuffer(
			bytes,
			PROTOCOL_LIMITS.maxFrameBytes,
			'Projection buffer',
		);
		if (!copied.ok) return copied;
		stableBytes = copied.value;
	} else if (bytes.byteLength > PROTOCOL_LIMITS.maxFrameBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'Projection buffer exceeds its limit');
	}
	const viewed = viewTerminalProjectionBuffer(stableBytes);
	if (!viewed.ok) return viewed;
	const projection = viewed.value;
	const validStringSection = validateProjectionStringSection(projection.stringBytes);
	if (!validStringSection.ok) return validStringSection;

	let previousRow = -1;
	let previousRowFlags = 0;
	let previousSpacerHead = false;
	for (let rowOffset = 0; rowOffset < projection.rowData.indices.length; rowOffset += 1) {
		const row = projection.rowData.indices[rowOffset] ?? MAX_UINT16;
		const rowFlags = projection.rowData.flags[rowOffset] ?? 0xff;
		const promptClass = projection.rowData.promptClasses[rowOffset] ?? 0xff;
		if ((rowFlags & ~ROW_FLAGS_MASK) !== 0) {
			return protocolFailure(
				ProtocolErrorCode.unknownRequiredFlag,
				`Projection row ${rowOffset} has unknown flags`,
			);
		}
		if (
			promptClass > 2 ||
			row >= projection.rows ||
			row <= previousRow ||
			(projection.kind === TerminalFrameKind.snapshot && row !== rowOffset)
		) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				`Projection row ${rowOffset} is invalid`,
			);
		}
		const followsPriorRow = previousRow >= 0 && row === previousRow + 1;
		if (
			followsPriorRow &&
			((previousRowFlags & TerminalRowFlag.wrap) !== 0) !==
				((rowFlags & TerminalRowFlag.wrapContinuation) !== 0)
		) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				'Adjacent projection row wrap flags disagree',
			);
		}

		let previousWidth = -1;
		let firstWidth = -1;
		let spacerHead = false;
		const firstCell = rowOffset * projection.columns;
		const endCell = firstCell + projection.columns;
		for (let cellOffset = firstCell; cellOffset < endCell; cellOffset += 1) {
			const width = projection.cells.widths[cellOffset] ?? 0xff;
			const semantic = projection.cells.semanticContents[cellOffset] ?? 0xff;
			const protectedCell = projection.cells.protected[cellOffset] ?? 0xff;
			const styleFlagsValue = projection.cells.styleFlags[cellOffset] ?? MAX_UINT16;
			const underlineKind = projection.cells.underlineKinds[cellOffset] ?? 0xff;
			if ((styleFlagsValue & ~STYLE_FLAGS_MASK) !== 0) {
				return protocolFailure(
					ProtocolErrorCode.unknownRequiredFlag,
					`Projection cell ${cellOffset} has unknown style flags`,
				);
			}
			if (
				width > 3 ||
				semantic > 2 ||
				protectedCell > 1 ||
				underlineKind > 5 ||
				!validProjectionColor(projection.cells.foregroundColors[cellOffset] ?? MAX_UINT32) ||
				!validProjectionColor(projection.cells.backgroundColors[cellOffset] ?? MAX_UINT32) ||
				!validProjectionColor(projection.cells.underlineColors[cellOffset] ?? MAX_UINT32) ||
				(width === 2 && previousWidth !== 1) ||
				(previousWidth === 1 && width !== 2) ||
				(width === 3 && (cellOffset !== endCell - 1 || (rowFlags & TerminalRowFlag.wrap) === 0))
			) {
				return protocolFailure(
					ProtocolErrorCode.invalidField,
					`Projection cell ${cellOffset} is invalid`,
				);
			}
			const graphemeStatus = projectionStringRangeStatus(
				projection.stringBytes,
				projection.cells.graphemeOffsets[cellOffset] ?? MAX_UINT32,
				projection.cells.graphemeLengths[cellOffset] ?? MAX_UINT16,
				false,
			);
			if (graphemeStatus !== ProjectionStringRangeStatus.valid) {
				return projectionStringRangeFailure(graphemeStatus, `Projection grapheme ${cellOffset}`);
			}
			const hyperlinkStatus = projectionStringRangeStatus(
				projection.stringBytes,
				projection.cells.hyperlinkOffsets[cellOffset] ?? MAX_UINT32,
				projection.cells.hyperlinkLengths[cellOffset] ?? MAX_UINT16,
				true,
			);
			if (hyperlinkStatus !== ProjectionStringRangeStatus.valid) {
				return projectionStringRangeFailure(hyperlinkStatus, `Projection hyperlink ${cellOffset}`);
			}
			if (cellOffset === firstCell) firstWidth = width;
			if (width === 3) spacerHead = true;
			previousWidth = width;
		}
		if (previousWidth === 1) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				`Projection row ${row} ends with a wide cell`,
			);
		}
		if (followsPriorRow && previousSpacerHead && firstWidth !== 1) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				'A projection row after spacer-head does not begin with a wide cell',
			);
		}
		previousRow = row;
		previousRowFlags = rowFlags;
		previousSpacerHead = spacerHead;
	}
	return viewed;
}

/**
 * Materializes a bounded snapshot from one or more already-decoded packed projections. This is a
 * local projection operation: it does not restore VT state or carry frame extensions.
 */
export function materializeTerminalProjectionFrame(
	metadata: DecodedTerminalProjectionBuffer,
	sources: readonly DecodedTerminalProjectionBuffer[],
	options: TerminalProjectionFrameMaterialization,
): ProtocolResult<TerminalFrame> {
	if (
		!Number.isSafeInteger(options.rowCount) ||
		options.rowCount < 1 ||
		options.rowCount > PROTOCOL_LIMITS.maxFrameRows ||
		options.rowCount * metadata.columns > PROTOCOL_LIMITS.maxFrameCells ||
		options.sequence <= 0n ||
		options.sourceStartRow < 0n ||
		options.scrollbackRows < 0n
	) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Projection materialization options are outside the protocol bounds',
		);
	}
	// Input-order assignment retains first-match precedence even for overlapping, unsorted sources.
	// Successor links skip rows already claimed by an earlier source, so each row is assigned once.
	const prepared = prepareProjectionMaterialization(metadata, sources, options);
	if (!prepared.ok) return prepared;
	const styles: TerminalFrame['styles'] = [];
	const styleIndices = new Map<string, number>();
	const rawStyleIndices = new WeakMap<DecodedTerminalProjectionBuffer, Map<string, number>>();
	const rowData: TerminalFrame['rowData'] = [];
	const { indexedSources, stringCache } = prepared.value;
	for (let outputRow = 0; outputRow < options.rowCount; outputRow += 1) {
		const absoluteRow = options.sourceStartRow + BigInt(outputRow);
		const source = indexedSources[outputRow];
		if (source === undefined || !projectionMaterializationSourceMatches(metadata, source)) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				`Projection materialization is missing source row ${absoluteRow}`,
			);
		}
		const sourceRow = Number(absoluteRow - source.scrollbackRows);
		const flags = source.rowData.flags[sourceRow];
		const promptClass = source.rowData.promptClasses[sourceRow];
		if (flags === undefined || promptClass === undefined) {
			return protocolFailure(
				ProtocolErrorCode.invalidField,
				`Projection materialization source row ${absoluteRow} is incomplete`,
			);
		}
		const cells: TerminalFrame['rowData'][number]['cells'] = [];
		for (let column = 0; column < metadata.columns; column += 1) {
			const cellOffset = sourceRow * metadata.columns + column;
			let sourceStyleIndices = rawStyleIndices.get(source);
			if (sourceStyleIndices === undefined) {
				sourceStyleIndices = new Map();
				rawStyleIndices.set(source, sourceStyleIndices);
			}
			const rawStyleKey = projectionStyleKey(source, cellOffset);
			let styleIndex = sourceStyleIndices.get(rawStyleKey);
			if (styleIndex === undefined) {
				const style = materializedStyle(stringCache, source, cellOffset);
				if (!style.ok) return style;
				const styleKey = JSON.stringify(style.value);
				styleIndex = styleIndices.get(styleKey);
				if (styleIndex === undefined) {
					if (styles.length >= PROTOCOL_LIMITS.maxFrameStyles) {
						return protocolFailure(
							ProtocolErrorCode.limitExceeded,
							'Projection materialization exceeds the style-table limit',
						);
					}
					styleIndex = styles.length;
					styles.push(style.value);
					styleIndices.set(styleKey, styleIndex);
				}
				sourceStyleIndices.set(rawStyleKey, styleIndex);
			}
			const grapheme = projectionString(
				stringCache,
				source,
				source.cells.graphemeOffsets[cellOffset],
				source.cells.graphemeLengths[cellOffset],
			);
			if (!grapheme.ok) return grapheme;
			const width = materializedWidth(source.cells.widths[cellOffset]);
			if (!width.ok) return width;
			const semanticContent = materializedSemantic(source.cells.semanticContents[cellOffset]);
			if (!semanticContent.ok) return semanticContent;
			const protectedCell = source.cells.protected[cellOffset];
			if (protectedCell !== 0 && protectedCell !== 1) {
				return protocolFailure(
					ProtocolErrorCode.invalidField,
					`Projection materialization cell ${cellOffset} is invalid`,
				);
			}
			cells.push({
				grapheme: grapheme.value ?? '',
				protected: protectedCell === 1,
				semanticContent: semanticContent.value,
				style: styleIndex,
				width: width.value,
			});
		}
		const prompt = materializedPromptClass(promptClass);
		if (!prompt.ok) return prompt;
		rowData.push({
			cells,
			index: outputRow,
			promptClass: prompt.value,
			wrap: (flags & TerminalRowFlag.wrap) !== 0,
			wrapContinuation: (flags & TerminalRowFlag.wrapContinuation) !== 0,
		});
	}
	const sourceCursorRow = metadata.scrollbackRows + BigInt(metadata.cursor.row);
	const cursorOffset = sourceCursorRow - options.sourceStartRow;
	const cursorVisible =
		options.revealCursor && cursorOffset >= 0n && cursorOffset < BigInt(options.rowCount);
	const frame = TerminalFrameSchema.safeParse({
		activeScreen: metadata.activeScreen,
		attachmentId: options.attachmentId,
		baseRevision: null,
		cellPixelHeight: metadata.cellPixelHeight,
		cellPixelWidth: metadata.cellPixelWidth,
		columns: metadata.columns,
		currentDirectory: metadata.currentDirectory,
		cursor: {
			...metadata.cursor,
			row: cursorVisible ? Number(cursorOffset) : 0,
			visible: cursorVisible && metadata.cursor.visible,
			wideTail: cursorVisible && metadata.cursor.wideTail,
		},
		defaultColors: metadata.defaultColors,
		extensions: [],
		generation: metadata.generation,
		hostEpoch: metadata.hostEpoch,
		interaction: metadata.interaction,
		kind: TerminalFrameKind.snapshot,
		revision: metadata.revision,
		rowData,
		rows: options.rowCount,
		scrollbackRows: options.scrollbackRows,
		sequence: options.sequence,
		styles,
		themeRevision: metadata.themeRevision,
		title: metadata.title,
	});
	return frame.success
		? protocolSuccess(frame.data)
		: protocolFailure(
				ProtocolErrorCode.invalidField,
				`Projection materialization failed: ${frame.error.issues[0]?.message ?? 'invalid frame'}`,
			);
}

function projectionMaterializationSourceMatches(
	metadata: DecodedTerminalProjectionBuffer,
	source: DecodedTerminalProjectionBuffer,
): boolean {
	return (
		source.kind === TerminalFrameKind.snapshot &&
		source.columns === metadata.columns &&
		source.cellPixelWidth === metadata.cellPixelWidth &&
		source.cellPixelHeight === metadata.cellPixelHeight &&
		source.generation === metadata.generation &&
		source.hostEpoch === metadata.hostEpoch &&
		source.revision === metadata.revision &&
		source.themeRevision === metadata.themeRevision
	);
}

function projectionString(
	cache: ProjectionMaterializationStringCache,
	source: DecodedTerminalProjectionBuffer,
	offset: number | undefined,
	length: number | undefined,
): ProtocolResult<string | null> {
	if (offset === undefined || length === undefined) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Projection string range is missing');
	}
	const prepared = cache.rangesBySource
		.get(source.stringBytes)
		?.get(projectionStringRangeKey(offset, length));
	if (prepared === undefined) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Projection string range was not preflighted',
		);
	}
	if (prepared.decoded !== null) return prepared.decoded;
	if (prepared.offset === MAX_UINT32) {
		prepared.decoded = protocolSuccess(null);
		return prepared.decoded;
	}
	try {
		prepared.decoded = protocolSuccess(
			textDecoder.decode(
				source.stringBytes.subarray(prepared.offset, prepared.offset + prepared.length),
			),
		);
	} catch {
		prepared.decoded = protocolFailure(
			ProtocolErrorCode.invalidUtf8,
			'Projection string is not valid UTF-8',
		);
	}
	return prepared.decoded;
}

function materializedColor(value: number | undefined): ProtocolResult<TerminalColor> {
	if (value === TerminalColorEncoding.default) {
		return protocolSuccess({ kind: TerminalColorKind.default });
	}
	if (value === TerminalColorEncoding.none) {
		return protocolSuccess({ kind: TerminalColorKind.none });
	}
	return value !== undefined && value <= COLOR_RGB_MASK
		? protocolSuccess({ kind: TerminalColorKind.rgb, rgb: value })
		: protocolFailure(ProtocolErrorCode.invalidField, 'Projection color is invalid');
}

function projectionStyleKey(source: DecodedTerminalProjectionBuffer, cellOffset: number): string {
	return `${source.cells.foregroundColors[cellOffset]}:${source.cells.backgroundColors[cellOffset]}:${source.cells.underlineColors[cellOffset]}:${source.cells.hyperlinkOffsets[cellOffset]}:${source.cells.hyperlinkLengths[cellOffset]}:${source.cells.styleFlags[cellOffset]}:${source.cells.underlineKinds[cellOffset]}`;
}

function materializedStyle(
	cache: ProjectionMaterializationStringCache,
	source: DecodedTerminalProjectionBuffer,
	cellOffset: number,
): ProtocolResult<TerminalStyle> {
	const foreground = materializedColor(source.cells.foregroundColors[cellOffset]);
	if (!foreground.ok) return foreground;
	const background = materializedColor(source.cells.backgroundColors[cellOffset]);
	if (!background.ok) return background;
	const underline = materializedColor(source.cells.underlineColors[cellOffset]);
	if (!underline.ok) return underline;
	const hyperlink = projectionString(
		cache,
		source,
		source.cells.hyperlinkOffsets[cellOffset],
		source.cells.hyperlinkLengths[cellOffset],
	);
	if (!hyperlink.ok) return hyperlink;
	const flags = source.cells.styleFlags[cellOffset];
	const underlineKind = materializedUnderlineKind(source.cells.underlineKinds[cellOffset]);
	if (flags === undefined || (flags & ~STYLE_FLAGS_MASK) !== 0 || !underlineKind.ok) {
		return protocolFailure(ProtocolErrorCode.invalidField, 'Projection style is invalid');
	}
	return protocolSuccess({
		background: background.value,
		blink: (flags & TerminalStyleFlag.blink) !== 0,
		bold: (flags & TerminalStyleFlag.bold) !== 0,
		faint: (flags & TerminalStyleFlag.faint) !== 0,
		foreground: foreground.value,
		hyperlink: hyperlink.value,
		inverse: (flags & TerminalStyleFlag.inverse) !== 0,
		invisible: (flags & TerminalStyleFlag.invisible) !== 0,
		italic: (flags & TerminalStyleFlag.italic) !== 0,
		overline: (flags & TerminalStyleFlag.overline) !== 0,
		strikethrough: (flags & TerminalStyleFlag.strikethrough) !== 0,
		underline: underline.value,
		underlineKind: underlineKind.value,
	});
}

function materializedWidth(value: number | undefined): ProtocolResult<CellWidthValue> {
	if (value === 0) return protocolSuccess(CellWidth.narrow);
	if (value === 1) return protocolSuccess(CellWidth.wide);
	if (value === 2) return protocolSuccess(CellWidth.spacerTail);
	if (value === 3) return protocolSuccess(CellWidth.spacerHead);
	return protocolFailure(ProtocolErrorCode.invalidField, 'Projection cell width is invalid');
}

function materializedSemantic(value: number | undefined): ProtocolResult<SemanticContentValue> {
	if (value === 0) return protocolSuccess(SemanticContent.output);
	if (value === 1) return protocolSuccess(SemanticContent.input);
	if (value === 2) return protocolSuccess(SemanticContent.prompt);
	return protocolFailure(ProtocolErrorCode.invalidField, 'Projection semantic content is invalid');
}

function materializedPromptClass(value: number | undefined): ProtocolResult<RowPromptClassValue> {
	if (value === 0) return protocolSuccess(RowPromptClass.none);
	if (value === 1) return protocolSuccess(RowPromptClass.prompt);
	if (value === 2) return protocolSuccess(RowPromptClass.promptContinuation);
	return protocolFailure(ProtocolErrorCode.invalidField, 'Projection prompt class is invalid');
}

function materializedUnderlineKind(value: number | undefined): ProtocolResult<UnderlineKindValue> {
	if (value === 0) return protocolSuccess(UnderlineKind.none);
	if (value === 1) return protocolSuccess(UnderlineKind.single);
	if (value === 2) return protocolSuccess(UnderlineKind.double);
	if (value === 3) return protocolSuccess(UnderlineKind.curly);
	if (value === 4) return protocolSuccess(UnderlineKind.dotted);
	if (value === 5) return protocolSuccess(UnderlineKind.dashed);
	return protocolFailure(ProtocolErrorCode.invalidField, 'Projection underline kind is invalid');
}
