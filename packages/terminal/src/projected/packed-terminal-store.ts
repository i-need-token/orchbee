import {
	decodeTerminalFrame,
	CanonicalUint64TextSchema,
	FrameAppliedSchema,
	TerminalCellWidthCode,
	TerminalFrameKind,
	type ActiveScreenValue,
	type AttachmentId,
	type CursorShapeValue,
	type DecodedTerminalFrame,
	type FrameApplied,
	type HostEpoch,
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
	readonly graphemeLengths: Uint32Array;
	readonly graphemeOffsets: Uint32Array;
	readonly hyperlinkLengths: Uint32Array;
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
	readonly rows: readonly PackedTerminalRowView[];
}

interface LastCommit {
	readonly baseRevision: bigint | null;
	readonly fingerprint: readonly [number, number, number, number];
	readonly generation: bigint;
	readonly kind: string;
	readonly revision: bigint;
	readonly sequence: bigint;
}

function decodeRowString(
	backing: PackedTerminalRowBacking,
	offsets: Uint32Array,
	lengths: Uint32Array,
	column: number,
): string {
	const offset = offsets[column];
	const length = lengths[column];
	if (offset === undefined || length === undefined || offset === NO_STRING_OFFSET || length === 0) {
		return '';
	}
	return textDecoder.decode(backing.stringBytes.subarray(offset, offset + length));
}

function fingerprint(bytes: Uint8Array): readonly [number, number, number, number] {
	let first = 0x811c_9dc5;
	let second = 0x9e37_79b9;
	let third = 0x85eb_ca6b;
	let fourth = 0xc2b2_ae35;
	for (const byte of bytes) {
		first = Math.imul(first ^ byte, 0x0100_0193) >>> 0;
		second = Math.imul(second ^ byte, 0x27d4_eb2d) >>> 0;
		third = Math.imul(third ^ byte, 0x1656_67b1) >>> 0;
		fourth = Math.imul(fourth ^ byte, 0x9e37_79b1) >>> 0;
	}
	return [first, second, third, fourth];
}

function equalFingerprint(
	left: readonly [number, number, number, number],
	right: readonly [number, number, number, number],
): boolean {
	return left.every((value, index) => value === right[index]);
}

