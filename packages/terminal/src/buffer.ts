/**
 * Buffer API — xterm.js-compatible buffer access.
 *
 * Provides read-only access to terminal buffer contents, cursor state,
 * and viewport position. Wraps the Grid/Terminal data access layer.
 *
 * Usage:
 * ```typescript
 * const cell = term.buffer.active.getLine(0)?.getCell(5);
 * console.log(cell?.getChars(), cell?.isBold());
 *
 * if (term.buffer.active.type === 'alternate') {
 *   console.log('Full-screen app running');
 * }
 * ```
 */

import { EventEmitter } from './event-emitter';
import type {
	CellData,
	IBuffer,
	IBufferCell,
	IBufferLine,
	IBufferNamespace,
	IEvent,
} from './types';
import { CellFlags } from './types';

// ============================================================================
// BufferTerminal — decoupled terminal interface
// ============================================================================

/**
 * Terminal interface required by Buffer classes.
 *
 * Decouples buffer access from the concrete Terminal class,
 * making the buffer API testable and portable.
 */
export interface BufferTerminal {
	isAlternateScreen(): boolean;
	readonly cursorX: number;
	readonly cursorY: number;
	readonly cols: number;
	readonly rows: number;
	readonly viewportY: number;
	getScrollbackLength(): number;
	getLine(y: number): CellData[] | null;
	getScrollbackLine(offset: number): CellData[] | null;
	isRowWrapped(row: number): boolean;
	isScrollbackRowWrapped(offset: number): boolean;
}

// ============================================================================
// BufferNamespace — Top-level buffer API
// ============================================================================

/**
 * Top-level buffer API namespace.
 * Provides access to active, normal, and alternate screen buffers.
 */
export class BufferNamespace implements IBufferNamespace {
	private terminal: BufferTerminal;
	private bufferChangeEmitter = new EventEmitter<IBuffer>();

	// Lazy-initialized buffer wrappers (stateless, so we can cache them)
	private _normalBuffer?: Buffer;
	private _alternateBuffer?: Buffer;

	constructor(terminal: BufferTerminal) {
		this.terminal = terminal;
	}

	get active(): IBuffer {
		return this.terminal.isAlternateScreen() ? this.alternate : this.normal;
	}

	get normal(): IBuffer {
		if (!this._normalBuffer) {
			this._normalBuffer = new Buffer(this.terminal, 'normal');
		}
		return this._normalBuffer;
	}

	get alternate(): IBuffer {
		if (!this._alternateBuffer) {
			this._alternateBuffer = new Buffer(this.terminal, 'alternate');
		}
		return this._alternateBuffer;
	}

	get onBufferChange(): IEvent<IBuffer> {
		return this.bufferChangeEmitter.event;
	}

	/**
	 * Internal: Fire buffer change event when screen switches.
	 * Should be called by Terminal when detecting screen change.
	 */
	_fireBufferChange(buffer: IBuffer): void {
		this.bufferChangeEmitter.fire(buffer);
	}
}

// ============================================================================
// Buffer — Represents a terminal buffer (normal or alternate)
// ============================================================================

/**
 * A terminal buffer (normal or alternate screen).
 */
export class Buffer implements IBuffer {
	private terminal: BufferTerminal;
	private bufferType: 'normal' | 'alternate';
	private nullCell: BufferCell;

	constructor(terminal: BufferTerminal, type: 'normal' | 'alternate') {
		this.terminal = terminal;
		this.bufferType = type;

		// Create a null cell (codepoint=0, default colors, no flags)
		const nullCellData: CellData = {
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
		this.nullCell = new BufferCell(nullCellData);
	}

	get type(): 'normal' | 'alternate' {
		return this.bufferType;
	}

	get cursorX(): number {
		return this.terminal.cursorX;
	}

	get cursorY(): number {
		return this.terminal.cursorY;
	}

	get viewportY(): number {
		if (this.bufferType === 'alternate') return 0;
		return Math.max(0, this.terminal.getScrollbackLength() - this.terminal.viewportY);
	}

	get baseY(): number {
		return this.bufferType === 'alternate' ? 0 : this.terminal.getScrollbackLength();
	}

	get length(): number {
		if (this.bufferType === 'alternate') {
			// Alternate buffer has no scrollback, just visible rows.
			return this.terminal.rows;
		}
		// Normal buffer: scrollback + visible rows.
		return this.terminal.getScrollbackLength() + this.terminal.rows;
	}

	getLine(y: number): IBufferLine | undefined {
		// Check bounds.
		if (y < 0 || y >= this.length) {
			return undefined;
		}

		const scrollbackLength = this.terminal.getScrollbackLength();
		let cells: CellData[] | null;
		let isWrapped: boolean;

		if (this.bufferType === 'normal' && y < scrollbackLength) {
			// Accessing scrollback.
			// Buffer coords: y=0 = oldest, y=(length-1) = newest.
			// scrollbackOffset = y directly.
			const scrollbackOffset = y;
			cells = this.terminal.getScrollbackLine(scrollbackOffset);
			isWrapped = this.terminal.isScrollbackRowWrapped(scrollbackOffset);
		} else {
			// Accessing visible screen.
			const lineNumber = this.bufferType === 'normal' ? y - scrollbackLength : y;
			cells = this.terminal.getLine(lineNumber);
			isWrapped = this.terminal.isRowWrapped(lineNumber);
		}

		if (!cells) {
			return undefined;
		}

		return new BufferLine(cells, isWrapped, this.terminal.cols);
	}

	getNullCell(): IBufferCell {
		return this.nullCell;
	}
}

// ============================================================================
// BufferLine — Represents a single line in the buffer
// ============================================================================

/**
 * A single line in the buffer.
 */
export class BufferLine implements IBufferLine {
	private cells: CellData[];
	private _isWrapped: boolean;
	private _length: number;

