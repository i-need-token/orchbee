import {
	ActiveScreen,
	TerminalColorEncoding,
	TerminalUnderlineKindCode,
	type TerminalGraphicsPlacement,
} from '@orchbee/terminal-protocol';

import type { ProjectedTerminalMetadata } from './packed-terminal-store';

interface TrailingBlankRow {
	readonly backgroundColors: Uint32Array;
	readonly promptClass: number;
	readonly rowFlags: number;
	readonly styleFlags: Uint16Array;
	readonly underlineColors: Uint32Array;
	readonly underlineKinds: Uint8Array;
	grapheme(column: number): string;
}

export interface TrailingBlankRowsProjection {
	readonly enabled: boolean;
	readonly metadata: ProjectedTerminalMetadata;
	readonly placements: readonly TerminalGraphicsPlacement[];
	readonly rows: readonly TrailingBlankRow[];
}

/**
 * Return the live-grid suffix that can be replaced by retained history without losing visible
 * presentation. A zero result is the conservative no-op boundary.
 */
export function trailingBlankRowsToTrim(projection: TrailingBlankRowsProjection): number {
	const { enabled, metadata, placements, rows } = projection;
	if (
		!enabled ||
		metadata.activeScreen !== ActiveScreen.primary ||
		applicationMouseTrackingIsActive(metadata) ||
		metadata.rows < 2 ||
		metadata.columns < 1 ||
		rows.length !== metadata.rows ||
		metadata.scrollbackRows < 1n
	) {
		return 0;
	}

	const graphicsRows = projectedGraphicsRows(metadata, placements);
	let blankRows = 0;
	for (let rowIndex = rows.length - 1; rowIndex > 0; rowIndex--) {
		const row = rows[rowIndex];
		if (
			row === undefined ||
			rowHasVisiblePresentation(row, metadata.columns) ||
			(metadata.cursor.visible && metadata.cursor.row === rowIndex) ||
			graphicsRows.has(rowIndex)
		) {
			break;
		}
		blankRows++;
	}

	const retainedRows =
		metadata.scrollbackRows > BigInt(Number.MAX_SAFE_INTEGER)
			? Number.MAX_SAFE_INTEGER
			: Number(metadata.scrollbackRows);
	return Math.min(blankRows, retainedRows);
}

function applicationMouseTrackingIsActive(metadata: ProjectedTerminalMetadata): boolean {
	const interaction = metadata.interaction;
	return (
		interaction.mouseAny ||
		interaction.mouseButton ||
		interaction.mouseNormal ||
		interaction.mouseX10
	);
}

function rowHasVisiblePresentation(row: TrailingBlankRow, columns: number): boolean {
	if (row.promptClass !== 0 || row.rowFlags !== 0) return true;
	for (let column = 0; column < columns; column++) {
		if (row.grapheme(column).trim().length > 0) return true;
		if ((row.styleFlags[column] ?? 0) !== 0) return true;
		if (
			(row.backgroundColors[column] ?? TerminalColorEncoding.default) !==
			TerminalColorEncoding.default
		) {
			return true;
		}
		if (
			(row.underlineKinds[column] ?? TerminalUnderlineKindCode.none) !==
			TerminalUnderlineKindCode.none
		) {
			return true;
		}
		if (
			(row.underlineColors[column] ?? TerminalColorEncoding.none) !== TerminalColorEncoding.none
		) {
			return true;
		}
	}
	return false;
}

function projectedGraphicsRows(
	metadata: ProjectedTerminalMetadata,
	placements: readonly TerminalGraphicsPlacement[],
): ReadonlySet<number> {
	const result = new Set<number>();
	const cellHeight = metadata.cellPixelHeight;
	if (!Number.isFinite(cellHeight) || cellHeight <= 0) return result;
	for (const placement of placements) {
		if (!Number.isFinite(placement.destinationHeight) || placement.destinationHeight <= 0) continue;
		const startPixel = placement.row * cellHeight + placement.offsetY;
		const endPixel = startPixel + placement.destinationHeight;
		const startRow = Math.max(0, Math.floor(startPixel / cellHeight));
		const endRow = Math.min(metadata.rows - 1, Math.ceil(endPixel / cellHeight) - 1);
		for (let row = startRow; row <= endRow; row++) result.add(row);
	}
	return result;
}
