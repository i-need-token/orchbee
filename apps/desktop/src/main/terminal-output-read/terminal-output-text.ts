import type { ActiveScreen, DecodedTerminalTextRow } from '@orchbee/terminal-protocol';
import {
	WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES,
	WorkbenchTerminalOutputReadOutputSchema,
	type WorkbenchTerminalOutputReadOutput,
} from '@orchbee/workbench/semantic-commands';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

export interface TerminalOutputTextProjection {
	readonly activeScreen: ActiveScreen;
	readonly generation: bigint;
	readonly presentationId: TerminalPresentationId;
	readonly requestedRows: number;
	readonly revision: bigint;
	readonly rows: readonly DecodedTerminalTextRow[];
	readonly totalRows: bigint;
}

export function projectTerminalOutputText(
	projection: TerminalOutputTextProjection,
): WorkbenchTerminalOutputReadOutput {
	if (projection.rows.length < 1 || projection.rows.length > projection.requestedRows) {
		throw new Error('Terminal output projection requires a non-empty bounded tail.');
	}
	const expectedIndex = projection.rows[0]?.index ?? 0n;
	for (const [offset, row] of projection.rows.entries()) {
		if (row.index !== expectedIndex + BigInt(offset)) {
			throw new Error('Terminal output projection rows must be contiguous.');
		}
		const prior = projection.rows[offset - 1];
		if (prior !== undefined && prior.wrap !== row.wrapContinuation) {
			throw new Error('Terminal output projection wrap boundaries are inconsistent.');
		}
	}
	const selected = selectBoundedTail(projection.rows);
	const startRow = projection.rows[selected.startOffset]?.index;
	if (startRow === undefined || startRow + BigInt(selected.returnedRows) > projection.totalRows) {
		throw new Error('Terminal output projection exceeds the retained row range.');
	}
	return WorkbenchTerminalOutputReadOutputSchema.parse({
		activeScreen: projection.activeScreen,
		format: 'plain-text',
		generation: projection.generation.toString(),
		presentationId: projection.presentationId,
		requestedRows: projection.requestedRows,
		returnedRows: selected.returnedRows,
		revision: projection.revision.toString(),
		source: 'tail-unwrapped',
		startRow: startRow.toString(),
		text: selected.text,
		totalRows: projection.totalRows.toString(),
		truncated: selected.partialRow || startRow > 0n,
	});
}

export function terminalOutputTextLimitReached(rows: readonly DecodedTerminalTextRow[]): boolean {
	let byteLength = 0;
	for (const [index, row] of rows.entries()) {
		byteLength += textEncoder.encode(row.text).byteLength;
		if (index < rows.length - 1 && !row.wrap) byteLength += 1;
		if (byteLength >= WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES) return true;
	}
	return false;
}

function selectBoundedTail(rows: readonly DecodedTerminalTextRow[]): {
	readonly partialRow: boolean;
	readonly returnedRows: number;
	readonly startOffset: number;
	readonly text: string;
} {
	const reverseSegments: string[] = [];
	let byteLength = 0;
	let returnedRows = 0;
	let partialRow = false;
	for (let index = rows.length - 1; index >= 0; index -= 1) {
		const row = rows[index];
		if (row === undefined) continue;
		const suffix = index < rows.length - 1 && !row.wrap ? '\n' : '';
		const segment = `${row.text}${suffix}`;
		const segmentBytes = textEncoder.encode(segment);
		if (byteLength + segmentBytes.byteLength <= WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES) {
			reverseSegments.push(segment);
			byteLength += segmentBytes.byteLength;
			returnedRows += 1;
			continue;
		}
		if (returnedRows === 0) {
			reverseSegments.push(utf8Suffix(segmentBytes, WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES));
			returnedRows = 1;
			partialRow = true;
		}
		break;
	}
	if (returnedRows < 1) throw new Error('Terminal output projection selected no rows.');
	return {
		partialRow,
		returnedRows,
		startOffset: rows.length - returnedRows,
		text: reverseSegments.reverse().join(''),
	};
}

function utf8Suffix(bytes: Uint8Array, maximumBytes: number): string {
	let start = Math.max(0, bytes.byteLength - maximumBytes);
	while (start < bytes.byteLength && (bytes[start] ?? 0) >= 0x80 && (bytes[start] ?? 0) < 0xc0) {
		start += 1;
	}
	return textDecoder.decode(bytes.subarray(start));
}