	constructor(cells: CellData[], isWrapped: boolean, length: number) {
		this.cells = cells;
		this._isWrapped = isWrapped;
		this._length = length;
	}

	get length(): number {
		return this._length;
	}

	get isWrapped(): boolean {
		return this._isWrapped;
	}

	getCell(x: number): IBufferCell | undefined {
		if (x < 0 || x >= this._length) {
			return undefined;
		}

		if (x >= this.cells.length) {
			// Cell beyond what was returned (empty/null cell).
			return new BufferCell({
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
			});
		}

		const cellData = this.cells[x];
		if (cellData === undefined) {
			return new BufferCell({
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
			});
		}
		return new BufferCell(cellData);
	}

	translateToString(trimRight = false, startColumn = 0, endColumn = this._length): string {
		// Clamp bounds.
		const start = Math.max(0, Math.min(startColumn, this._length));
		const end = Math.max(start, Math.min(endColumn, this._length));

		let result = '';
		for (let x = start; x < end; x++) {
			const cell = this.getCell(x);
			if (cell) {
				result += cell.getChars();
			}
		}

		if (trimRight) {
			result = result.trimEnd();
		}

		return result;
	}
}

// ============================================================================
// BufferCell — Represents a single cell in the buffer
// ============================================================================

/**
 * A single cell in the buffer.
 *
 * Wraps CellData (our internal representation) to expose the
 * xterm.js-compatible IBufferCell interface.
 */
export class BufferCell implements IBufferCell {
	private cell: CellData;

	constructor(cell: CellData) {
		this.cell = cell;
	}

	getChars(): string {
		const codepoint = this.cell.codepoint;

		// Return empty string for null character.
		if (codepoint === 0) {
			return '';
		}

		// Use the pre-decoded text if available (grapheme cluster).
		if (this.cell.text && this.cell.text !== ' ') {
			return this.cell.text;
		}

		// Validate codepoint is within valid Unicode range.
		// Valid: 0x0000 to 0x10FFFF, excluding surrogates (0xD800-0xDFFF).
		if (codepoint < 0 || codepoint > 0x10ffff || (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
			return '�';
		}

		return String.fromCodePoint(codepoint);
	}

	getCode(): number {
		return this.cell.codepoint;
	}

	getWidth(): number {
		return this.cell.width;
	}

	getFgColorMode(): number {
		// -1 = RGB (we always use RGB in our implementation).
		// xterm.js convention: 0 = default, 1 = palette 16, 2 = palette 256, 3 = RGB.
		return this.cell.fg === null ? 0 : 3;
	}

	getBgColorMode(): number {
		return this.cell.bg === null ? 0 : 3;
	}

	getFgColor(): number {
		// Pack RGB into a single number: 0xRRGGBB.
		if (this.cell.fg === null) return -1;
		return (this.cell.fg.r << 16) | (this.cell.fg.g << 8) | this.cell.fg.b;
	}

	getBgColor(): number {
		if (this.cell.bg === null) return -1;
		return (this.cell.bg.r << 16) | (this.cell.bg.g << 8) | this.cell.bg.b;
	}

	isBold(): number {
		return this.cell.hasStyling && this.hasFlag(CellFlags.BOLD) ? 1 : 0;
	}

	isItalic(): number {
		return this.cell.hasStyling && this.hasFlag(CellFlags.ITALIC) ? 1 : 0;
	}

	isUnderline(): number {
		return this.cell.hasStyling && this.hasFlag(CellFlags.UNDERLINE) ? 1 : 0;
	}

	isStrikethrough(): number {
		return this.cell.hasStyling && this.hasFlag(CellFlags.STRIKETHROUGH) ? 1 : 0;
	}

	isBlink(): number {
		return this.cell.hasStyling && this.hasFlag(CellFlags.BLINK) ? 1 : 0;
	}

	isInverse(): number {
		return this.cell.hasStyling && this.hasFlag(CellFlags.INVERSE) ? 1 : 0;
	}

	isInvisible(): number {
		return this.cell.hasStyling && this.hasFlag(CellFlags.INVISIBLE) ? 1 : 0;
	}

	isFaint(): number {
		return this.cell.hasStyling && this.hasFlag(CellFlags.FAINT) ? 1 : 0;
	}

	/**
	 * Get hyperlink ID for this cell (0 = no link).
	 * Used by link detection system.
	 */
	getHyperlinkId(): number {
		return this.cell.hyperlinkId;
	}

	/**
	 * Get the Unicode codepoint for this cell.
	 * Used by link detection system.
	 */
	getCodepoint(): number {
		return this.cell.codepoint;
	}

	/**
	 * Check if cell has dim/faint attribute.
	 */
	isDim(): boolean {
		return this.cell.hasStyling && this.hasFlag(CellFlags.FAINT);
	}

	/** Check a style flag. */
	private hasFlag(_flag: number): boolean {
		return (this.cell.flags & _flag) !== 0;
	}
}
