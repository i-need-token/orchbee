import {
	decodeTerminalFrame,
	CanonicalUint64TextSchema,
	FrameAppliedSchema,
	inspectTerminalFrameHeader,
	TerminalCellWidthCode,
	TerminalFrameKind,
	type ActiveScreenValue,
	type AttachmentId,
	type CursorShapeValue,
	type DecodedTerminalFrame,
	type DecodedTerminalProjectionBuffer,
	type FrameApplied,
	type HostEpoch,
	type InspectedTerminalFrameHeader,
	type NeedSnapshot,
	type TerminalInteraction,
} from '@orchbee/terminal-protocol';

const NO_STRING_OFFSET = 0xffff_ffff;
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

export const ProjectedStorePhase = {
	disposed: 'disposed',
	empty: 'empty',
	snapshotRequired: 'snapshot-required',
	synchronized: 'synchronized',
} as const;

export type ProjectedStorePhase = (typeof ProjectedStorePhase)[keyof typeof ProjectedStorePhase];

export type ProjectedFrameAcknowledgement = FrameApplied;

export type ProjectedFrameApplyResult =
	| {
			readonly acknowledgement: ProjectedFrameAcknowledgement;
			readonly kind: 'committed' | 'duplicate';
	  }
	| { readonly kind: 'snapshot-required'; readonly request: NeedSnapshot }
	| { readonly kind: 'disposed' };

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
	readonly revision: bigint;
	readonly rows: number;
	readonly scrollbackRows: bigint;
	readonly title: string | null;
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

	grapheme(column: number): string {
		return decodeRowString(
			this.#backing,
			this.#backing.graphemeOffsets,
			this.#backing.graphemeLengths,
			column,
		);
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
		this.#dirty = true;
	}
}

interface PackedGrid {
	readonly directByteLength?: number;
	readonly rows: readonly PackedTerminalRowView[];
}

