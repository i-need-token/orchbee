import {
	decodeTerminalFrame,
	CanonicalUint64TextSchema,
	FrameAppliedSchema,
	inspectTerminalFrameHeader,
	PROTOCOL_LIMITS,
	ProtocolErrorCode,
	TerminalCellWidthCode,
	TerminalCloseActivityCode,
	TerminalFrameExtensionKind,
	TerminalFrameKind,
	TerminalGraphicsUpdateKind,
	type ActiveScreenValue,
	type AttachmentId,
	type CursorShapeValue,
	type DecodedTerminalFrame,
	type DecodedTerminalProjectionBuffer,
	type FrameApplied,
	type HostEpoch,
	type InspectedTerminalFrameHeader,
	type NeedSnapshot,
	type ProtocolFailure,
	type TerminalInteraction,
	type TerminalGraphics,
	type TerminalGraphicsImage,
	type TerminalGraphicsPlacement,
	type TerminalCommandCompletion,
	type TerminalProgress,
} from '@orchbee/terminal-protocol';

import type { PackedRendererRow, PackedRendererSource } from '../rendering';

const NO_STRING_OFFSET = 0xffff_ffff;
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { ignoreBOM: true });

export const ProjectedStorePhase = {
	connectionTerminal: 'connection-terminal',
	disposed: 'disposed',
	empty: 'empty',
	snapshotRequired: 'snapshot-required',
	synchronized: 'synchronized',
} as const;

export type ProjectedStorePhase = (typeof ProjectedStorePhase)[keyof typeof ProjectedStorePhase];

type ProjectedFrameAcknowledgement = FrameApplied;

export type ProjectedTerminalCloseActivity = 'active' | 'idle' | 'unknown';

export type ProjectedFrameApplyResult =
	| {
			readonly acknowledgement: ProjectedFrameAcknowledgement;
			readonly kind: 'committed' | 'duplicate';
	  }
	| { readonly kind: 'connection-terminal'; readonly reason: ProjectedFrameConnectionTerminal }
	| { readonly kind: 'snapshot-required'; readonly request: NeedSnapshot }
	| { readonly kind: 'disposed' };

export type ProjectedFrameConnectionTerminal =
	| { readonly failure: ProtocolFailure; readonly kind: 'protocol' }
	| {
			readonly actual: number;
			readonly kind: 'presentation-limit-exceeded';
			readonly limit: 'aggregate-bytes' | 'strings' | 'styles';
			readonly maximum: number;
	  };

export const MAXIMUM_PACKED_TERMINAL_PRESENTATION_BYTES = PROTOCOL_LIMITS.maxFrameBytes;

export type ProjectedSnapshotReplaceResult =
	| {
			readonly acknowledgement: ProjectedFrameAcknowledgement;
			readonly byteLength: number;
			readonly kind: 'committed';
	  }
	| { readonly kind: 'disposed' | 'invalid' | 'too-large' };

export interface ProjectedTerminalMetadata {
	readonly activeScreen: ActiveScreenValue;
	readonly cellPixelHeight: number;
	readonly cellPixelWidth: number;
	readonly columns: number;
	readonly commandCompletion: TerminalCommandCompletion | null;
	readonly closeActivity: ProjectedTerminalCloseActivity;
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
	readonly interaction: TerminalInteraction;
	readonly progress: TerminalProgress;
	readonly revision: bigint;
	readonly rows: number;
	readonly scrollbackRows: bigint;
	readonly themeRevision: bigint;
	readonly title: string | null;
}

/** Decoded Kitty resources and visible placements retained independently from text rows. */
export interface ProjectedTerminalGraphics {
	readonly animationTick: bigint;
	readonly images: ReadonlyMap<number, TerminalGraphicsImage>;
	readonly placements: readonly TerminalGraphicsPlacement[];
	readonly resourceRevision: bigint;
}

/** Immutable, display-only packed state retained without transport or attachment authority. */
export interface ProjectedTerminalPresentationSnapshot extends PackedRendererSource {
	readonly byteLength: number;
	readonly graphics: ProjectedTerminalGraphics;
	readonly metadata: ProjectedTerminalMetadata;
}

const EMPTY_TERMINAL_GRAPHICS: ProjectedTerminalGraphics = {
	animationTick: 0n,
	images: new Map(),
	placements: [],
	resourceRevision: 0n,
};

function graphicsFromSnapshot(graphics: TerminalGraphics | null): ProjectedTerminalGraphics {
	if (graphics === null) return EMPTY_TERMINAL_GRAPHICS;
	if (graphics.kind !== TerminalGraphicsUpdateKind.snapshot) {
		throw new Error('Decoded terminal snapshot omitted its graphics resource snapshot');
	}
	return {
		animationTick: graphics.animationTick,
		images: new Map(graphics.images.map((image) => [image.id, image])),
		placements: graphics.placements,
		resourceRevision: graphics.resourceRevision,
	};
}

function graphicsFromDelta(
	current: ProjectedTerminalGraphics,
	graphics: TerminalGraphics | null,
): ProjectedTerminalGraphics | null {
	if (graphics === null) return current;
	if (
		graphics.kind !== TerminalGraphicsUpdateKind.placements ||
		graphics.resourceRevision !== current.resourceRevision ||
		graphics.animationTick < current.animationTick
	) {
		return null;
	}
	for (const placement of graphics.placements) {
		const image = current.images.get(placement.imageId);
		if (
			image === undefined ||
			placement.sourceX + placement.sourceWidth > image.width ||
			placement.sourceY + placement.sourceHeight > image.height
		) {
			return null;
		}
	}
	return {
		animationTick: graphics.animationTick,
		images: current.images,
		placements: graphics.placements,
		resourceRevision: current.resourceRevision,
	};
}

function graphicsByteLength(graphics: ProjectedTerminalGraphics): number {
	let byteLength = 0;
	for (const image of graphics.images.values()) byteLength += image.data.byteLength;
	return byteLength;
}

interface PackedTerminalRowBacking {
	readonly backgroundColors: Uint32Array;
	readonly foregroundColors: Uint32Array;
	readonly graphemeLengths: Uint16Array | Uint32Array;
	readonly graphemeOffsets: Uint32Array;
	readonly hyperlinkLengths: Uint16Array | Uint32Array;
	readonly hyperlinkOffsets: Uint32Array;
	readonly protectedCells: Uint8Array;
	readonly semanticContents: Uint8Array;
	readonly stringBytes: Uint8Array;
	readonly styleFlags: Uint16Array;
	readonly underlineColors: Uint32Array;
	readonly underlineKinds: Uint8Array;
	readonly widths: Uint8Array;
	readonly promptClass: number;
	readonly rowFlags: number;
}

