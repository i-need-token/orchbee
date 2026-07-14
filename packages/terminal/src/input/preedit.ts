export interface CompositionSelection {
	readonly start: number;
	readonly end: number;
}

export interface PreeditCellData {
	readonly text: string;
	readonly width: number;
}

export interface PreeditState {
	readonly cells: readonly PreeditCellData[];
	readonly width: number;
	readonly selection: { readonly start: number; readonly end: number };
}

export function shouldSuppressComposingControlInput(
	text: string | null,
	composing: boolean,
): boolean {
	if (!composing || text === null) return false;
	const codepoints = [...text];
	return codepoints.length === 1 && (codepoints[0]?.codePointAt(0) ?? 0x20) < 0x20;
}

/** Map DOM UTF-16 composition offsets to terminal cell offsets. */
export function buildPreeditState(
	data: string,
	selection: CompositionSelection | undefined,
	getWidth: (codepoint: number) => number,
): PreeditState {
	const cells: PreeditCellData[] = [];
	const boundaries: Array<{ readonly utf16: number; readonly cells: number }> = [];
	let utf16Offset = 0;
	let cellOffset = 0;
	let joinNext = false;
	for (const text of data) {
		const codepoint = text.codePointAt(0) ?? 0;
		const measuredWidth = Math.max(0, Math.min(2, getWidth(codepoint)));
		const previous = cells.at(-1);
		if (previous && (measuredWidth === 0 || joinNext)) {
			cells[cells.length - 1] = { text: previous.text + text, width: previous.width };
		} else {
			const width = Math.max(1, measuredWidth);
			cells.push({ text, width });
			cellOffset += width;
		}
		utf16Offset += text.length;
		boundaries.push({ utf16: utf16Offset, cells: cellOffset });
		joinNext = codepoint === 0x200d;
	}
	const start = mapUtf16Offset(selection?.start ?? data.length, boundaries);
	const end = mapUtf16Offset(selection?.end ?? selection?.start ?? data.length, boundaries);
	return {
		cells,
		width: cellOffset,
		selection: { start, end: Math.max(start, end) },
	};
}

function mapUtf16Offset(
	offset: number,
	boundaries: readonly { readonly utf16: number; readonly cells: number }[],
): number {
	if (offset <= 0) return 0;
	for (const boundary of boundaries) {
		if (offset <= boundary.utf16) return boundary.cells;
	}
	return boundaries.at(-1)?.cells ?? 0;
}
