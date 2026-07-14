/** Cached adapter between the upstream Ghostty render state and the renderer. */

import type {
	CellData,
	DirtyState,
	GhosttyCell,
	IRenderable,
	IScrollbackProvider,
	RGB,
	RenderStateCursor,
} from './types';
import { DirtyState as DirtyStateValue, PointTag } from './types';
import type { Terminal } from './terminal';

export class GhosttyTerminal implements IRenderable, IScrollbackProvider {
	private readonly term: Terminal;
	private lines: GhosttyCell[][] = [];
	private cellDataLines: CellData[][] = [];
	private textLines: string[][] = [];
	private wrappedRows: boolean[] = [];
	private readonly scrollbackCache = new Map<
		number,
		{ cells: GhosttyCell[]; cellData: CellData[]; texts: string[]; wrapped: boolean }
	>();
	private dirtyRows: boolean[] = [];
	private dirtyState: DirtyState = DirtyStateValue.FULL;
	private cursor: RenderStateCursor = {
		x: 0,
		y: 0,
		viewportX: 0,
		viewportY: 0,
		visible: true,
		blinking: false,
		style: 'block',
	};

	constructor(term: Terminal) {
		this.term = term;
	}

	get hasFrame(): boolean {
		return this.lines.length > 0;
	}

	/** Capture one immutable render frame after Terminal has updated the render state. */
	refresh(dirtyState: DirtyState): void {
		const frame = this.term.grid.captureFrame();
		const foreground = this.term.foregroundColor;
		const background = this.term.backgroundColor;
		const cols = this.term.cols;

		this.dirtyState = dirtyState;
		this.dirtyRows = [...frame.dirtyRows];
		this.lines = [];
		this.cellDataLines = [];
		this.textLines = [];
		this.wrappedRows = [...frame.wrappedRows];
		this.scrollbackCache.clear();

		for (let y = 0; y < this.term.rows; y++) {
			const source = frame.lines[y] ?? [];
			const cells: GhosttyCell[] = [];
			const texts: string[] = [];
			for (let x = 0; x < cols; x++) {
				const cell = source[x] ?? EMPTY_CELL_DATA;
				cells.push(this.cellDataToGhosttyCell(cell, foreground, background));
				texts.push(cell.text || ' ');
			}
			this.lines.push(cells);
			this.cellDataLines.push([...source]);
			this.textLines.push(texts);
		}

		const x = this.term.cursorX;
		const y = this.term.cursorY;
		this.cursor = {
			x,
			y,
			viewportX: x,
			viewportY: y,
			visible: this.term.cursorVisible,
			blinking: this.term.cursorBlinking,
			style: this.term.cursorStyle,
		};
	}

	getLine(y: number): GhosttyCell[] | null {
		if (y < 0 || y >= this.lines.length) return null;
		return this.lines[y] ?? null;
	}

	getCellDataLine(y: number): CellData[] | null {
		if (y < 0 || y >= this.cellDataLines.length) return null;
		return this.cellDataLines[y] ?? null;
	}

	isRowWrapped(y: number): boolean {
		return this.wrappedRows[y] ?? false;
	}

	getCursor(): RenderStateCursor {
		return this.cursor;
	}

	getDimensions(): { cols: number; rows: number } {
		return { cols: this.term.cols, rows: this.term.rows };
	}

	isRowDirty(y: number): boolean {
		return this.dirtyRows[y] ?? false;
	}

	needsFullRedraw(): boolean {
		return this.dirtyState === DirtyStateValue.FULL;
	}

	clearDirty(): void {
		this.term.grid.markClean();
		this.dirtyState = DirtyStateValue.NONE;
		this.dirtyRows.fill(false);
	}

	getGraphemeString(row: number, col: number): string {
		return this.textLines[row]?.[col] ?? ' ';
	}

	getScrollbackLine(offset: number): GhosttyCell[] | null {
		return this.getCachedScrollbackLine(offset)?.cells ?? null;
	}

	getScrollbackCellData(offset: number): CellData[] | null {
		return this.getCachedScrollbackLine(offset)?.cellData ?? null;
	}

	isScrollbackRowWrapped(offset: number): boolean {
		return this.getCachedScrollbackLine(offset)?.wrapped ?? false;
	}

	getScrollbackGraphemeString(offset: number, col: number): string {
		return this.getCachedScrollbackLine(offset)?.texts[col] ?? ' ';
	}

	getHyperlinkUri(row: number, col: number): string | null {
		return this.term.grid.getHyperlinkUri(PointTag.VIEWPORT, row, col);
	}

	getScrollbackHyperlinkUri(offset: number, col: number): string | null {
		return this.term.grid.getHyperlinkUri(PointTag.HISTORY, offset, col);
	}

	getScrollbackLength(): number {
		return this.term.grid.getScrollbackLength();
	}

	private cellDataToGhosttyCell(cell: CellData, foreground: RGB, background: RGB): GhosttyCell {
		const fg = cell.fg ?? foreground;
		const bg = cell.bg ?? background;
		return {
			codepoint: cell.codepoint,
			fg_r: fg.r,
			fg_g: fg.g,
			fg_b: fg.b,
			bg_r: bg.r,
			bg_g: bg.g,
			bg_b: bg.b,
			flags: cell.flags,
			width: cell.width,
			hyperlink_id: cell.hyperlinkId,
			grapheme_len: Math.max(0, cell.graphemeLen - 1),
			selected: cell.selected,
			underline_style: cell.underlineStyle,
			underline_r: cell.underlineColor?.r ?? -1,
			underline_g: cell.underlineColor?.g ?? -1,
			underline_b: cell.underlineColor?.b ?? -1,
			overline: cell.overline,
		};
	}

	private getCachedScrollbackLine(
		offset: number,
	): { cells: GhosttyCell[]; cellData: CellData[]; texts: string[]; wrapped: boolean } | undefined {
		const cached = this.scrollbackCache.get(offset);
		if (cached) return cached;

		const line = this.term.grid.readScrollbackLine(offset);
		if (!line) return undefined;
		const foreground = this.term.foregroundColor;
		const background = this.term.backgroundColor;
		const cellData = [...line.cells];
		const resolved = {
			cells: cellData.map((cell) => this.cellDataToGhosttyCell(cell, foreground, background)),
			cellData,
			texts: cellData.map((cell) => cell.text || ' '),
			wrapped: line.wrapped,
		};
		this.scrollbackCache.set(offset, resolved);
		return resolved;
	}
}

const EMPTY_CELL_DATA: CellData = {
	codepoint: 0,
	text: ' ',
	fg: null,
	bg: null,
	selected: false,
	hasStyling: false,
	flags: 0,
	width: 1,
	hyperlinkId: 0,
	graphemeLen: 0,
	semanticContent: 0,
	underlineStyle: 0,
	underlineColor: null,
	overline: false,
};