export class PackedTerminalRowView {
	#backing: PackedTerminalRowBacking;
	readonly #graphemeCache: string[] = [];
	#dirty = true;
	readonly index: number;

	constructor(index: number, backing: PackedTerminalRowBacking) {
		this.index = index;
		this.#backing = backing;
	}

	get backgroundColors(): Uint32Array {
		return this.#backing.backgroundColors;
	}

	get foregroundColors(): Uint32Array {
		return this.#backing.foregroundColors;
	}

	get protectedCells(): Uint8Array {
		return this.#backing.protectedCells;
	}

	get semanticContents(): Uint8Array {
		return this.#backing.semanticContents;
	}

	get styleFlags(): Uint16Array {
		return this.#backing.styleFlags;
	}

	get underlineColors(): Uint32Array {
		return this.#backing.underlineColors;
	}

	get underlineKinds(): Uint8Array {
		return this.#backing.underlineKinds;
	}

	get widths(): Uint8Array {
		return this.#backing.widths;
	}

	get promptClass(): number {
		return this.#backing.promptClass;
	}

	get rowFlags(): number {
		return this.#backing.rowFlags;
	}

	get dirty(): boolean {
		return this.#dirty;
	}

	get byteLength(): number {
		return packedRowByteLength(this.#backing);
	}

	capturePresentationRow(): PackedRendererRow {
		return new PackedTerminalPresentationRow(this.#backing);
	}

	grapheme(column: number): string {
		const length = this.#backing.graphemeLengths[column];
		if (length === undefined || length === 0) return '';
		const cached = this.#graphemeCache[column];
		if (cached !== undefined) return cached;
		const grapheme = decodeRowString(
			this.#backing,
			this.#backing.graphemeOffsets,
			this.#backing.graphemeLengths,
			column,
		);
		this.#graphemeCache[column] = grapheme;
		return grapheme;
	}

	hyperlink(column: number): string | null {
		const offset = this.#backing.hyperlinkOffsets[column];
		if (offset === undefined || offset === NO_STRING_OFFSET) return null;
		return decodeRowString(
			this.#backing,
			this.#backing.hyperlinkOffsets,
			this.#backing.hyperlinkLengths,
			column,
		);
	}

	markClean(): void {
		this.#dirty = false;
	}

	replace(backing: PackedTerminalRowBacking): void {
		this.#backing = backing;
		this.#graphemeCache.length = 0;
		this.#dirty = true;
	}
}

class PackedTerminalPresentationRow implements PackedRendererRow {
	readonly #backing: PackedTerminalRowBacking;

	constructor(backing: PackedTerminalRowBacking) {
		this.#backing = backing;
	}

	get backgroundColors(): Uint32Array {
		return this.#backing.backgroundColors;
	}

	get dirty(): boolean {
		return true;
	}

	get foregroundColors(): Uint32Array {
		return this.#backing.foregroundColors;
	}

	get styleFlags(): Uint16Array {
		return this.#backing.styleFlags;
	}

	get underlineColors(): Uint32Array {
		return this.#backing.underlineColors;
	}

	get underlineKinds(): Uint8Array {
		return this.#backing.underlineKinds;
	}

	get widths(): Uint8Array {
		return this.#backing.widths;
	}

	grapheme(column: number): string {
		const length = this.#backing.graphemeLengths[column];
		if (length === undefined || length === 0) return '';
		return decodeRowString(
			this.#backing,
			this.#backing.graphemeOffsets,
			this.#backing.graphemeLengths,
			column,
		);
	}
}

interface PackedGrid {
	readonly backings: PackedTerminalRowBacking[];
	readonly directByteLength?: number;
	readonly rows: readonly PackedTerminalRowView[];
}

interface LastCommit {
	readonly baseRevision: bigint | null;
	readonly generation: bigint;
	readonly kind: string;
	readonly revision: bigint;
	readonly sequence: bigint;
	readonly themeRevision: bigint;
}

function decodeRowString(
	backing: PackedTerminalRowBacking,
	offsets: Uint32Array,
	lengths: Uint16Array | Uint32Array,
	column: number,
): string {
	const offset = offsets[column];
	const length = lengths[column];
	if (offset === undefined || length === undefined || offset === NO_STRING_OFFSET || length === 0) {
		return '';
	}
	return textDecoder.decode(backing.stringBytes.subarray(offset, offset + length));
}

interface PackedFrameBuildContext {
	readonly encodedStrings: (Uint8Array | undefined)[];
	readonly referencedStrings: number[];
	readonly seenAtRow: Uint32Array;
	readonly sharedCellColumns: PackedFrameCellColumns | null;
	readonly stringLengths: Uint32Array;
	readonly stringOffsets: Uint32Array;
}

interface PackedFrameCellColumns {
	readonly backgroundColors: Uint32Array;
	readonly foregroundColors: Uint32Array;
	readonly graphemeLengths: Uint32Array;
	readonly graphemeOffsets: Uint32Array;
	readonly hyperlinkLengths: Uint32Array;
	readonly hyperlinkOffsets: Uint32Array;
	readonly styleFlags: Uint16Array;
	readonly underlineColors: Uint32Array;
	readonly underlineKinds: Uint8Array;
}

function createPackedFrameCellColumns(cellCount: number): PackedFrameCellColumns {
	const hyperlinkOffsets = new Uint32Array(cellCount);
	hyperlinkOffsets.fill(NO_STRING_OFFSET);
	return {
		backgroundColors: new Uint32Array(cellCount),
		foregroundColors: new Uint32Array(cellCount),
		graphemeLengths: new Uint32Array(cellCount),
		graphemeOffsets: new Uint32Array(cellCount),
		hyperlinkLengths: new Uint32Array(cellCount),
		hyperlinkOffsets,
		styleFlags: new Uint16Array(cellCount),
		underlineColors: new Uint32Array(cellCount),
		underlineKinds: new Uint8Array(cellCount),
	};
}

function createPackedFrameBuildContext(
	frame: DecodedTerminalFrame,
	sharePackedCellColumns: boolean,
): PackedFrameBuildContext {
	return {
		encodedStrings: Array.from({ length: frame.strings.length }, () => undefined),
		referencedStrings: [],
		seenAtRow: new Uint32Array(frame.strings.length),
		sharedCellColumns: sharePackedCellColumns
			? createPackedFrameCellColumns(frame.cells.widths.length)
			: null,
		stringLengths: new Uint32Array(frame.strings.length),
		stringOffsets: new Uint32Array(frame.strings.length),
	};
}

function buildRowBacking(
	frame: DecodedTerminalFrame,
	rowOffset: number,
	context: PackedFrameBuildContext,
	borrowDecodedCellColumns: boolean,
): PackedTerminalRowBacking {
	const cellOffset = frame.rowData.cellOffsets[rowOffset];
	if (cellOffset === undefined) throw new Error('Decoded frame omitted a row cell offset');
	const columns = frame.columns;
	const cellEnd = cellOffset + columns;
	const sharedCellColumns = context.sharedCellColumns;
	const foregroundColors =
		sharedCellColumns?.foregroundColors.subarray(cellOffset, cellEnd) ?? new Uint32Array(columns);
	const backgroundColors =
		sharedCellColumns?.backgroundColors.subarray(cellOffset, cellEnd) ?? new Uint32Array(columns);
	const underlineColors =
		sharedCellColumns?.underlineColors.subarray(cellOffset, cellEnd) ?? new Uint32Array(columns);
	const styleFlags =
		sharedCellColumns?.styleFlags.subarray(cellOffset, cellEnd) ?? new Uint16Array(columns);
	const underlineKinds =
		sharedCellColumns?.underlineKinds.subarray(cellOffset, cellEnd) ?? new Uint8Array(columns);
	const widths = borrowDecodedCellColumns
		? frame.cells.widths.subarray(cellOffset, cellEnd)
		: frame.cells.widths.slice(cellOffset, cellEnd);
	const semanticContents = borrowDecodedCellColumns
		? frame.cells.semanticContents.subarray(cellOffset, cellEnd)
		: frame.cells.semanticContents.slice(cellOffset, cellEnd);
	const protectedCells = borrowDecodedCellColumns
		? frame.cells.protected.subarray(cellOffset, cellEnd)
		: frame.cells.protected.slice(cellOffset, cellEnd);
	const graphemeOffsets =
		sharedCellColumns?.graphemeOffsets.subarray(cellOffset, cellEnd) ?? new Uint32Array(columns);
	const graphemeLengths =
		sharedCellColumns?.graphemeLengths.subarray(cellOffset, cellEnd) ?? new Uint32Array(columns);
	const hyperlinkOffsets =
		sharedCellColumns?.hyperlinkOffsets.subarray(cellOffset, cellEnd) ?? new Uint32Array(columns);
	const hyperlinkLengths =
		sharedCellColumns?.hyperlinkLengths.subarray(cellOffset, cellEnd) ?? new Uint32Array(columns);
	if (sharedCellColumns === null) hyperlinkOffsets.fill(NO_STRING_OFFSET);

	const rowToken = rowOffset + 1;
	context.referencedStrings.length = 0;
	let byteLength = 0;
	for (let column = 0; column < columns; column += 1) {
		const absolute: number = cellOffset + column;
		const graphemeIndex = frame.cells.graphemeStringIndices[absolute];
		const styleIndex: number | undefined = frame.cells.styleIndices[absolute];
		if (graphemeIndex === undefined || styleIndex === undefined) {
			throw new Error('Decoded frame omitted a cell table value');
		}
		if (context.seenAtRow[graphemeIndex] !== rowToken) {
			const value = frame.strings[graphemeIndex];
			if (value === undefined) throw new Error('Decoded frame references a missing string');
			const encoded = context.encodedStrings[graphemeIndex] ?? textEncoder.encode(value);
			context.encodedStrings[graphemeIndex] = encoded;
			context.seenAtRow[graphemeIndex] = rowToken;
			context.referencedStrings.push(graphemeIndex);
			byteLength += encoded.byteLength;
		}
		const hyperlinkIndex = frame.styles.hyperlinkStringIndices[styleIndex];
		if (
			hyperlinkIndex !== undefined &&
			hyperlinkIndex !== NO_STRING_OFFSET &&
			context.seenAtRow[hyperlinkIndex] !== rowToken
		) {
			const value = frame.strings[hyperlinkIndex];
			if (value === undefined) throw new Error('Decoded frame references a missing string');
			const encoded = context.encodedStrings[hyperlinkIndex] ?? textEncoder.encode(value);
			context.encodedStrings[hyperlinkIndex] = encoded;
			context.seenAtRow[hyperlinkIndex] = rowToken;
			context.referencedStrings.push(hyperlinkIndex);
			byteLength += encoded.byteLength;
		}
	}

	const stringBytes = new Uint8Array(byteLength);
	let nextOffset = 0;
	for (const index of context.referencedStrings) {
		const bytes = context.encodedStrings[index];
		if (bytes === undefined) throw new Error('Decoded frame references a missing string');
		stringBytes.set(bytes, nextOffset);
		context.stringOffsets[index] = nextOffset;
		context.stringLengths[index] = bytes.byteLength;
		nextOffset += bytes.byteLength;
	}

	for (let column = 0; column < columns; column += 1) {
		const absolute: number = cellOffset + column;
		const graphemeIndex = frame.cells.graphemeStringIndices[absolute];
		const styleIndex: number | undefined = frame.cells.styleIndices[absolute];
		if (graphemeIndex === undefined || styleIndex === undefined) {
			throw new Error('Decoded frame omitted a cell table value');
		}
		const graphemeOffset = context.stringOffsets[graphemeIndex];
		const graphemeLength = context.stringLengths[graphemeIndex];
		if (
			graphemeOffset === undefined ||
			graphemeLength === undefined ||
			context.seenAtRow[graphemeIndex] !== rowToken
		) {
			throw new Error('Decoded frame omitted a grapheme string');
		}
		graphemeOffsets[column] = graphemeOffset;
		graphemeLengths[column] = graphemeLength;
		foregroundColors[column] = frame.styles.foregroundColors[styleIndex] ?? 0;
		backgroundColors[column] = frame.styles.backgroundColors[styleIndex] ?? 0;
		underlineColors[column] = frame.styles.underlineColors[styleIndex] ?? 0;
		styleFlags[column] = frame.styles.flags[styleIndex] ?? 0;
		underlineKinds[column] = frame.styles.underlineKinds[styleIndex] ?? 0;
		const hyperlinkIndex = frame.styles.hyperlinkStringIndices[styleIndex];
		if (hyperlinkIndex !== undefined && hyperlinkIndex !== NO_STRING_OFFSET) {
			const hyperlinkOffset = context.stringOffsets[hyperlinkIndex];
			const hyperlinkLength = context.stringLengths[hyperlinkIndex];
			if (
				hyperlinkOffset === undefined ||
				hyperlinkLength === undefined ||
				context.seenAtRow[hyperlinkIndex] !== rowToken
			) {
				throw new Error('Decoded frame omitted a hyperlink string');
			}
			hyperlinkOffsets[column] = hyperlinkOffset;
			hyperlinkLengths[column] = hyperlinkLength;
		}
	}

	return {
		backgroundColors,
		foregroundColors,
		graphemeLengths,
		graphemeOffsets,
		hyperlinkLengths,
		hyperlinkOffsets,
		protectedCells,
		semanticContents,
		stringBytes,
		styleFlags,
		underlineColors,
		underlineKinds,
		widths,
		promptClass: frame.rowData.promptClasses[rowOffset] ?? 0,
		rowFlags: frame.rowData.flags[rowOffset] ?? 0,
	};
}

function packedRowByteLength(backing: PackedTerminalRowBacking): number {
	return (
		backing.backgroundColors.byteLength +
		backing.foregroundColors.byteLength +
		backing.graphemeLengths.byteLength +
		backing.graphemeOffsets.byteLength +
		backing.hyperlinkLengths.byteLength +
		backing.hyperlinkOffsets.byteLength +
		backing.protectedCells.byteLength +
		backing.semanticContents.byteLength +
		backing.stringBytes.byteLength +
		backing.styleFlags.byteLength +
		backing.underlineColors.byteLength +
		backing.underlineKinds.byteLength +
		backing.widths.byteLength
	);
}

function metadataFromFrame(frame: DecodedTerminalFrame): ProjectedTerminalMetadata {
	return {
		activeScreen: frame.activeScreen,
		cellPixelHeight: frame.cellPixelHeight,
		cellPixelWidth: frame.cellPixelWidth,
		columns: frame.columns,
		commandCompletion: frame.commandCompletion,
		closeActivity: closeActivityFromExtensions(frame.extensions),
		currentDirectory: frame.currentDirectory,
		cursor: frame.cursor,
		defaultColors: frame.defaultColors,
		generation: frame.generation,
		interaction: frame.interaction,
		progress: frame.progress,
		revision: frame.revision,
		rows: frame.rows,
		scrollbackRows: frame.scrollbackRows,
		themeRevision: frame.themeRevision,
		title: frame.title,
	};
}

function metadataFromProjectionBuffer(
	frame: DecodedTerminalProjectionBuffer,
): ProjectedTerminalMetadata {
	return {
		activeScreen: frame.activeScreen,
		cellPixelHeight: frame.cellPixelHeight,
		cellPixelWidth: frame.cellPixelWidth,
		columns: frame.columns,
		commandCompletion: null,
		closeActivity: 'unknown',
		currentDirectory: frame.currentDirectory,
		cursor: frame.cursor,
		defaultColors: frame.defaultColors,
		generation: frame.generation,
		interaction: frame.interaction,
		progress: { state: 'remove', value: null },
		revision: frame.revision,
		rows: frame.rows,
		scrollbackRows: frame.scrollbackRows,
		themeRevision: frame.themeRevision,
		title: frame.title,
	};
}

function closeActivityFromExtensions(
	extensions: DecodedTerminalFrame['extensions'],
): ProjectedTerminalCloseActivity {
	const matches = extensions.filter(
		(extension) => extension.kind === TerminalFrameExtensionKind.closeActivity,
	);
	if (matches.length !== 1 || matches[0]?.data.byteLength !== 1) return 'unknown';
	switch (matches[0].data[0]) {
		case TerminalCloseActivityCode.active:
			return 'active';
		case TerminalCloseActivityCode.idle:
			return 'idle';
		default:
			return 'unknown';
	}
}

function candidateRowsHaveValidTopology(rows: readonly PackedTerminalRowBacking[]): boolean {
	for (let index = 1; index < rows.length; index += 1) {
		const previous = rows[index - 1];
		const next = rows[index];
		if (previous === undefined || next === undefined) return false;
		if ((previous.rowFlags & 0x01) !== (next.rowFlags & 0x02) >> 1) return false;
		if (
			previous.widths.at(-1) === TerminalCellWidthCode.spacerHead &&
			next.widths[0] !== TerminalCellWidthCode.wide
		) {
			return false;
		}
	}
	return true;
}

interface RetainedPresentationBudget {
	aggregateBytes: number;
	encodedBytes: number;
	readonly strings: Map<string, number>;
	readonly styles: Map<string, number>;
}

function createRetainedPresentationBudget(
	metadata: ProjectedTerminalMetadata,
	rows: readonly PackedTerminalRowBacking[],
	graphics: ProjectedTerminalGraphics,
): RetainedPresentationBudget {
	const cellCount = rows.reduce((total, row) => total + row.widths.length, 0);
	const directBytes =
		160 +
		rows.length * 8 +
		cellCount * 8 +
		metadataExtensionBytes(metadata) +
		graphicsSnapshotEncodedBytes(graphics);
	const budget: RetainedPresentationBudget = {
		aggregateBytes: directBytes,
		encodedBytes: directBytes,
		strings: new Map(),
		styles: new Map(),
	};
	addBudgetMetadata(budget, metadata);
	for (const row of rows) addBudgetRow(budget, row);
	return budget;
}

function validateRetainedPresentationBudget(
	budget: RetainedPresentationBudget,
): ProjectedFrameConnectionTerminal | null {
	if (budget.strings.size > PROTOCOL_LIMITS.maxFrameStrings) {
		return {
			actual: budget.strings.size,
			kind: 'presentation-limit-exceeded',
			limit: 'strings',
			maximum: PROTOCOL_LIMITS.maxFrameStrings,
		};
	}
	if (budget.styles.size > PROTOCOL_LIMITS.maxFrameStyles) {
		return {
			actual: budget.styles.size,
			kind: 'presentation-limit-exceeded',
			limit: 'styles',
			maximum: PROTOCOL_LIMITS.maxFrameStyles,
		};
	}
	const byteCharge = Math.max(budget.aggregateBytes, budget.encodedBytes);
	if (byteCharge > MAXIMUM_PACKED_TERMINAL_PRESENTATION_BYTES) {
		return {
			actual: byteCharge,
			kind: 'presentation-limit-exceeded',
			limit: 'aggregate-bytes',
			maximum: MAXIMUM_PACKED_TERMINAL_PRESENTATION_BYTES,
		};
	}
	return null;
}

function cloneRetainedPresentationBudget(
	budget: RetainedPresentationBudget,
): RetainedPresentationBudget {
	return {
		aggregateBytes: budget.aggregateBytes,
		encodedBytes: budget.encodedBytes,
		strings: new Map(budget.strings),
		styles: new Map(budget.styles),
	};
}

function replaceBudgetMetadata(
	budget: RetainedPresentationBudget,
	previous: ProjectedTerminalMetadata,
	replacement: ProjectedTerminalMetadata,
): void {
	removeBudgetOptionalString(budget, previous.title);
	removeBudgetOptionalString(budget, previous.currentDirectory);
	addBudgetMetadata(budget, replacement);
	const directDifference = metadataExtensionBytes(replacement) - metadataExtensionBytes(previous);
	budget.aggregateBytes += directDifference;
	budget.encodedBytes += directDifference;
}

function replaceBudgetRow(
	budget: RetainedPresentationBudget,
	previous: PackedTerminalRowBacking,
	replacement: PackedTerminalRowBacking,
): void {
	removeBudgetRow(budget, previous);
	addBudgetRow(budget, replacement);
}

function replaceBudgetGraphics(
	budget: RetainedPresentationBudget,
	previous: ProjectedTerminalGraphics,
	replacement: ProjectedTerminalGraphics,
): void {
	const directDifference =
		graphicsSnapshotEncodedBytes(replacement) - graphicsSnapshotEncodedBytes(previous);
	budget.aggregateBytes += directDifference;
	budget.encodedBytes += directDifference;
}

function addBudgetMetadata(
	budget: RetainedPresentationBudget,
	metadata: ProjectedTerminalMetadata,
): void {
	addBudgetOptionalString(budget, metadata.title);
	addBudgetOptionalString(budget, metadata.currentDirectory);
}

function addBudgetRow(budget: RetainedPresentationBudget, row: PackedTerminalRowBacking): void {
	for (let column = 0; column < row.widths.length; column += 1) {
		const grapheme = decodeRowString(row, row.graphemeOffsets, row.graphemeLengths, column);
		addBudgetString(budget, grapheme);
		const hyperlink = budgetRowHyperlink(row, column);
		addBudgetOptionalString(budget, hyperlink);
		addBudgetStyle(budget, budgetRowStyleKey(row, column, hyperlink));
	}
}

function removeBudgetRow(budget: RetainedPresentationBudget, row: PackedTerminalRowBacking): void {
	for (let column = 0; column < row.widths.length; column += 1) {
		const grapheme = decodeRowString(row, row.graphemeOffsets, row.graphemeLengths, column);
		removeBudgetString(budget, grapheme);
		const hyperlink = budgetRowHyperlink(row, column);
		removeBudgetOptionalString(budget, hyperlink);
		removeBudgetStyle(budget, budgetRowStyleKey(row, column, hyperlink));
	}
}

function budgetRowHyperlink(row: PackedTerminalRowBacking, column: number): string | null {
	const hyperlinkOffset = row.hyperlinkOffsets[column];
	return hyperlinkOffset === undefined || hyperlinkOffset === NO_STRING_OFFSET
		? null
		: decodeRowString(row, row.hyperlinkOffsets, row.hyperlinkLengths, column);
}

function budgetRowStyleKey(
	row: PackedTerminalRowBacking,
	column: number,
	hyperlink: string | null,
): string {
	return JSON.stringify([
		row.foregroundColors[column] ?? 0,
		row.backgroundColors[column] ?? 0,
		row.underlineColors[column] ?? 0,
		row.styleFlags[column] ?? 0,
		row.underlineKinds[column] ?? 0,
		hyperlink,
	]);
}

function addBudgetOptionalString(budget: RetainedPresentationBudget, value: string | null): void {
	if (value !== null) addBudgetString(budget, value);
}

function removeBudgetOptionalString(
	budget: RetainedPresentationBudget,
	value: string | null,
): void {
	if (value !== null) removeBudgetString(budget, value);
}

function addBudgetString(budget: RetainedPresentationBudget, value: string): void {
	const occurrenceBytes = 4 + textEncoder.encode(value).byteLength;
	budget.aggregateBytes += occurrenceBytes;
	const count = budget.strings.get(value) ?? 0;
	if (count === 0) budget.encodedBytes += occurrenceBytes;
	budget.strings.set(value, count + 1);
}

function removeBudgetString(budget: RetainedPresentationBudget, value: string): void {
	const count = budget.strings.get(value);
	if (count === undefined || count < 1) throw new Error('Retained string budget is inconsistent');
	const occurrenceBytes = 4 + textEncoder.encode(value).byteLength;
	budget.aggregateBytes -= occurrenceBytes;
	if (count === 1) {
		budget.strings.delete(value);
		budget.encodedBytes -= occurrenceBytes;
		return;
	}
	budget.strings.set(value, count - 1);
}

function addBudgetStyle(budget: RetainedPresentationBudget, value: string): void {
	budget.aggregateBytes += 20;
	const count = budget.styles.get(value) ?? 0;
	if (count === 0) budget.encodedBytes += 20;
	budget.styles.set(value, count + 1);
}

function removeBudgetStyle(budget: RetainedPresentationBudget, value: string): void {
	const count = budget.styles.get(value);
	if (count === undefined || count < 1) throw new Error('Retained style budget is inconsistent');
	budget.aggregateBytes -= 20;
	if (count === 1) {
		budget.styles.delete(value);
		budget.encodedBytes -= 20;
		return;
	}
	budget.styles.set(value, count - 1);
}

function metadataExtensionBytes(metadata: ProjectedTerminalMetadata): number {
	return (
		(metadata.closeActivity === 'unknown' ? 0 : 8 + 1) +
		(metadata.progress.state === 'remove' && metadata.progress.value === null ? 0 : 8 + 2) +
		(metadata.commandCompletion === null ? 0 : 8 + 25)
	);
}

function graphicsSnapshotEncodedBytes(graphics: ProjectedTerminalGraphics): number {
	if (
		graphics.resourceRevision === 0n &&
		graphics.animationTick === 0n &&
		graphics.images.size === 0 &&
		graphics.placements.length === 0
	) {
		return 0;
	}
	return (
		8 +
		40 +
		graphics.images.size * 32 +
		graphicsByteLength(graphics) +
		graphics.placements.length * 56
	);
}

function acknowledgement(
	frame: DecodedTerminalFrame | DecodedTerminalProjectionBuffer | InspectedTerminalFrameHeader,
): ProjectedFrameAcknowledgement {
	return FrameAppliedSchema.parse({
		attachmentId: frame.attachmentId,
		generation: frame.generation.toString(),
		revision: frame.revision.toString(),
		sequence: frame.sequence.toString(),
	});
}

export class PackedTerminalStore {
	#budget: RetainedPresentationBudget | null = null;
	#connectionTerminal: ProjectedFrameConnectionTerminal | null = null;
	#fullRedraw = false;
	#graphics: ProjectedTerminalGraphics = EMPTY_TERMINAL_GRAPHICS;
	#grid: PackedGrid | null = null;
	#lastCommit: LastCommit | null = null;
	#metadata: ProjectedTerminalMetadata | null = null;
	#phase: ProjectedStorePhase = ProjectedStorePhase.empty;
	readonly attachmentId: AttachmentId;
	readonly hostEpoch: HostEpoch;

	constructor(hostEpoch: HostEpoch, attachmentId: AttachmentId) {
		this.hostEpoch = hostEpoch;
		this.attachmentId = attachmentId;
	}

	get phase(): ProjectedStorePhase {
		return this.#phase;
	}

	get metadata(): ProjectedTerminalMetadata | null {
		return this.#metadata;
	}

	get graphics(): ProjectedTerminalGraphics {
		return this.#graphics;
	}
	get rows(): readonly PackedTerminalRowView[] {
		return this.#grid?.rows ?? [];
	}

	get needsFullRedraw(): boolean {
		return this.#fullRedraw;
	}

	get byteLength(): number {
		return (
			(this.#grid?.directByteLength ??
				this.rows.reduce((total, row) => total + row.byteLength, 0)) +
			graphicsByteLength(this.#graphics)
		);
	}

	/** Canonical OBTF Snapshot bytes required to encode the retained presentation. */
	get retainedSnapshotBytes(): number {
		return this.#budget?.encodedBytes ?? 0;
	}

	/** Conservative aggregate charge for every retained presentation value occurrence. */
	get retainedAggregateBytes(): number {
		return this.#budget?.aggregateBytes ?? 0;
	}

	get lastFrameKind(): string | null {
		return this.#lastCommit?.kind ?? null;
	}

	capturePresentationSnapshot(): ProjectedTerminalPresentationSnapshot | null {
		const metadata = this.#metadata;
		const grid = this.#grid;
		if (this.#phase !== ProjectedStorePhase.synchronized || metadata === null || grid === null) {
			return null;
		}
		return createPresentationSnapshot(
			this.byteLength,
			this.#graphics,
			metadata,
			grid.rows.map((row) => row.capturePresentationRow()),
		);
	}

	apply(bytes: Uint8Array): ProjectedFrameApplyResult {
		if (this.#phase === ProjectedStorePhase.disposed) return { kind: 'disposed' };
		if (this.#connectionTerminal !== null) {
			return { kind: 'connection-terminal', reason: this.#connectionTerminal };
		}
		const inspected = inspectTerminalFrameHeader(bytes);
		if (!inspected.ok) return this.#terminateConnection({ failure: inspected, kind: 'protocol' });
		const header = inspected.value;
		if (header.hostEpoch !== this.hostEpoch || header.attachmentId !== this.attachmentId) {
			return this.#terminateConnection({
				failure: {
					code: ProtocolErrorCode.invalidIdentifier,
					detail: 'Terminal frame header cannot be associated with this presentation store',
					ok: false,
				},
				kind: 'protocol',
			});
		}
		const decoded = decodeTerminalFrame(bytes);
		if (!decoded.ok) return this.#requireSnapshot('decoder-rejected', header.sequence);
		const frame = decoded.value;
		if (this.#lastCommit && frame.sequence === this.#lastCommit.sequence) {
			if (
				frame.revision === this.#lastCommit.revision &&
				frame.generation === this.#lastCommit.generation &&
				frame.baseRevision === this.#lastCommit.baseRevision &&
				frame.kind === this.#lastCommit.kind &&
				frame.themeRevision === this.#lastCommit.themeRevision
			) {
				return { acknowledgement: acknowledgement(frame), kind: 'duplicate' };
			}
			return this.#requireSnapshot('sequence-gap', frame.sequence);
		}
		if (
			(frame.kind === TerminalFrameKind.snapshot &&
				this.#lastCommit !== null &&
				frame.sequence <= this.#lastCommit.sequence) ||
			(frame.kind === TerminalFrameKind.delta &&
				(frame.sequence === 0n || frame.sequence - 1n !== (this.#lastCommit?.sequence ?? 0n)))
		) {
			return this.#requireSnapshot('sequence-gap', frame.sequence);
		}

		if (frame.kind === TerminalFrameKind.snapshot) {
			if (this.#metadata && frame.revision < this.#metadata.revision) {
				return this.#requireSnapshot('base-mismatch', frame.sequence);
			}
			const buildContext = createPackedFrameBuildContext(frame, false);
			const stagedGraphics = graphicsFromSnapshot(frame.graphics);
			const stagedBackings = Array.from(frame.rowData.indices, (_rowIndex, rowOffset) =>
				buildRowBacking(frame, rowOffset, buildContext, false),
			);
			const stagedRows = Array.from(frame.rowData.indices, (rowIndex, rowOffset) => {
				const backing = stagedBackings[rowOffset];
				if (backing === undefined) throw new Error('Decoded frame omitted a staged row backing');
				return new PackedTerminalRowView(rowIndex, backing);
			});
			const stagedMetadata = metadataFromFrame(frame);
			const budget = createRetainedPresentationBudget(
				stagedMetadata,
				stagedBackings,
				stagedGraphics,
			);
			const limit = validateRetainedPresentationBudget(budget);
			if (limit !== null) return this.#terminateConnection(limit);
			this.#budget = budget;
			this.#grid = { backings: stagedBackings, rows: stagedRows };
			this.#graphics = stagedGraphics;
			this.#metadata = stagedMetadata;
			this.#phase = ProjectedStorePhase.synchronized;
			this.#fullRedraw = true;
			this.#recordCommit(frame);
			return { acknowledgement: acknowledgement(frame), kind: 'committed' };
		}

		if (
			this.#phase !== ProjectedStorePhase.synchronized ||
			!this.#grid ||
			!this.#metadata ||
			!this.#budget
		) {
			return this.#requireSnapshot('store-reset', frame.sequence);
		}
		if (frame.generation !== this.#metadata.generation) {
			return this.#requireSnapshot('generation-mismatch', frame.sequence);
		}
		if (
			frame.columns !== this.#metadata.columns ||
			frame.rows !== this.#metadata.rows ||
			frame.cellPixelWidth !== this.#metadata.cellPixelWidth ||
			frame.cellPixelHeight !== this.#metadata.cellPixelHeight
		) {
			return this.#requireSnapshot('dimensions-mismatch', frame.sequence);
		}
		if (frame.activeScreen !== this.#metadata.activeScreen) {
			return this.#requireSnapshot('generation-mismatch', frame.sequence);
		}
		if (frame.baseRevision !== this.#metadata.revision) {
			return this.#requireSnapshot('base-mismatch', frame.sequence);
		}
		const stagedGraphics = graphicsFromDelta(this.#graphics, frame.graphics);
		if (stagedGraphics === null) {
			return this.#requireSnapshot('base-mismatch', frame.sequence);
		}

		const buildContext = createPackedFrameBuildContext(frame, false);
		const staged = Array.from(frame.rowData.indices, (rowIndex, rowOffset) => ({
			backing: buildRowBacking(frame, rowOffset, buildContext, false),
			rowIndex,
		}));
		const candidateBackings = [...this.#grid.backings];
		const replacements = staged.map((replacement) => {
			const row = this.#grid?.rows[replacement.rowIndex];
			if (row === undefined || candidateBackings[replacement.rowIndex] === undefined) return null;
			candidateBackings[replacement.rowIndex] = replacement.backing;
			return { backing: replacement.backing, row, rowIndex: replacement.rowIndex };
		});
		if (replacements.some((replacement) => replacement === null)) {
			return this.#requireSnapshot('dimensions-mismatch', frame.sequence);
		}
		if (!candidateRowsHaveValidTopology(candidateBackings)) {
			return this.#requireSnapshot('base-mismatch', frame.sequence);
		}
		const stagedMetadata = metadataFromFrame(frame);
		const budget = cloneRetainedPresentationBudget(this.#budget);
		replaceBudgetMetadata(budget, this.#metadata, stagedMetadata);
		for (const replacement of replacements) {
			if (replacement === null) return this.#requireSnapshot('dimensions-mismatch', frame.sequence);
			const previous = this.#grid.backings[replacement.rowIndex];
			if (previous === undefined) {
				return this.#requireSnapshot('dimensions-mismatch', frame.sequence);
			}
			replaceBudgetRow(budget, previous, replacement.backing);
		}
		replaceBudgetGraphics(budget, this.#graphics, stagedGraphics);
		const limit = validateRetainedPresentationBudget(budget);
		if (limit !== null) return this.#terminateConnection(limit);
		for (const replacement of replacements) {
			if (replacement === null) return this.#requireSnapshot('dimensions-mismatch', frame.sequence);
			replacement.row.replace(replacement.backing);
			this.#grid.backings[replacement.rowIndex] = replacement.backing;
		}
		this.#graphics = stagedGraphics;
		this.#metadata = stagedMetadata;
		this.#budget = budget;
		this.#recordCommit(frame);
		return { acknowledgement: acknowledgement(frame), kind: 'committed' };
	}

	replaceSnapshot(bytes: Uint8Array, maximumBytes: number): ProjectedSnapshotReplaceResult {
		if (this.#phase === ProjectedStorePhase.disposed) return { kind: 'disposed' };
		if (this.#phase === ProjectedStorePhase.connectionTerminal) return { kind: 'invalid' };
		const decoded = decodeTerminalFrame(bytes);
		if (!decoded.ok) return { kind: 'invalid' };
		return this.#replaceSnapshot(decoded.value, maximumBytes, bytes);
	}

	/** Commits one immutable history snapshot that was already validated by the row-window decoder. */
	replaceDecodedSnapshot(
		frame: DecodedTerminalFrame,
		maximumBytes: number,
	): ProjectedSnapshotReplaceResult {
		if (this.#phase === ProjectedStorePhase.disposed) return { kind: 'disposed' };
		if (this.#phase === ProjectedStorePhase.connectionTerminal) return { kind: 'invalid' };
		return this.#replaceSnapshot(frame, maximumBytes, null);
	}

	/** Retains one immutable OBPB 0.1.0 buffer and exposes per-row typed views without repacking. */
	replaceProjectionBufferSnapshot(
		frame: DecodedTerminalProjectionBuffer,
		maximumBytes: number,
		graphics: TerminalGraphics | null = null,
	): ProjectedSnapshotReplaceResult {
		if (this.#phase === ProjectedStorePhase.disposed) return { kind: 'disposed' };
		if (this.#phase === ProjectedStorePhase.connectionTerminal) return { kind: 'invalid' };
		if (
			frame.kind !== TerminalFrameKind.snapshot ||
			frame.hostEpoch !== this.hostEpoch ||
			frame.attachmentId !== this.attachmentId
		) {
			return { kind: 'invalid' };
		}
		if (graphics !== null && graphics.kind !== TerminalGraphicsUpdateKind.snapshot) {
			return { kind: 'invalid' };
		}
		const stagedGraphics = graphicsFromSnapshot(graphics);
		const byteLength = frame.bytes.byteLength + graphicsByteLength(stagedGraphics);
		if (byteLength > maximumBytes) return { kind: 'too-large' };
		const stagedBackings = Array.from(frame.rowData.indices, (_rowIndex, rowOffset) => {
			const cellOffset = rowOffset * frame.columns;
			const cellEnd = cellOffset + frame.columns;
			return {
				backgroundColors: frame.cells.backgroundColors.subarray(cellOffset, cellEnd),
				foregroundColors: frame.cells.foregroundColors.subarray(cellOffset, cellEnd),
				graphemeLengths: frame.cells.graphemeLengths.subarray(cellOffset, cellEnd),
				graphemeOffsets: frame.cells.graphemeOffsets.subarray(cellOffset, cellEnd),
				hyperlinkLengths: frame.cells.hyperlinkLengths.subarray(cellOffset, cellEnd),
				hyperlinkOffsets: frame.cells.hyperlinkOffsets.subarray(cellOffset, cellEnd),
				protectedCells: frame.cells.protected.subarray(cellOffset, cellEnd),
				promptClass: frame.rowData.promptClasses[rowOffset] ?? 0,
				rowFlags: frame.rowData.flags[rowOffset] ?? 0,
				semanticContents: frame.cells.semanticContents.subarray(cellOffset, cellEnd),
				stringBytes: frame.stringBytes,
				styleFlags: frame.cells.styleFlags.subarray(cellOffset, cellEnd),
				underlineColors: frame.cells.underlineColors.subarray(cellOffset, cellEnd),
				underlineKinds: frame.cells.underlineKinds.subarray(cellOffset, cellEnd),
				widths: frame.cells.widths.subarray(cellOffset, cellEnd),
			};
		});
		const stagedRows = Array.from(frame.rowData.indices, (rowIndex, rowOffset) => {
			const backing = stagedBackings[rowOffset];
			if (backing === undefined) throw new Error('Projection buffer omitted a row backing');
			return new PackedTerminalRowView(rowIndex, backing);
		});
		const stagedMetadata = metadataFromProjectionBuffer(frame);
		const budget = createRetainedPresentationBudget(stagedMetadata, stagedBackings, stagedGraphics);
		if (validateRetainedPresentationBudget(budget) !== null) return { kind: 'too-large' };
		this.#grid = {
			backings: stagedBackings,
			directByteLength: frame.bytes.byteLength,
			rows: stagedRows,
		};
		this.#graphics = stagedGraphics;
		this.#metadata = stagedMetadata;
		this.#budget = budget;
		this.#phase = ProjectedStorePhase.synchronized;
		this.#fullRedraw = true;
		this.#lastCommit = {
			baseRevision: frame.baseRevision,
			generation: frame.generation,
			kind: frame.kind,
			revision: frame.revision,
			sequence: frame.sequence,
			themeRevision: frame.themeRevision,
		};
		return {
			acknowledgement: acknowledgement(frame),
			byteLength,
			kind: 'committed',
		};
	}

	#replaceSnapshot(
		frame: DecodedTerminalFrame,
		maximumBytes: number,
		sourceBytes: Uint8Array | null,
	): ProjectedSnapshotReplaceResult {
		if (
			frame.kind !== TerminalFrameKind.snapshot ||
			frame.hostEpoch !== this.hostEpoch ||
			frame.attachmentId !== this.attachmentId
		) {
			return { kind: 'invalid' };
		}
		const buildContext = createPackedFrameBuildContext(frame, sourceBytes === null);
		const stagedGraphics = graphicsFromSnapshot(frame.graphics);
		const stagedBackings = Array.from(frame.rowData.indices, (_rowIndex, rowOffset) =>
			buildRowBacking(frame, rowOffset, buildContext, sourceBytes === null),
		);
		const stagedRows = Array.from(frame.rowData.indices, (rowIndex, rowOffset) => {
			const backing = stagedBackings[rowOffset];
			if (backing === undefined) throw new Error('Decoded snapshot omitted a row backing');
			return new PackedTerminalRowView(rowIndex, backing);
		});
		const byteLength =
			stagedRows.reduce((total, row) => total + row.byteLength, 0) +
			graphicsByteLength(stagedGraphics);
		if (byteLength > maximumBytes) return { kind: 'too-large' };
		const stagedMetadata = metadataFromFrame(frame);
		const budget = createRetainedPresentationBudget(stagedMetadata, stagedBackings, stagedGraphics);
		if (validateRetainedPresentationBudget(budget) !== null) return { kind: 'too-large' };
		this.#grid = { backings: stagedBackings, rows: stagedRows };
		this.#graphics = stagedGraphics;
		this.#metadata = stagedMetadata;
		this.#budget = budget;
		this.#phase = ProjectedStorePhase.synchronized;
		this.#fullRedraw = true;
		this.#recordCommit(frame);
		return { acknowledgement: acknowledgement(frame), byteLength, kind: 'committed' };
	}

	markRendered(): void {
		this.#fullRedraw = false;
		for (const row of this.#grid?.rows ?? []) row.markClean();
	}

	dispose(): void {
		this.#budget = null;
		this.#connectionTerminal = null;
		this.#grid = null;
		this.#graphics = EMPTY_TERMINAL_GRAPHICS;
		this.#metadata = null;
		this.#lastCommit = null;
		this.#fullRedraw = false;
		this.#phase = ProjectedStorePhase.disposed;
	}

	#recordCommit(frame: DecodedTerminalFrame): void {
		this.#lastCommit = {
			baseRevision: frame.baseRevision,
			generation: frame.generation,
			kind: frame.kind,
			revision: frame.revision,
			sequence: frame.sequence,
			themeRevision: frame.themeRevision,
		};
	}

	#requireSnapshot(reason: NeedSnapshot['reason'], sequence: bigint): ProjectedFrameApplyResult {
		this.#phase = ProjectedStorePhase.snapshotRequired;
		return {
			kind: 'snapshot-required',
			request: {
				attachmentId: this.attachmentId,
				reason,
				sequence: CanonicalUint64TextSchema.parse(sequence.toString()),
			},
		};
	}

	#terminateConnection(reason: ProjectedFrameConnectionTerminal): ProjectedFrameApplyResult {
		this.#connectionTerminal = reason;
		this.#phase = ProjectedStorePhase.connectionTerminal;
		return { kind: 'connection-terminal', reason };
	}
}

/** Bounded visible slice over one immutable packed history-window commit. */
export class PackedTerminalViewport {
	readonly #graphics: ProjectedTerminalGraphics;
	readonly #metadata: ProjectedTerminalMetadata | null;
	readonly #rows: readonly PackedTerminalRowView[];
	readonly #store: PackedTerminalStore;

	constructor(
		store: PackedTerminalStore,
		rowOffset: number,
		visibleRows: number,
		metadataOverride?: ProjectedTerminalMetadata,
	) {
		if (!Number.isInteger(rowOffset) || rowOffset < 0) {
			throw new RangeError('Packed terminal viewport row offset must be non-negative.');
		}
		if (!Number.isInteger(visibleRows) || visibleRows < 1) {
			throw new RangeError('Packed terminal viewport must contain at least one row.');
		}
		this.#store = store;
		this.#rows = store.rows.slice(rowOffset, rowOffset + visibleRows);
		this.#graphics = {
			...store.graphics,
			placements: store.graphics.placements.map((placement) => ({
				...placement,
				row: placement.row - rowOffset,
			})),
		};
		if (metadataOverride !== undefined && metadataOverride.rows !== visibleRows) {
			throw new RangeError('Packed terminal viewport metadata must match its visible row count.');
		}
		const metadata = store.metadata;
		this.#metadata =
			metadataOverride ??
			(metadata === null
				? null
				: {
						...metadata,
						cursor: {
							...metadata.cursor,
							row: metadata.cursor.row - rowOffset,
							visible:
								metadata.cursor.visible &&
								metadata.cursor.row >= rowOffset &&
								metadata.cursor.row < rowOffset + visibleRows,
						},
						rows: visibleRows,
						scrollbackRows: metadata.scrollbackRows + BigInt(rowOffset),
					});
	}

	get metadata(): ProjectedTerminalMetadata | null {
		return this.#metadata;
	}

	get graphics(): ProjectedTerminalGraphics {
		return this.#graphics;
	}

	get rows(): readonly PackedTerminalRowView[] {
		return this.#rows;
	}

	get needsFullRedraw(): boolean {
		return this.#store.needsFullRedraw;
	}

	get byteLength(): number {
		return this.#store.byteLength;
	}

	capturePresentationSnapshot(): ProjectedTerminalPresentationSnapshot | null {
		const metadata = this.#metadata;
		if (metadata === null) return null;
		return createPresentationSnapshot(
			this.byteLength,
			this.#graphics,
			metadata,
			this.#rows.map((row) => row.capturePresentationRow()),
		);
	}

	markRendered(): void {
		this.#store.markRendered();
	}

	dispose(): void {
		this.#store.dispose();
	}
}

function createPresentationSnapshot(
	byteLength: number,
	graphics: ProjectedTerminalGraphics,
	metadata: ProjectedTerminalMetadata,
	rows: readonly PackedRendererRow[],
): ProjectedTerminalPresentationSnapshot {
	return Object.freeze({
		byteLength,
		graphics,
		markRendered: () => undefined,
		metadata,
		needsFullRedraw: true,
		rows: Object.freeze(rows),
	});
}

export function projectedCellDisplayWidth(width: number): 0 | 1 | 2 {
	if (width === TerminalCellWidthCode.wide) return 2;
	if (width === TerminalCellWidthCode.narrow) return 1;
	return 0;
}
