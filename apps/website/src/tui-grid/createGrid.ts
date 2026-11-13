// A cell grid that paints like the Orchbee terminal interface. Frames and status glyphs become
// single-cell shapes so their alignment never depends on which fallback font supplies a glyph.

type TuiTone =
	| 'primary'
	| 'secondary'
	| 'divider'
	| 'unfocused'
	| 'focus'
	| 'warning'
	| 'success'
	| 'error'
	| 'info'
	| 'attention'
	| 'working'
	| 'done'
	| 'failed'
	| 'idle'
	| 'unread';

export type TuiSurface = 'host' | 'current' | 'overlay' | 'selected';

type TuiShape =
	| 'h'
	| 'v'
	| 'square-tl'
	| 'square-tr'
	| 'square-bl'
	| 'square-br'
	| 'round-tl'
	| 'round-tr'
	| 'round-bl'
	| 'round-br'
	| 'hex'
	| 'hex-ring'
	| 'dot'
	| 'bullet'
	| 'block'
	| 'cell';

type TuiMotion = 'pulse' | 'breathe';

export type AgentState = 'attention' | 'working' | 'done' | 'failed' | 'idle';

export interface TuiSegment {
	readonly text: string;
	readonly tone: TuiTone;
	readonly surface: TuiSurface;
	readonly bold: boolean;
	readonly shape?: TuiShape;
	readonly motion?: TuiMotion;
}

export type TuiLine = readonly TuiSegment[];

export interface TuiPutOptions {
	readonly surface?: TuiSurface;
	readonly bold?: boolean;
	readonly motion?: TuiMotion;
}

type TuiRunPart = readonly [text: string, tone?: TuiTone, options?: TuiPutOptions];

export interface TuiGrid {
	readonly cols: number;
	/** Writes text from a cell and returns the column after it. */
	put(row: number, col: number, text: string, tone?: TuiTone, options?: TuiPutOptions): number;
	run(row: number, col: number, parts: readonly TuiRunPart[]): number;
	/** Writes text so that its last character lands on `end`. */
	right(row: number, end: number, text: string, tone?: TuiTone, options?: TuiPutOptions): number;
	fill(row: number, from: number, to: number, surface: TuiSurface): void;
	clear(row: number, from: number, to: number, surface: TuiSurface): void;
	frame(
		top: number,
		left: number,
		bottom: number,
		right: number,
		tone: TuiTone,
		corners: 'square' | 'round',
	): void;
	glyph(row: number, col: number, state: AgentState, surface?: TuiSurface): number;
	lines(): TuiLine[];
}

interface Cell {
	ch: string;
	tone: TuiTone;
	surface: TuiSurface;
	bold: boolean;
	motion: TuiMotion | undefined;
}

const SHAPES: Readonly<Record<string, TuiShape>> = {
	'─': 'h',
	'│': 'v',
	'┌': 'square-tl',
	'┐': 'square-tr',
	'└': 'square-bl',
	'┘': 'square-br',
	'╭': 'round-tl',
	'╮': 'round-tr',
	'╰': 'round-bl',
	'╯': 'round-br',
	'⬢': 'hex',
	'⬡': 'hex-ring',
	'●': 'dot',
	'•': 'bullet',
	'█': 'block',
};

// The glyph, color role and animation the terminal interface gives each Agent state.
const STATE_GLYPHS: Readonly<
	Record<AgentState, readonly [glyph: string, tone: TuiTone, motion: TuiMotion | undefined]>
> = {
	attention: ['⬢', 'attention', 'pulse'],
	working: ['⬢', 'working', 'breathe'],
	done: ['⬢', 'done', undefined],
	failed: ['⬢', 'failed', undefined],
	idle: ['⬡', 'idle', undefined],
};

const LAST_ASCII = 126;

