import { TerminalCellWidthCode } from '@orchbee/terminal-protocol';

export interface PackedCursorPresentation {
	readonly column: number;
	readonly width: 1 | 2;
}

/** Project the canonical Host cursor onto the complete grapheme painted by the renderer. */
export function resolvePackedCursorPresentation(
	cursor: { readonly column: number; readonly wideTail: boolean },
	widths: Uint8Array | undefined,
): PackedCursorPresentation {
	const column = cursor.wideTail && cursor.column > 0 ? cursor.column - 1 : cursor.column;
	const width =
		(cursor.wideTail && cursor.column > 0) || widths?.[column] === TerminalCellWidthCode.wide
			? 2
			: 1;
	return { column, width };
}
