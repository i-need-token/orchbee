import { createHash } from 'node:crypto';

import {
	CellWidth,
	decodeTerminalProjectionBuffer,
	decodeTerminalRowWindow,
	decodeTerminalGraphics,
	encodeTerminalFrame,
	encodeTerminalGraphics,
	encodeTerminalRowWindow,
	materializeTerminalProjectionFrame,
	PROTOCOL_LIMITS,
	TerminalFrameKind,
	TerminalGraphicsUpdateKind,
	TerminalIdentitySchema,
	TerminalRowWindowDetail,
	TERMINAL_ROW_WINDOW_HEADER_BYTES,
	TerminalThemeStateSchema,
	type DecodedTerminalProjectionBuffer,
	type DecodedTerminalRenderRowWindow,
	type TerminalFrame,
	type TerminalGraphics,
	type TerminalGraphicsImage,
	type TerminalIdentity,
	type TerminalTextRow,
	type TerminalThemeState,
} from '@orchbee/terminal-protocol';
import {
	TerminalPresentationIdSchema,
	type TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';
import { z } from 'zod';

export const MAX_TERMINAL_ARCHIVE_ROWS = 5_000;
export const MAX_TERMINAL_ARCHIVE_BYTES = 10 * 1024 * 1024;
export const MAX_TERMINAL_ARCHIVE_STORE_BYTES = 250 * 1024 * 1024;
const TERMINAL_ARCHIVE_REPRESENTABILITY_FAILURE_DETAIL =
	'Terminal archive exceeds archived projection representability limits';

const TERMINAL_ARCHIVE_HEADER_BYTES = 12;
const TERMINAL_ARCHIVE_MAGIC = new Uint8Array([0x4f, 0x42, 0x54, 0x41]);
const TERMINAL_ARCHIVE_VERSION = 1;
const TERMINAL_FRAME_STRING_LENGTH_BYTES = 4;
const TERMINAL_FRAME_STYLE_RECORD_BYTES = 20;
const TERMINAL_TEXT_MAPPING_BYTES = 12;
const TERMINAL_TEXT_ROW_HEADER_BYTES = 12;
const textDecoder = new TextDecoder('utf-8', { fatal: true });
const textEncoder = new TextEncoder();

const TerminalArchiveDigestMetadataSchema = z.strictObject({
	chunkLengths: z.array(z.number().int().min(1).max(MAX_TERMINAL_ARCHIVE_BYTES)).max(5_000),
	frameLength: z.number().int().min(1).max(MAX_TERMINAL_ARCHIVE_BYTES),
	graphicsLength: z.number().int().min(1).max(PROTOCOL_LIMITS.maxTerminalGraphicsExtensionBytes),
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
	readonly graphics: Uint8Array;
	readonly identity: TerminalIdentity;
	readonly presentationId: TerminalPresentationId;
	readonly rowWindows: readonly Uint8Array[];
	readonly theme: TerminalThemeState;
}

export interface TerminalArchive {
	readonly encodedByteLength: number;
	readonly frame: DecodedTerminalProjectionBuffer;
	readonly frameBytes: Uint8Array;
	readonly graphics: TerminalGraphics;
	readonly graphicsBytes: Uint8Array;
	readonly identity: TerminalIdentity;
	readonly presentationId: TerminalPresentationId;
	readonly retainedRows: number;
	readonly rowWindowBytes: readonly Uint8Array[];
	readonly rowWindows: readonly DecodedTerminalRenderRowWindow[];
	readonly sourceStartRow: bigint;
	readonly theme: TerminalThemeState;
}

export type TerminalArchiveCodecResult<Value> =
	| { readonly ok: true; readonly value: Value }
	| { readonly detail: string; readonly ok: false };

interface ArchiveRepresentabilityRow {
	readonly frameStrings: readonly string[];
	readonly projectionGraphemeBytes: number;
	readonly projectionHyperlinks: readonly string[];
	readonly styleKeys: readonly string[];
	readonly textBytesUpperBound: number;
}

interface ArchiveRepresentabilityRows {
	readonly rows: readonly ArchiveRepresentabilityRow[];
	readonly stringByteLengths: ReadonlyMap<string, number>;
}

export function encodeTerminalArchive(
	value: TerminalArchiveWrite,
): TerminalArchiveCodecResult<Uint8Array> {
	return encodeTerminalArchiveDocument(value, true);
}

/**
 * Preserves the capture loop's existing byte-based retention calculation without turning an
 * archive-only representability failure into a new truncation boundary. Capture must still call
 * encodeTerminalArchive once on the completed archive before exposing it to the store.
 */
export function encodeTerminalArchiveForCaptureSizing(
	value: TerminalArchiveWrite,
): TerminalArchiveCodecResult<Uint8Array> {
	return encodeTerminalArchiveDocument(value, false);
}

function encodeTerminalArchiveDocument(
	value: TerminalArchiveWrite,
	preflightRepresentability: boolean,
): TerminalArchiveCodecResult<Uint8Array> {
	if (!archivePayloadFitsWithinLimit(value)) {
		return failure('Terminal archive exceeds its per-presentation byte limit');
	}
	const validated = validateTerminalArchiveParts(value, preflightRepresentability);
	if (!validated.ok) return validated;
	const digestMetadata = TerminalArchiveDigestMetadataSchema.parse({
		chunkLengths: value.rowWindows.map((window) => window.byteLength),
		frameLength: value.frame.byteLength,
		graphicsLength: value.graphics.byteLength,
		identity: value.identity,
		presentationId: value.presentationId,
		retainedRows: validated.value.retainedRows,
		sourceStartRow: validated.value.sourceStartRow.toString(),
		theme: value.theme,
		version: TERMINAL_ARCHIVE_VERSION,
	});
	const header = TerminalArchiveHeaderSchema.parse({
		...digestMetadata,
		contentDigest: archiveContentDigest(
			digestMetadata,
			value.frame,
			value.graphics,
			value.rowWindows,
		),
	});
	const headerBytes = textEncoder.encode(JSON.stringify(header));
	const totalBytes =
		TERMINAL_ARCHIVE_HEADER_BYTES +
		headerBytes.byteLength +
		value.frame.byteLength +
		value.graphics.byteLength +
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
	encoded.set(value.graphics, offset);
	offset += value.graphics.byteLength;
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
		header.data.frameLength +
		header.data.graphicsLength +
		header.data.chunkLengths.reduce((total, length) => total + length, 0);
	if (TERMINAL_ARCHIVE_HEADER_BYTES + headerLength + payloadLength !== encoded.byteLength) {
		return failure('Terminal archive payload length does not match its header');
	}
	let offset = TERMINAL_ARCHIVE_HEADER_BYTES + headerLength;
	const frameBytes = encoded.subarray(offset, offset + header.data.frameLength);
	offset += header.data.frameLength;
	const graphicsBytes = encoded.subarray(offset, offset + header.data.graphicsLength);
	offset += header.data.graphicsLength;
	const rowWindowBytes = header.data.chunkLengths.map((length) => {
		const window = encoded.subarray(offset, offset + length);
		offset += length;
		return window;
	});
	const digestMetadata: TerminalArchiveDigestMetadata = {
		chunkLengths: header.data.chunkLengths,
		frameLength: header.data.frameLength,
		graphicsLength: header.data.graphicsLength,
		identity: header.data.identity,
		presentationId: header.data.presentationId,
		retainedRows: header.data.retainedRows,
		sourceStartRow: header.data.sourceStartRow,
		theme: header.data.theme,
		version: header.data.version,
	};
	if (
		archiveContentDigest(digestMetadata, frameBytes, graphicsBytes, rowWindowBytes) !==
		header.data.contentDigest
	) {
		return failure('Terminal archive content digest does not match its header');
	}
	const validated = validateTerminalArchiveParts(
		{
			frame: frameBytes,
			graphics: graphicsBytes,
			identity: header.data.identity,
			presentationId: header.data.presentationId,
			rowWindows: rowWindowBytes,
			theme: header.data.theme,
		},
		true,
	);
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
			graphics: validated.value.graphics,
			graphicsBytes,
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

function validateTerminalArchiveParts(
	value: TerminalArchiveWrite,
	preflightRepresentability: boolean,
): TerminalArchiveCodecResult<{
	readonly frame: DecodedTerminalProjectionBuffer;
	readonly graphics: TerminalGraphics;
	readonly retainedRows: number;
	readonly rowWindows: readonly DecodedTerminalRenderRowWindow[];
	readonly sourceStartRow: bigint;
}> {
	const presentationId = TerminalPresentationIdSchema.safeParse(value.presentationId);
	const identity = TerminalIdentitySchema.safeParse(value.identity);
	const theme = TerminalThemeStateSchema.safeParse(value.theme);
	if (!presentationId.success || !identity.success || !theme.success) {
		return failure('Terminal archive identity, presentation, or theme is invalid');
	}
	const decodedFrame = decodeTerminalProjectionBuffer(value.frame);
	const decodedGraphics = decodeTerminalGraphics(value.graphics);
	if (!decodedFrame.ok || decodedFrame.value.kind !== TerminalFrameKind.snapshot) {
		return failure('Terminal archive frame is not a valid packed snapshot');
	}
	if (!decodedGraphics.ok || decodedGraphics.value.kind !== TerminalGraphicsUpdateKind.snapshot) {
		return failure('Terminal archive graphics are not a valid resource snapshot');
	}
	const frame = decodedFrame.value;
	const graphics = decodedGraphics.value;
	if (
		frame.hostEpoch !== identity.data.hostEpoch ||
		frame.themeRevision !== BigInt(theme.data.revision)
	) {
		return failure('Terminal archive frame identity or theme revision is inconsistent');
	}
	const decodedWindows: DecodedTerminalRenderRowWindow[] = [];
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
			window.cellPixelWidth !== frame.cellPixelWidth ||
			window.cellPixelHeight !== frame.cellPixelHeight ||
			window.generation !== frame.generation ||
			window.hostEpoch !== frame.hostEpoch ||
			window.revision !== frame.revision ||
			window.themeRevision !== frame.themeRevision
		) {
			return failure('Terminal archive row-window metadata is inconsistent');
		}
		decodedWindows.push(decoded.value);
	}
	if (!archiveGraphicsSnapshotsAreConsistent(graphics, decodedWindows)) {
		return failure('Terminal archive graphics snapshots are inconsistent');
	}
	let expectedStart = decodedWindows[0]?.frame.scrollbackRows ?? frame.scrollbackRows;
	for (const window of decodedWindows) {
		if (window.frame.scrollbackRows !== expectedStart) {
			return failure('Terminal archive row windows are not contiguous');
		}
		expectedStart += BigInt(window.frame.rows);
	}
	if (expectedStart !== frame.scrollbackRows) {
		return failure('Terminal archive history does not meet the final viewport');
	}
	const sourceStartRow = decodedWindows[0]?.frame.scrollbackRows ?? frame.scrollbackRows;
	const retainedRows = Number(frame.scrollbackRows + BigInt(frame.rows) - sourceStartRow);
	if (retainedRows < 1 || retainedRows > MAX_TERMINAL_ARCHIVE_ROWS) {
		return failure('Terminal archive retained row count is outside its limit');
	}
	if (
		preflightRepresentability &&
		!validateTerminalArchiveRepresentability(
			frame,
			graphics,
			decodedWindows,
			sourceStartRow,
			retainedRows,
		)
	) {
		return failure(TERMINAL_ARCHIVE_REPRESENTABILITY_FAILURE_DETAIL);
	}
	return {
		ok: true,
		value: { frame, graphics, retainedRows, rowWindows: decodedWindows, sourceStartRow },
	};
}

function archiveGraphicsSnapshotsAreConsistent(
	viewport: TerminalGraphics,
	rowWindows: readonly DecodedTerminalRenderRowWindow[],
): boolean {
	const images = new Map<number, TerminalGraphicsImage>();
	for (const snapshot of [viewport, ...rowWindows.map((window) => window.graphics)]) {
		if (
			snapshot.kind !== TerminalGraphicsUpdateKind.snapshot ||
			snapshot.resourceRevision !== viewport.resourceRevision ||
			snapshot.animationTick !== viewport.animationTick
		) {
			return false;
		}
		for (const image of snapshot.images) {
			const previous = images.get(image.id);
			if (previous === undefined) {
				images.set(image.id, image);
				continue;
			}
			if (
				previous.generation !== image.generation ||
				previous.format !== image.format ||
				previous.width !== image.width ||
				previous.height !== image.height ||
				previous.data.byteLength !== image.data.byteLength
			) {
				return false;
			}
			for (let offset = 0; offset < image.data.byteLength; offset++) {
				if (previous.data[offset] !== image.data[offset]) return false;
			}
		}
	}
	return true;
}

function validateTerminalArchiveRepresentability(
	frame: DecodedTerminalProjectionBuffer,
	graphics: TerminalGraphics,
	rowWindows: readonly DecodedTerminalRenderRowWindow[],
	sourceStartRow: bigint,
	retainedRows: number,
): boolean {
	const sources = [...rowWindows.map((window) => window.frame), frame];
	// Archived operations may splice any retained range across stored OBPB chunks. Summarizing each
	// retained cell once keeps admission linear while the archive-wide canonical tables avoid
	// inventing a representability-driven row-retention policy.
	const summarized = summarizeArchiveRepresentability(frame, sources, retainedRows);
	if (summarized === null || summarized.rows.length !== retainedRows) return false;

	const frameStart = maximumFrameTableWindowStart(
		summarized.rows,
		frame.rows,
		frame.title,
		frame.currentDirectory,
		summarized.stringByteLengths,
	);
	const frameCandidate = materializeArchiveFrame(
		frame,
		sources,
		sourceStartRow,
		frameStart,
		frame.rows,
	);
	if (frameCandidate === null || !encodeTerminalFrame(frameCandidate).ok) return false;

	const renderWindow = maximumProjectionStringWindow(
		summarized.rows,
		frame.rows,
		summarized.stringByteLengths,
	);
	if (renderWindow === null) return false;
	const renderCandidate = materializeArchiveFrame(
		frame,
		sources,
		sourceStartRow,
		renderWindow.start,
		frame.rows,
	);
	const renderGraphics = materializeArchiveGraphics(
		frame,
		graphics,
		rowWindows,
		sourceStartRow,
		BigInt(renderWindow.start),
		frame.rows,
	);
	if (
		renderCandidate === null ||
		renderGraphics === null ||
		!encodeTerminalRowWindow({
			...archiveRowWindowMetadata(frame, retainedRows, renderWindow.start),
			detail: TerminalRowWindowDetail.render,
			frame: renderCandidate,
			graphics: renderGraphics,
		}).ok
	) {
		return false;
	}

	const textWindow = maximumNumericWindow(
		summarized.rows.map((row) => row.textBytesUpperBound),
		frame.rows,
	);
	if (TERMINAL_ROW_WINDOW_HEADER_BYTES + textWindow.total > PROTOCOL_LIMITS.maxRowWindowBytes) {
		return false;
	}
	const textCandidate = materializeArchiveFrame(
		frame,
		sources,
		sourceStartRow,
		textWindow.start,
		frame.rows,
	);
	return (
		textCandidate !== null &&
		encodeTerminalRowWindow({
			...archiveRowWindowMetadata(frame, retainedRows, textWindow.start),
			detail: TerminalRowWindowDetail.text,
			rows: terminalArchiveTextRows(textCandidate, BigInt(textWindow.start)),
		}).ok
	);
}

function summarizeArchiveRepresentability(
	metadata: DecodedTerminalProjectionBuffer,
	sources: readonly DecodedTerminalProjectionBuffer[],
	retainedRows: number,
): ArchiveRepresentabilityRows | null {
	if (!projectionMaterializationStringsAreBounded(metadata, sources)) return null;
	const rows: ArchiveRepresentabilityRow[] = [];
	const aggregateStyles = new Set<string>();
	const aggregateStrings = new Set<string>();
	const stringByteLengths = new Map<string, number>();
	let selectionBytesUpperBound = Math.max(0, retainedRows - 1);
	let previousRow: TerminalFrame['rowData'][number] | undefined;
	const addAggregateString = (value: string | null): boolean => {
		if (value === null || aggregateStrings.has(value)) return true;
		if (aggregateStrings.size >= PROTOCOL_LIMITS.maxFrameStrings) return false;
		aggregateStrings.add(value);
		stringByteLengths.set(value, textEncoder.encode(value).byteLength);
		return true;
	};
	if (!addAggregateString(metadata.title) || !addAggregateString(metadata.currentDirectory)) {
		return null;
	}

	for (const source of sources) {
		const materialized = materializeTerminalProjectionFrame(metadata, [source], {
			attachmentId: metadata.attachmentId,
			revealCursor: false,
			rowCount: source.rows,
			scrollbackRows: 0n,
			sequence: 1n,
			sourceStartRow: source.scrollbackRows,
		});
		if (!materialized.ok) return null;
		const styleKeys: string[] = [];
		for (const materializedStyle of materialized.value.styles) {
			const key = JSON.stringify(materializedStyle);
			if (!aggregateStyles.has(key)) {
				if (aggregateStyles.size >= PROTOCOL_LIMITS.maxFrameStyles) return null;
				aggregateStyles.add(key);
			}
			if (!addAggregateString(materializedStyle.hyperlink)) return null;
			styleKeys.push(key);
		}

		for (const row of materialized.value.rowData) {
			if (rows.length >= retainedRows || !archiveRowsAreStructurallyComposable(previousRow, row)) {
				return null;
			}
			const frameStrings: string[] = [];
			const projectionHyperlinks: string[] = [];
			const rowStyleKeys: string[] = [];
			let projectionGraphemeBytes = 0;
			let textBytesUpperBound = TERMINAL_TEXT_ROW_HEADER_BYTES;
			for (const cell of row.cells) {
				const materializedStyle = materialized.value.styles[cell.style];
				const styleKey = styleKeys[cell.style];
				if (materializedStyle === undefined || styleKey === undefined) return null;
				const graphemeBytes = stringByteLength(
					cell.grapheme,
					stringByteLengths,
					addAggregateString,
				);
				if (graphemeBytes === null) return null;
				rowStyleKeys.push(styleKey);
				frameStrings.push(cell.grapheme);
				projectionGraphemeBytes += graphemeBytes;
				if (materializedStyle.hyperlink !== null) {
					frameStrings.push(materializedStyle.hyperlink);
					projectionHyperlinks.push(materializedStyle.hyperlink);
				}
				if (cell.width !== CellWidth.spacerTail) {
					selectionBytesUpperBound += graphemeBytes;
					if (selectionBytesUpperBound > PROTOCOL_LIMITS.maxSelectionExportBytes) return null;
					if (cell.grapheme.length > 0) {
						textBytesUpperBound += graphemeBytes + TERMINAL_TEXT_MAPPING_BYTES;
					}
				}
			}
			rows.push({
				frameStrings,
				projectionGraphemeBytes,
				projectionHyperlinks,
				styleKeys: rowStyleKeys,
				textBytesUpperBound,
			});
			previousRow = row;
		}
	}
	return { rows, stringByteLengths };
}

function archiveRowsAreStructurallyComposable(
	previousRow: TerminalFrame['rowData'][number] | undefined,
	nextRow: TerminalFrame['rowData'][number],
): boolean {
	if (previousRow === undefined) return true;
	return (
		previousRow.wrap === nextRow.wrapContinuation &&
		(previousRow.cells.at(-1)?.width !== CellWidth.spacerHead ||
			nextRow.cells[0]?.width === CellWidth.wide)
	);
}

function projectionMaterializationStringsAreBounded(
	metadata: DecodedTerminalProjectionBuffer,
	sources: readonly DecodedTerminalProjectionBuffer[],
): boolean {
	// The shared materializer decodes graphemes and hyperlinks per cell. Account their expanded
	// ranges before it allocates so aliased strict-valid OBPB ranges cannot make archive admission
	// perform work beyond the existing full-selection byte budget.
	let expandedBytes = 0;
	const account = (byteLength: number): boolean => {
		if (byteLength > PROTOCOL_LIMITS.maxSelectionExportBytes - expandedBytes) return false;
		expandedBytes += byteLength;
		return true;
	};
	for (const value of [metadata.title, metadata.currentDirectory]) {
		if (value !== null && !account(textEncoder.encode(value).byteLength)) return false;
	}
	for (const source of sources) {
		const cellCount = source.rows * source.columns;
		for (let index = 0; index < cellCount; index += 1) {
			const graphemeBytes = source.cells.graphemeLengths[index];
			const hyperlinkBytes = source.cells.hyperlinkLengths[index];
			if (
				graphemeBytes === undefined ||
				hyperlinkBytes === undefined ||
				!account(graphemeBytes) ||
				!account(hyperlinkBytes)
			) {
				return false;
			}
		}
	}
	return true;
}

function stringByteLength(
	value: string,
	lengths: Map<string, number>,
	addAggregateString: (value: string) => boolean,
): number | null {
	const existing = lengths.get(value);
	if (existing !== undefined) return existing;
	if (!addAggregateString(value)) return null;
	return lengths.get(value) ?? null;
}

function maximumFrameTableWindowStart(
	rows: readonly ArchiveRepresentabilityRow[],
	rowCount: number,
	title: string | null,
	currentDirectory: string | null,
	stringByteLengths: ReadonlyMap<string, number>,
): number {
	const styleCounts = new Map<string, number>();
	const stringCounts = new Map<string, number>();
	let tableBytes = 0;
	const addString = (value: string): void => {
		const count = stringCounts.get(value) ?? 0;
		if (count === 0) {
			tableBytes += TERMINAL_FRAME_STRING_LENGTH_BYTES + (stringByteLengths.get(value) ?? 0);
		}
		stringCounts.set(value, count + 1);
	};
	const removeString = (value: string): void => {
		const count = stringCounts.get(value);
		if (count === undefined) return;
		if (count === 1) {
			stringCounts.delete(value);
			tableBytes -= TERMINAL_FRAME_STRING_LENGTH_BYTES + (stringByteLengths.get(value) ?? 0);
		} else {
			stringCounts.set(value, count - 1);
		}
	};
	const addStyle = (value: string): void => {
		const count = styleCounts.get(value) ?? 0;
		if (count === 0) tableBytes += TERMINAL_FRAME_STYLE_RECORD_BYTES;
		styleCounts.set(value, count + 1);
	};
	const removeStyle = (value: string): void => {
		const count = styleCounts.get(value);
		if (count === undefined) return;
		if (count === 1) {
			styleCounts.delete(value);
			tableBytes -= TERMINAL_FRAME_STYLE_RECORD_BYTES;
		} else {
			styleCounts.set(value, count - 1);
		}
	};
	if (title !== null) addString(title);
	if (currentDirectory !== null) addString(currentDirectory);
	const addRow = (row: ArchiveRepresentabilityRow): void => {
		for (const value of row.styleKeys) addStyle(value);
		for (const value of row.frameStrings) addString(value);
	};
	const removeRow = (row: ArchiveRepresentabilityRow): void => {
		for (const value of row.styleKeys) removeStyle(value);
		for (const value of row.frameStrings) removeString(value);
	};
	for (let index = 0; index < rowCount; index += 1) {
		const row = rows[index];
		if (row !== undefined) addRow(row);
	}
	let maximumBytes = tableBytes;
	let maximumStart = 0;
	for (let start = 1; start + rowCount <= rows.length; start += 1) {
		const removed = rows[start - 1];
		const added = rows[start + rowCount - 1];
		if (removed !== undefined) removeRow(removed);
		if (added !== undefined) addRow(added);
		if (tableBytes > maximumBytes) {
			maximumBytes = tableBytes;
			maximumStart = start;
		}
	}
	return maximumStart;
}

function maximumProjectionStringWindow(
	rows: readonly ArchiveRepresentabilityRow[],
	rowCount: number,
	stringByteLengths: ReadonlyMap<string, number>,
): { readonly start: number; readonly total: number } | null {
	const hyperlinkCounts = new Map<string, number>();
	let total = 0;
	const addRow = (row: ArchiveRepresentabilityRow): boolean => {
		total += row.projectionGraphemeBytes;
		for (const hyperlink of row.projectionHyperlinks) {
			const count = hyperlinkCounts.get(hyperlink) ?? 0;
			if (count === 0) {
				const byteLength = stringByteLengths.get(hyperlink);
				if (byteLength === undefined) return false;
				total += byteLength;
			}
			hyperlinkCounts.set(hyperlink, count + 1);
		}
		return true;
	};
	const removeRow = (row: ArchiveRepresentabilityRow): boolean => {
		total -= row.projectionGraphemeBytes;
		for (const hyperlink of row.projectionHyperlinks) {
			const count = hyperlinkCounts.get(hyperlink);
			if (count === undefined) return false;
			if (count === 1) {
				const byteLength = stringByteLengths.get(hyperlink);
				if (byteLength === undefined) return false;
				hyperlinkCounts.delete(hyperlink);
				total -= byteLength;
			} else {
				hyperlinkCounts.set(hyperlink, count - 1);
			}
		}
		return true;
	};
	for (let index = 0; index < rowCount; index += 1) {
		const row = rows[index];
		if (row === undefined || !addRow(row)) return null;
	}
	let maximum = total;
	let maximumStart = 0;
	for (let start = 1; start + rowCount <= rows.length; start += 1) {
		const removed = rows[start - 1];
		const added = rows[start + rowCount - 1];
		if (removed === undefined || added === undefined || !removeRow(removed) || !addRow(added)) {
			return null;
		}
		if (total > maximum) {
			maximum = total;
			maximumStart = start;
		}
	}
	return { start: maximumStart, total: maximum };
}

function maximumNumericWindow(
	values: readonly number[],
	windowLength: number,
): { readonly start: number; readonly total: number } {
	let total = values.slice(0, windowLength).reduce((sum, value) => sum + value, 0);
	let maximum = total;
	let maximumStart = 0;
	for (let start = 1; start + windowLength <= values.length; start += 1) {
		total -= values[start - 1] ?? 0;
		total += values[start + windowLength - 1] ?? 0;
		if (total > maximum) {
			maximum = total;
			maximumStart = start;
		}
	}
	return { start: maximumStart, total: maximum };
}

function materializeArchiveFrame(
	metadata: DecodedTerminalProjectionBuffer,
	sources: readonly DecodedTerminalProjectionBuffer[],
	sourceStartRow: bigint,
	startRow: number,
	rowCount: number,
): TerminalFrame | null {
	const materialized = materializeTerminalProjectionFrame(metadata, sources, {
		attachmentId: metadata.attachmentId,
		revealCursor: false,
		rowCount,
		scrollbackRows: BigInt(startRow),
		sequence: 1n,
		sourceStartRow: sourceStartRow + BigInt(startRow),
	});
	return materialized.ok ? materialized.value : null;
}

/** Re-roots retained graphics snapshots into one archive-relative render row range. */
export function terminalArchiveGraphicsForRows(
	archive: TerminalArchive,
	startRow: bigint,
	rowCount: number,
): TerminalGraphics | null {
	if (startRow < 0n || rowCount < 1 || startRow + BigInt(rowCount) > BigInt(archive.retainedRows)) {
		return null;
	}
	return materializeArchiveGraphics(
		archive.frame,
		archive.graphics,
		archive.rowWindows,
		archive.sourceStartRow,
		startRow,
		rowCount,
	);
}

function materializeArchiveGraphics(
	frame: DecodedTerminalProjectionBuffer,
	viewport: TerminalGraphics,
	rowWindows: readonly DecodedTerminalRenderRowWindow[],
	sourceStartRow: bigint,
	startRow: bigint,
	rowCount: number,
): TerminalGraphics | null {
	const targetStart = sourceStartRow + startRow;
	const targetHeight = rowCount * frame.cellPixelHeight;
	const images = new Map<number, TerminalGraphicsImage>();
	const placements: TerminalGraphics['placements'][number][] = [];
	const placementKeys = new Set<string>();
	const sources = [
		...rowWindows.map((window) => ({
			graphics: window.graphics,
			startRow: window.startRow,
		})),
		{ graphics: viewport, startRow: frame.scrollbackRows },
	];
	for (const source of sources) {
		const sourceRowDelta = Number(source.startRow - targetStart);
		for (const placement of source.graphics.placements) {
			const pixelY = (sourceRowDelta + placement.row) * frame.cellPixelHeight + placement.offsetY;
			if (pixelY + placement.destinationHeight <= 0 || pixelY >= targetHeight) continue;
			const row = Math.floor(pixelY / frame.cellPixelHeight);
			const offsetY = pixelY - row * frame.cellPixelHeight;
			const reRooted = { ...placement, offsetY, row };
			const key = [
				reRooted.imageId,
				reRooted.placementId,
				reRooted.internalId ? 1 : 0,
				reRooted.column,
				reRooted.row,
				reRooted.offsetX,
				reRooted.offsetY,
				reRooted.sourceX,
				reRooted.sourceY,
				reRooted.sourceWidth,
				reRooted.sourceHeight,
				reRooted.destinationWidth,
				reRooted.destinationHeight,
				reRooted.z,
			].join(':');
			if (placementKeys.has(key)) continue;
			const image = source.graphics.images.find((candidate) => candidate.id === placement.imageId);
			if (image === undefined) return null;
			placementKeys.add(key);
			placements.push(reRooted);
			images.set(image.id, image);
		}
	}
	const graphics: TerminalGraphics = {
		animationTick: viewport.animationTick,
		images: [...images.values()],
		kind: TerminalGraphicsUpdateKind.snapshot,
		placements,
		resourceRevision: viewport.resourceRevision,
	};
	return encodeTerminalGraphics(graphics).ok ? graphics : null;
}

function archiveRowWindowMetadata(
	frame: DecodedTerminalProjectionBuffer,
	retainedRows: number,
	startRow: number,
) {
	return {
		attachmentId: frame.attachmentId,
		columns: frame.columns,
		generation: frame.generation,
		hostEpoch: frame.hostEpoch,
		revision: frame.revision,
		sequence: 1n,
		startRow: BigInt(startRow),
		totalRows: BigInt(retainedRows),
	};
}

export function terminalArchiveTextRows(
	frame: TerminalFrame,
	startRow: bigint,
): readonly TerminalTextRow[] {
	return frame.rowData.map((row, rowOffset) => {
		const mappings: Array<{
			byteOffset: number;
			column: number;
			utf16Offset: number;
			width: 1 | 2;
		}> = [];
		let text = '';
		let byteOffset = 0;
		let utf16Offset = 0;
		for (const [column, cell] of row.cells.entries()) {
			if (cell.width === CellWidth.spacerTail || cell.grapheme.length === 0) continue;
			mappings.push({
				byteOffset,
				column,
				utf16Offset,
				width: cell.width === CellWidth.wide ? 2 : 1,
			});
			text += cell.grapheme;
			byteOffset += textEncoder.encode(cell.grapheme).byteLength;
			utf16Offset += cell.grapheme.length;
		}
		return {
			index: startRow + BigInt(rowOffset),
			mappings,
			promptClass: row.promptClass,
			text,
			wrap: row.wrap,
			wrapContinuation: row.wrapContinuation,
		};
	});
}

function failure(detail: string): TerminalArchiveCodecResult<never> {
	return { detail, ok: false };
}

function archivePayloadFitsWithinLimit(value: TerminalArchiveWrite): boolean {
	let payloadBytes = value.frame.byteLength + value.graphics.byteLength;
	if (payloadBytes > MAX_TERMINAL_ARCHIVE_BYTES) return false;
	for (const window of value.rowWindows) {
		if (window.byteLength > MAX_TERMINAL_ARCHIVE_BYTES - payloadBytes) return false;
		payloadBytes += window.byteLength;
	}
	return true;
}

function archiveContentDigest(
	metadata: TerminalArchiveDigestMetadata,
	frame: Uint8Array,
	graphics: Uint8Array,
	rowWindows: readonly Uint8Array[],
): string {
	const hash = createHash('sha256');
	hash.update(JSON.stringify(metadata), 'utf8');
	hash.update(frame);
	hash.update(graphics);
	for (const window of rowWindows) hash.update(window);
	return hash.digest('hex');
}