export function createGrid(cols: number, rows: number, surface: TuiSurface = 'host'): TuiGrid {
	const cells: Cell[][] = Array.from({ length: rows }, () =>
		Array.from({ length: cols }, () => ({
			ch: ' ',
			tone: 'primary',
			surface,
			bold: false,
			motion: undefined,
		})),
	);
	const at = (row: number, col: number): Cell | undefined => cells[row]?.[col];

	const grid: TuiGrid = {
		cols,
		put(row, col, text, tone = 'primary', options = {}) {
			const characters = Array.from(text);
			characters.forEach((ch, index) => {
				const cell = at(row, col + index);
				if (cell === undefined) return;
				cell.ch = ch;
				cell.tone = tone;
				if (options.surface !== undefined) cell.surface = options.surface;
				cell.bold = options.bold === true;
				cell.motion = options.motion;
			});
			return col + characters.length;
		},
		run(row, col, parts) {
			let next = col;
			for (const [text, tone, options] of parts) next = grid.put(row, next, text, tone, options);
			return next;
		},
		right(row, end, text, tone, options) {
			return grid.put(row, end - Array.from(text).length + 1, text, tone, options);
		},
		fill(row, from, to, fillSurface) {
			for (let col = from; col <= to; col += 1) {
				const cell = at(row, col);
				if (cell !== undefined) cell.surface = fillSurface;
			}
		},
		clear(row, from, to, clearSurface) {
			for (let col = from; col <= to; col += 1) {
				const cell = at(row, col);
				if (cell === undefined) continue;
				cell.ch = ' ';
				cell.tone = 'primary';
				cell.surface = clearSurface;
				cell.bold = false;
				cell.motion = undefined;
			}
		},
		frame(top, left, bottom, right, tone, corners) {
			for (let col = left + 1; col < right; col += 1) {
				grid.put(top, col, '─', tone);
				grid.put(bottom, col, '─', tone);
			}
			for (let row = top + 1; row < bottom; row += 1) {
				grid.put(row, left, '│', tone);
				grid.put(row, right, '│', tone);
			}
			const [tl, tr, bl, br] = corners === 'square' ? '┌┐└┘' : '╭╮╰╯';
			grid.put(top, left, tl ?? '', tone);
			grid.put(top, right, tr ?? '', tone);
			grid.put(bottom, left, bl ?? '', tone);
			grid.put(bottom, right, br ?? '', tone);
		},
		glyph(row, col, state, glyphSurface) {
			const [glyph, tone, motion] = STATE_GLYPHS[state];
			return grid.put(row, col, glyph, tone, {
				...(glyphSurface === undefined ? {} : { surface: glyphSurface }),
				...(motion === undefined ? {} : { motion }),
			});
		},
		lines() {
			return cells.map((row) => {
				const segments: TuiSegment[] = [];
				let open: { text: string; cell: Cell } | undefined;
				const flush = (): void => {
					if (open === undefined) return;
					segments.push(segment(open.text, open.cell));
					open = undefined;
				};
				for (const cell of row) {
					const shape =
						SHAPES[cell.ch] ?? ((cell.ch.codePointAt(0) ?? 0) > LAST_ASCII ? 'cell' : undefined);
					if (shape !== undefined) {
						flush();
						segments.push(segment(shape === 'cell' ? cell.ch : '', cell, shape));
						continue;
					}
					if (open !== undefined && sameStyle(open.cell, cell)) open.text += cell.ch;
					else {
						flush();
						open = { text: cell.ch, cell };
					}
				}
				flush();
				return segments;
			});
		},
	};
	return grid;
}

function sameStyle(a: Cell, b: Cell): boolean {
	return a.tone === b.tone && a.surface === b.surface && a.bold === b.bold && a.motion === b.motion;
}

function segment(text: string, cell: Cell, shape?: TuiShape): TuiSegment {
	return {
		text,
		tone: cell.tone,
		surface: cell.surface,
		bold: cell.bold,
		...(shape === undefined ? {} : { shape }),
		...(cell.motion === undefined ? {} : { motion: cell.motion }),
	};
}
