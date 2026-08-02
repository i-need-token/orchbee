import {
	PROTOCOL_LIMITS,
	RowPromptClass,
	SemanticContent,
	TerminalCellWidthCode,
	TerminalRowFlag,
	TerminalRowPromptClassCode,
	TerminalSemanticContentCode,
	type DecodedTerminalRowWindow,
	type RowPromptClass as RowPromptClassValue,
	type SemanticContent as SemanticContentValue,
} from '@orchbee/terminal-protocol';

const textEncoder = new TextEncoder();

export const TerminalBufferRowDetail = {
	render: 'render',
	text: 'text',
} as const;

export type TerminalBufferRowDetail =
	(typeof TerminalBufferRowDetail)[keyof typeof TerminalBufferRowDetail];

export interface TerminalBufferRowsRequest {
	readonly detail: TerminalBufferRowDetail;
	readonly generation: bigint;
	readonly maximumBytes: number;
	readonly purpose?: 'buffer' | 'search';
	readonly revision: bigint;
	readonly rowCount: number;
	readonly startRow: bigint;
}

export interface TerminalBufferRowsSnapshot {
	readonly generation: bigint;
	readonly revision: bigint;
	readonly totalRows: bigint;
}

export interface TerminalBufferCellMapping {
	readonly byteOffset: number;
	readonly column: number;
	readonly utf16Offset: number;
	readonly width: 1 | 2;
}

export interface TerminalBufferRenderCell {
	readonly column: number;
	readonly grapheme: string;
	readonly hyperlink: string | null;
	readonly semanticContent: SemanticContentValue;
	readonly width: 0 | 1 | 2;
}

export interface TerminalBufferRow {
	readonly cells: readonly TerminalBufferRenderCell[] | null;
	/** Narrow ASCII text whose byte, UTF-16, and column offsets are all the character index. */
	readonly identityAscii?: true;
	readonly index: bigint;
	readonly mappings: readonly TerminalBufferCellMapping[];
	readonly promptClass: RowPromptClassValue;
	readonly text: string;
	readonly wrap: boolean;
	readonly wrapContinuation: boolean;
}

export interface TerminalBufferRows {
	readonly byteLength: number;
	readonly detail: TerminalBufferRowDetail;
	readonly generation: bigint;
	readonly kind: 'rows';
	readonly revision: bigint;
	readonly rows: readonly TerminalBufferRow[];
	readonly startRow: bigint;
	readonly totalRows: bigint;
}

export type BufferRowsResult =
	| TerminalBufferRows
	| {
			readonly kind: 'cancelled' | 'evicted' | 'stale' | 'too-large' | 'unavailable';
			readonly generation: bigint;
			readonly revision: bigint;
			readonly totalRows: bigint;
	  };

export interface TerminalBufferRowReader {
	getBufferRowsSnapshot(): TerminalBufferRowsSnapshot;
	readBufferRows(
		request: TerminalBufferRowsRequest,
		signal: AbortSignal,
	): Promise<BufferRowsResult>;
}

export function validateBufferRowsRequest(request: TerminalBufferRowsRequest): boolean {
	return (
		(request.detail === TerminalBufferRowDetail.render ||
			request.detail === TerminalBufferRowDetail.text) &&
		(request.purpose === undefined ||
			request.purpose === 'buffer' ||
			request.purpose === 'search') &&
		request.generation > 0n &&
		request.revision >= 0n &&
		request.startRow >= 0n &&
		Number.isInteger(request.rowCount) &&
		request.rowCount >= 1 &&
		request.rowCount <= PROTOCOL_LIMITS.maxRowWindowRows &&
		Number.isInteger(request.maximumBytes) &&
		request.maximumBytes >= 1 &&
		request.maximumBytes <= PROTOCOL_LIMITS.maxRowWindowBytes
	);
}