interface LastCommit {
	readonly baseRevision: bigint | null;
	readonly generation: bigint;
	readonly kind: string;
	readonly revision: bigint;
	readonly sequence: bigint;
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
		currentDirectory: frame.currentDirectory,
		cursor: frame.cursor,
		defaultColors: frame.defaultColors,
		generation: frame.generation,
		interaction: frame.interaction,
		revision: frame.revision,
		rows: frame.rows,
		scrollbackRows: frame.scrollbackRows,
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
		currentDirectory: frame.currentDirectory,
		cursor: frame.cursor,
		defaultColors: frame.defaultColors,
		generation: frame.generation,
		interaction: frame.interaction,
		revision: frame.revision,
		rows: frame.rows,
		scrollbackRows: frame.scrollbackRows,
		title: frame.title,
	};
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
	#fullRedraw = false;
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

	get rows(): readonly PackedTerminalRowView[] {
		return this.#grid?.rows ?? [];
	}

	get needsFullRedraw(): boolean {
		return this.#fullRedraw;
	}

	get byteLength(): number {
		return (
			this.#grid?.directByteLength ?? this.rows.reduce((total, row) => total + row.byteLength, 0)
		);
	}

	get lastFrameKind(): string | null {
		return this.#lastCommit?.kind ?? null;
	}

	apply(bytes: Uint8Array): ProjectedFrameApplyResult {
		if (this.#phase === ProjectedStorePhase.disposed) return { kind: 'disposed' };
		const inspected = inspectTerminalFrameHeader(bytes);
		if (!inspected.ok) return this.#requireSnapshot('decoder-rejected', this.#expectedSequence());
		const header = inspected.value;
		if (header.hostEpoch !== this.hostEpoch || header.attachmentId !== this.attachmentId) {
			return this.#requireSnapshot('identity-mismatch', header.sequence);
		}
		if (this.#lastCommit && header.sequence === this.#lastCommit.sequence) {
			if (
				header.revision === this.#lastCommit.revision &&
				header.generation === this.#lastCommit.generation &&
				header.baseRevision === this.#lastCommit.baseRevision &&
				header.kind === this.#lastCommit.kind
			) {
				return { acknowledgement: acknowledgement(header), kind: 'duplicate' };
			}
			return this.#requireSnapshot('sequence-gap', header.sequence);
		}

		const decoded = decodeTerminalFrame(bytes);
		if (!decoded.ok) return this.#requireSnapshot('decoder-rejected', this.#expectedSequence());
		const frame = decoded.value;
		const expectedSequence = this.#expectedSequence();
		if (
			(frame.kind === TerminalFrameKind.snapshot && frame.sequence < expectedSequence) ||
			(frame.kind === TerminalFrameKind.delta && frame.sequence !== expectedSequence)
		) {
			return this.#requireSnapshot('sequence-gap', frame.sequence);
		}

		if (frame.kind === TerminalFrameKind.snapshot) {
			if (this.#metadata && frame.revision < this.#metadata.revision) {
				return this.#requireSnapshot('base-mismatch', frame.sequence);
			}
			const buildContext = createPackedFrameBuildContext(frame, false);
			const stagedRows = Array.from(frame.rowData.indices, (rowIndex, rowOffset) => {
				const backing = buildRowBacking(frame, rowOffset, buildContext, false);
				return new PackedTerminalRowView(rowIndex, backing);
			});
			this.#grid = { rows: stagedRows };
			this.#metadata = metadataFromFrame(frame);
			this.#phase = ProjectedStorePhase.synchronized;
			this.#fullRedraw = true;
			this.#recordCommit(frame);
			return { acknowledgement: acknowledgement(frame), kind: 'committed' };
		}

		if (this.#phase !== ProjectedStorePhase.synchronized || !this.#grid || !this.#metadata) {
			return this.#requireSnapshot('store-reset', frame.sequence);
		}
		if (frame.generation !== this.#metadata.generation) {
			return this.#requireSnapshot('generation-mismatch', frame.sequence);
		}
		if (frame.columns !== this.#metadata.columns || frame.rows !== this.#metadata.rows) {
			return this.#requireSnapshot('dimensions-mismatch', frame.sequence);
		}
		if (frame.activeScreen !== this.#metadata.activeScreen) {
			return this.#requireSnapshot('generation-mismatch', frame.sequence);
		}
		if (frame.baseRevision !== this.#metadata.revision) {
			return this.#requireSnapshot('base-mismatch', frame.sequence);
		}

		const buildContext = createPackedFrameBuildContext(frame, false);
		const staged = Array.from(frame.rowData.indices, (rowIndex, rowOffset) => ({
			backing: buildRowBacking(frame, rowOffset, buildContext, false),
			rowIndex,
		}));
		for (const replacement of staged) {
			const row = this.#grid.rows[replacement.rowIndex];
			if (!row) return this.#requireSnapshot('dimensions-mismatch', frame.sequence);
			row.replace(replacement.backing);
		}
		this.#metadata = metadataFromFrame(frame);
		this.#recordCommit(frame);
		return { acknowledgement: acknowledgement(frame), kind: 'committed' };
	}

	replaceSnapshot(bytes: Uint8Array, maximumBytes: number): ProjectedSnapshotReplaceResult {
		if (this.#phase === ProjectedStorePhase.disposed) return { kind: 'disposed' };
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
		return this.#replaceSnapshot(frame, maximumBytes, null);
	}

	/** Retains one immutable OBPB 0.0.1 buffer and exposes per-row typed views without repacking. */
	replaceProjectionBufferSnapshot(
		frame: DecodedTerminalProjectionBuffer,
		maximumBytes: number,
	): ProjectedSnapshotReplaceResult {
		if (this.#phase === ProjectedStorePhase.disposed) return { kind: 'disposed' };
		if (
			frame.kind !== TerminalFrameKind.snapshot ||
			frame.hostEpoch !== this.hostEpoch ||
			frame.attachmentId !== this.attachmentId
		) {
			return { kind: 'invalid' };
		}
		if (frame.bytes.byteLength > maximumBytes) return { kind: 'too-large' };
		const stagedRows = Array.from(frame.rowData.indices, (rowIndex, rowOffset) => {
			const cellOffset = rowOffset * frame.columns;
			const cellEnd = cellOffset + frame.columns;
			return new PackedTerminalRowView(rowIndex, {
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
			});
		});
		this.#grid = { directByteLength: frame.bytes.byteLength, rows: stagedRows };
		this.#metadata = metadataFromProjectionBuffer(frame);
		this.#phase = ProjectedStorePhase.synchronized;
		this.#fullRedraw = true;
		this.#lastCommit = {
			baseRevision: frame.baseRevision,
			generation: frame.generation,
			kind: frame.kind,
			revision: frame.revision,
			sequence: frame.sequence,
		};
		return {
			acknowledgement: acknowledgement(frame),
			byteLength: frame.bytes.byteLength,
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
		const stagedRows = Array.from(frame.rowData.indices, (rowIndex, rowOffset) => {
			const backing = buildRowBacking(frame, rowOffset, buildContext, sourceBytes === null);
			return new PackedTerminalRowView(rowIndex, backing);
		});
		const byteLength = stagedRows.reduce((total, row) => total + row.byteLength, 0);
		if (byteLength > maximumBytes) return { kind: 'too-large' };
		this.#grid = { rows: stagedRows };
		this.#metadata = metadataFromFrame(frame);
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
		this.#grid = null;
		this.#metadata = null;
		this.#lastCommit = null;
		this.#fullRedraw = false;
		this.#phase = ProjectedStorePhase.disposed;
	}

	#expectedSequence(): bigint {
		return (this.#lastCommit?.sequence ?? 0n) + 1n;
	}

	#recordCommit(frame: DecodedTerminalFrame): void {
		this.#lastCommit = {
			baseRevision: frame.baseRevision,
			generation: frame.generation,
			kind: frame.kind,
			revision: frame.revision,
			sequence: frame.sequence,
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
}

/** Bounded visible slice over one immutable packed history-window commit. */
export class PackedTerminalViewport {
	readonly #metadata: ProjectedTerminalMetadata | null;
	readonly #rows: readonly PackedTerminalRowView[];
	readonly #store: PackedTerminalStore;

	constructor(store: PackedTerminalStore, rowOffset: number, visibleRows: number) {
		if (!Number.isInteger(rowOffset) || rowOffset < 0) {
			throw new RangeError('Packed terminal viewport row offset must be non-negative.');
		}
		if (!Number.isInteger(visibleRows) || visibleRows < 1) {
			throw new RangeError('Packed terminal viewport must contain at least one row.');
		}
		this.#store = store;
		this.#rows = store.rows.slice(rowOffset, rowOffset + visibleRows);
		const metadata = store.metadata;
		this.#metadata =
			metadata === null
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
					};
	}

	get metadata(): ProjectedTerminalMetadata | null {
		return this.#metadata;
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

	markRendered(): void {
		this.#store.markRendered();
	}

	dispose(): void {
		this.#store.dispose();
	}
}

export function projectedCellDisplayWidth(width: number): number {
	if (width === TerminalCellWidthCode.wide) return 2;
	if (width === TerminalCellWidthCode.narrow) return 1;
	return 0;
}
