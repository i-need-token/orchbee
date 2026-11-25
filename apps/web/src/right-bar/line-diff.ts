/** One line of a two-sided comparison, numbered from 1 on each side it appears in. */
export type DiffLine =
	| {
			readonly kind: 'same';
			readonly oldNumber: number;
			readonly newNumber: number;
			readonly text: string;
	  }
	| { readonly kind: 'removed'; readonly oldNumber: number; readonly text: string }
	| { readonly kind: 'added'; readonly newNumber: number; readonly text: string };

/** What the preview shows: changed lines with a little context, and the unchanged runs between. */
export type DiffRow = DiffLine | { readonly kind: 'gap'; readonly hidden: number };

/** Edits the comparison looks for before it shows the changed middle as replaced whole. */
const MAXIMUM_EDITS = 1_000;

/** A text's lines, without their terminators; a final newline ends the last line. */
export function splitLines(text: string): string[] {
	if (text === '') return [];
	const lines = text.split('\n');
	if (lines.at(-1) === '') lines.pop();
	return lines.map((line) => (line.endsWith('\r') ? line.slice(0, -1) : line));
}

/**
 * The shortest line edit from `before` to `after` (Myers), after the common start and end are set
 * aside. A middle that needs more than `maximumEdits` edits shows as removed and added whole, which
 * keeps a large rewrite from costing the window time or memory.
 */
export function diffLines(
	before: readonly string[],
	after: readonly string[],
	maximumEdits: number = MAXIMUM_EDITS,
): DiffLine[] {
	let start = 0;
	while (start < before.length && start < after.length && before[start] === after[start]) {
		start += 1;
	}
	let beforeEnd = before.length;
	let afterEnd = after.length;
	while (beforeEnd > start && afterEnd > start && before[beforeEnd - 1] === after[afterEnd - 1]) {
		beforeEnd -= 1;
		afterEnd -= 1;
	}
	const lines: DiffLine[] = [];
	for (let index = 0; index < start; index += 1) {
		lines.push({
			kind: 'same',
			newNumber: index + 1,
			oldNumber: index + 1,
			text: before[index] ?? '',
		});
	}
	const middle =
		shortestEdit(before, after, start, beforeEnd, afterEnd, maximumEdits) ??
		replacement(before, after, start, beforeEnd, afterEnd);
	lines.push(...middle);
	for (let offset = 0; beforeEnd + offset < before.length; offset += 1) {
		lines.push({
			kind: 'same',
			newNumber: afterEnd + offset + 1,
			oldNumber: beforeEnd + offset + 1,
			text: before[beforeEnd + offset] ?? '',
		});
	}
	return lines;
}

function replacement(
	before: readonly string[],
	after: readonly string[],
	start: number,
	beforeEnd: number,
	afterEnd: number,
): DiffLine[] {
	const lines: DiffLine[] = [];
	for (let index = start; index < beforeEnd; index += 1) {
		lines.push({ kind: 'removed', oldNumber: index + 1, text: before[index] ?? '' });
	}
	for (let index = start; index < afterEnd; index += 1) {
		lines.push({ kind: 'added', newNumber: index + 1, text: after[index] ?? '' });
	}
	return lines;
}

/** Myers' greedy forward search with its frontiers kept for the walk back; null past the limit. */
function shortestEdit(
	before: readonly string[],
	after: readonly string[],
	start: number,
	beforeEnd: number,
	afterEnd: number,
	maximumEdits: number,
): DiffLine[] | null {
	const n = beforeEnd - start;
	const m = afterEnd - start;
	const limit = Math.min(n + m, maximumEdits);
	const offset = limit + 1;
	const frontier = new Int32Array(2 * limit + 3);
	// Each step's frontier for diagonals -d..d, indexed by diagonal + d.
	const trace: Int32Array[] = [];
	for (let d = 0; d <= limit; d += 1) {
		for (let k = -d; k <= d; k += 2) {
			const down =
				k === -d || (k !== d && (frontier[k - 1 + offset] ?? 0) < (frontier[k + 1 + offset] ?? 0));
			let x = down ? (frontier[k + 1 + offset] ?? 0) : (frontier[k - 1 + offset] ?? 0) + 1;
			let y = x - k;
			while (x < n && y < m && before[start + x] === after[start + y]) {
				x += 1;
				y += 1;
			}
			frontier[k + offset] = x;
			if (x >= n && y >= m) {
				trace.push(frontier.slice(offset - d, offset + d + 1));
				return walkBack(trace, before, after, start, n, m);
			}
		}
		trace.push(frontier.slice(offset - d, offset + d + 1));
	}
	return null;
}

function walkBack(
	trace: readonly Int32Array[],
	before: readonly string[],
	after: readonly string[],
	start: number,
	n: number,
	m: number,
): DiffLine[] {
	const reversed: DiffLine[] = [];
	const same = (x: number, y: number): void => {
		reversed.push({
			kind: 'same',
			newNumber: start + y + 1,
			oldNumber: start + x + 1,
			text: before[start + x] ?? '',
		});
	};
	let x = n;
	let y = m;
	for (let d = trace.length - 1; d > 0; d -= 1) {
		const previous = trace[d - 1];
		if (previous === undefined) break;
		const at = (k: number): number => previous[k + d - 1] ?? 0;
		const k = x - y;
		const down = k === -d || (k !== d && at(k - 1) < at(k + 1));
		const previousK = down ? k + 1 : k - 1;
		const previousX = at(previousK);
		const previousY = previousX - previousK;
		while (x > previousX && y > previousY) {
			x -= 1;
			y -= 1;
			same(x, y);
		}
		if (down) {
			reversed.push({
				kind: 'added',
				newNumber: start + previousY + 1,
				text: after[start + previousY] ?? '',
			});
		} else {
			reversed.push({
				kind: 'removed',
				oldNumber: start + previousX + 1,
				text: before[start + previousX] ?? '',
			});
		}
		x = previousX;
		y = previousY;
	}
	while (x > 0 && y > 0) {
		x -= 1;
		y -= 1;
		same(x, y);
	}
	return reversed.reverse();
}

/** Keeps `context` unchanged lines around each change and folds longer unchanged runs into gaps. */
export function presentDiff(lines: readonly DiffLine[], context = 3): DiffRow[] {
	const rows: DiffRow[] = [];
	let run: DiffLine[] = [];
	const flush = (last: boolean): void => {
		const leading = rows.length === 0 ? 0 : context;
		const trailing = last ? 0 : context;
		if (run.length <= leading + trailing + 1) {
			rows.push(...run);
		} else {
			rows.push(...run.slice(0, leading));
			rows.push({ hidden: run.length - leading - trailing, kind: 'gap' });
			rows.push(...run.slice(run.length - trailing));
		}
		run = [];
	};
	for (const line of lines) {
		if (line.kind === 'same') {
			run.push(line);
		} else {
			if (run.length > 0) flush(false);
			rows.push(line);
		}
	}
	if (run.length > 0) flush(true);
	return rows;
}