export function bufferRowsFromDecodedWindow(
	window: DecodedTerminalRowWindow,
	byteLength: number,
): TerminalBufferRows {
	if (window.detail === TerminalBufferRowDetail.text) {
		return {
			byteLength,
			detail: TerminalBufferRowDetail.text,
			generation: window.generation,
			kind: 'rows',
			revision: window.revision,
			rows: window.rows.map((row) => ({
				cells: null,
				...(row.identityAscii ? { identityAscii: true as const } : {}),
				index: row.index,
				mappings: row.identityAscii
					? []
					: Array.from(row.columns, (column, index) => ({
							byteOffset: row.byteOffsets[index] ?? 0,
							column,
							utf16Offset: row.utf16Offsets[index] ?? 0,
							width: row.widths[index] === 2 ? 2 : 1,
						})),
				promptClass: row.promptClass,
				text: row.text,
				wrap: row.wrap,
				wrapContinuation: row.wrapContinuation,
			})),
			startRow: window.startRow,
			totalRows: window.totalRows,
		};
	}

	const frame = window.frame;
	const rows = Array.from(frame.rowData.indices, (_rowIndex, rowOffset) => {
		const cellOffset = frame.rowData.cellOffsets[rowOffset] ?? 0;
		const cells = Array.from({ length: frame.columns }, (_, column) => {
			const absolute = cellOffset + column;
			const stringIndex = frame.cells.graphemeStringIndices[absolute] ?? 0;
			const styleIndex = frame.cells.styleIndices[absolute] ?? 0;
			const hyperlinkIndex = frame.styles.hyperlinkStringIndices[styleIndex] ?? 0xffff_ffff;
			return {
				column,
				grapheme: frame.strings[stringIndex] ?? '',
				hyperlink: hyperlinkIndex === 0xffff_ffff ? null : (frame.strings[hyperlinkIndex] ?? null),
				semanticContent: semanticContent(frame.cells.semanticContents[absolute] ?? 0),
				width: displayWidth(frame.cells.widths[absolute] ?? TerminalCellWidthCode.narrow),
			} satisfies TerminalBufferRenderCell;
		});
		return buildBufferRow(
			window.startRow + BigInt(rowOffset),
			cells,
			promptClass(frame.rowData.promptClasses[rowOffset] ?? 0),
			((frame.rowData.flags[rowOffset] ?? 0) & TerminalRowFlag.wrap) !== 0,
			((frame.rowData.flags[rowOffset] ?? 0) & TerminalRowFlag.wrapContinuation) !== 0,
			true,
		);
	});
	return {
		byteLength,
		detail: TerminalBufferRowDetail.render,
		generation: window.generation,
		kind: 'rows',
		revision: window.revision,
		rows,
		startRow: window.startRow,
		totalRows: window.totalRows,
	};
}

function buildBufferRow(
	index: bigint,
	cells: readonly TerminalBufferRenderCell[],
	promptClassValue: RowPromptClassValue,
	wrap: boolean,
	wrapContinuation: boolean,
	includeCells: boolean,
): TerminalBufferRow {
	let lastContentColumn = -1;
	for (const cell of cells) {
		if (cell.width > 0 && (cell.grapheme !== '' || cell.hyperlink !== null)) {
			lastContentColumn = cell.column;
		}
	}
	let byteOffset = 0;
	let utf16Offset = 0;
	let text = '';
	const mappings: TerminalBufferCellMapping[] = [];
	for (const cell of cells) {
		if (cell.column > lastContentColumn || cell.width === 0) continue;
		const grapheme = cell.grapheme || ' ';
		mappings.push({ byteOffset, column: cell.column, utf16Offset, width: cell.width });
		text += grapheme;
		byteOffset += textEncoder.encode(grapheme).byteLength;
		utf16Offset += grapheme.length;
	}
	return {
		cells: includeCells ? cells : null,
		index,
		mappings,
		promptClass: promptClassValue,
		text,
		wrap,
		wrapContinuation,
	};
}

function displayWidth(width: number): 0 | 1 | 2 {
	if (width === TerminalCellWidthCode.wide) return 2;
	if (width === TerminalCellWidthCode.narrow) return 1;
	return 0;
}

function promptClass(value: number): RowPromptClassValue {
	if (value === TerminalRowPromptClassCode.prompt) return RowPromptClass.prompt;
	if (value === TerminalRowPromptClassCode.promptContinuation) {
		return RowPromptClass.promptContinuation;
	}
	return RowPromptClass.none;
}

function semanticContent(value: number): SemanticContentValue {
	if (value === TerminalSemanticContentCode.input) {
		return SemanticContent.input;
	}
	if (value === TerminalSemanticContentCode.prompt) {
		return SemanticContent.prompt;
	}
	return SemanticContent.output;
}