function buildRowBacking(frame: DecodedTerminalFrame, rowOffset: number): PackedTerminalRowBacking {
	const cellOffset = frame.rowData.cellOffsets[rowOffset];
	if (cellOffset === undefined) throw new Error('Decoded frame omitted a row cell offset');
	const columns = frame.columns;
	const foregroundColors = new Uint32Array(columns);
	const backgroundColors = new Uint32Array(columns);
	const underlineColors = new Uint32Array(columns);
	const styleFlags = new Uint16Array(columns);
	const underlineKinds = new Uint8Array(columns);
	const widths = frame.cells.widths.slice(cellOffset, cellOffset + columns);
	const semanticContents = frame.cells.semanticContents.slice(cellOffset, cellOffset + columns);
	const protectedCells = frame.cells.protected.slice(cellOffset, cellOffset + columns);
	const graphemeOffsets = new Uint32Array(columns);
	const graphemeLengths = new Uint32Array(columns);
	const hyperlinkOffsets = new Uint32Array(columns);
	const hyperlinkLengths = new Uint32Array(columns);
	graphemeOffsets.fill(NO_STRING_OFFSET);
	hyperlinkOffsets.fill(NO_STRING_OFFSET);

	const encoded = new Map<number, Uint8Array>();
	const referencedStrings = new Set<number>();
	for (let column = 0; column < columns; column += 1) {
		const absolute = cellOffset + column;
		const graphemeIndex = frame.cells.graphemeStringIndices[absolute];
		const styleIndex = frame.cells.styleIndices[absolute];
		if (graphemeIndex === undefined || styleIndex === undefined) {
			throw new Error('Decoded frame omitted a cell table value');
		}
		referencedStrings.add(graphemeIndex);
		const hyperlinkIndex = frame.styles.hyperlinkStringIndices[styleIndex];
		if (hyperlinkIndex !== undefined && hyperlinkIndex !== NO_STRING_OFFSET) {
			referencedStrings.add(hyperlinkIndex);
		}
	}

	let byteLength = 0;
	for (const index of referencedStrings) {
		const value = frame.strings[index];
		if (value === undefined) throw new Error('Decoded frame references a missing string');
		const bytes = textEncoder.encode(value);
		encoded.set(index, bytes);
		byteLength += bytes.byteLength;
	}
	const stringBytes = new Uint8Array(byteLength);
	const locations = new Map<number, readonly [number, number]>();
	let nextOffset = 0;
	for (const [index, bytes] of encoded) {
		stringBytes.set(bytes, nextOffset);
		locations.set(index, [nextOffset, bytes.byteLength]);
		nextOffset += bytes.byteLength;
	}

	for (let column = 0; column < columns; column += 1) {
		const absolute = cellOffset + column;
		const graphemeIndex = frame.cells.graphemeStringIndices[absolute];
		const styleIndex = frame.cells.styleIndices[absolute];
		if (graphemeIndex === undefined || styleIndex === undefined) {
			throw new Error('Decoded frame omitted a cell table value');
		}
		const graphemeLocation = locations.get(graphemeIndex);
		if (!graphemeLocation) throw new Error('Decoded frame omitted a grapheme string');
		graphemeOffsets[column] = graphemeLocation[0];
		graphemeLengths[column] = graphemeLocation[1];
		foregroundColors[column] = frame.styles.foregroundColors[styleIndex] ?? 0;
		backgroundColors[column] = frame.styles.backgroundColors[styleIndex] ?? 0;
		underlineColors[column] = frame.styles.underlineColors[styleIndex] ?? 0;
		styleFlags[column] = frame.styles.flags[styleIndex] ?? 0;
		underlineKinds[column] = frame.styles.underlineKinds[styleIndex] ?? 0;
		const hyperlinkIndex = frame.styles.hyperlinkStringIndices[styleIndex];
		if (hyperlinkIndex !== undefined && hyperlinkIndex !== NO_STRING_OFFSET) {
			const hyperlinkLocation = locations.get(hyperlinkIndex);
			if (!hyperlinkLocation) throw new Error('Decoded frame omitted a hyperlink string');
			hyperlinkOffsets[column] = hyperlinkLocation[0];
			hyperlinkLengths[column] = hyperlinkLocation[1];
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

function acknowledgement(frame: DecodedTerminalFrame): ProjectedFrameAcknowledgement {
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
		return this.rows.reduce((total, row) => total + row.byteLength, 0);
	}

	get lastFrameKind(): string | null {
		return this.#lastCommit?.kind ?? null;
	}

	apply(bytes: Uint8Array): ProjectedFrameApplyResult {
		if (this.#phase === ProjectedStorePhase.disposed) return { kind: 'disposed' };
		const decoded = decodeTerminalFrame(bytes);
		if (!decoded.ok) return this.#requireSnapshot('decoder-rejected', this.#expectedSequence());
		const frame = decoded.value;
		if (frame.hostEpoch !== this.hostEpoch || frame.attachmentId !== this.attachmentId) {
			return this.#requireSnapshot('identity-mismatch', frame.sequence);
		}

		const incomingFingerprint = fingerprint(bytes);
		if (this.#lastCommit && frame.sequence === this.#lastCommit.sequence) {
			if (
				frame.revision === this.#lastCommit.revision &&
				frame.generation === this.#lastCommit.generation &&
				frame.baseRevision === this.#lastCommit.baseRevision &&
				frame.kind === this.#lastCommit.kind &&
				equalFingerprint(incomingFingerprint, this.#lastCommit.fingerprint)
			) {
				return { acknowledgement: acknowledgement(frame), kind: 'duplicate' };
			}
			return this.#requireSnapshot('sequence-gap', frame.sequence);
		}
		if (frame.sequence !== this.#expectedSequence()) {
			return this.#requireSnapshot('sequence-gap', frame.sequence);
		}

		if (frame.kind === TerminalFrameKind.snapshot) {
			if (this.#metadata && frame.revision < this.#metadata.revision) {
				return this.#requireSnapshot('base-mismatch', frame.sequence);
			}
			const stagedRows = Array.from(frame.rowData.indices, (rowIndex, rowOffset) => {
				const backing = buildRowBacking(frame, rowOffset);
				return new PackedTerminalRowView(rowIndex, backing);
			});
			this.#grid = { rows: stagedRows };
			this.#metadata = metadataFromFrame(frame);
			this.#phase = ProjectedStorePhase.synchronized;
			this.#fullRedraw = true;
			this.#recordCommit(frame, incomingFingerprint);
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

		const staged = Array.from(frame.rowData.indices, (rowIndex, rowOffset) => ({
			backing: buildRowBacking(frame, rowOffset),
			rowIndex,
		}));
		for (const replacement of staged) {
			const row = this.#grid.rows[replacement.rowIndex];
			if (!row) return this.#requireSnapshot('dimensions-mismatch', frame.sequence);
			row.replace(replacement.backing);
		}
		this.#metadata = metadataFromFrame(frame);
		this.#recordCommit(frame, incomingFingerprint);
		return { acknowledgement: acknowledgement(frame), kind: 'committed' };
	}

	replaceSnapshot(bytes: Uint8Array, maximumBytes: number): ProjectedSnapshotReplaceResult {
		if (this.#phase === ProjectedStorePhase.disposed) return { kind: 'disposed' };
		const decoded = decodeTerminalFrame(bytes);
		if (!decoded.ok) return { kind: 'invalid' };
		const frame = decoded.value;
		if (
			frame.kind !== TerminalFrameKind.snapshot ||
			frame.hostEpoch !== this.hostEpoch ||
			frame.attachmentId !== this.attachmentId
		) {
			return { kind: 'invalid' };
		}
		const stagedRows = Array.from(frame.rowData.indices, (rowIndex, rowOffset) => {
			const backing = buildRowBacking(frame, rowOffset);
			return new PackedTerminalRowView(rowIndex, backing);
		});
		const byteLength = stagedRows.reduce((total, row) => total + row.byteLength, 0);
		if (byteLength > maximumBytes) return { kind: 'too-large' };
		this.#grid = { rows: stagedRows };
		this.#metadata = metadataFromFrame(frame);
		this.#phase = ProjectedStorePhase.synchronized;
		this.#fullRedraw = true;
		this.#recordCommit(frame, fingerprint(bytes));
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

	#recordCommit(
		frame: DecodedTerminalFrame,
		frameFingerprint: readonly [number, number, number, number],
	): void {
		this.#lastCommit = {
			baseRevision: frame.baseRevision,
			fingerprint: frameFingerprint,
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
