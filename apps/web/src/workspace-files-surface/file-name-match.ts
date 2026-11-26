/**
 * Quick open's matching: what a typed file name asks for, and how well a path answers it. A query
 * is pieces split by whitespace, each matched in order of its characters but not necessarily
 * together, the way editors' quick open matches; every piece must match. A piece without a slash
 * prefers the file's own name and falls back to the whole path; one with a slash matches the path.
 */

/** A span of text a query matched, in UTF-16 offsets, from `start` up to `end`. */
interface TextRange {
	readonly start: number;
	readonly end: number;
}

/** What a typed query asks for: its pieces, and the line and column it names, if any. */
export interface FileQuery {
	/** The pieces lowered, which is how they match. */
	readonly pieces: readonly string[];
	/** The pieces as typed, whose case earns a small bonus where a path agrees. */
	readonly typed: readonly string[];
	readonly line?: number | undefined;
	readonly column?: number | undefined;
}

/** A path that answered a query, with what matched in its name and in its folder. */
export interface FileNameMatch {
	readonly path: string;
	readonly name: string;
	/** The folder holding the file, empty at the Workspace's top. */
	readonly folder: string;
	readonly nameMatches: readonly TextRange[];
	readonly folderMatches: readonly TextRange[];
}

/** A trailing `:line`, `:line:column` or `:line,column`, as editors and compilers write them. */
const LOCATION_SUFFIX = /:(\d+)(?:[:,](\d+))?$/u;

export function parseFileQuery(text: string): FileQuery {
	let name = text.trim();
	let line: number | undefined;
	let column: number | undefined;
	const location = LOCATION_SUFFIX.exec(name);
	if (location !== null) {
		const lineNumber = Number(location[1]);
		const columnNumber = location[2] === undefined ? undefined : Number(location[2]);
		if (lineNumber > 0) {
			name = name.slice(0, location.index);
			line = lineNumber;
			if (columnNumber !== undefined && columnNumber > 0) column = columnNumber;
		}
	}
	const typed = name
		.replaceAll('\\', '/')
		.split(/\s+/u)
		.filter((piece) => piece.length > 0);
	return {
		pieces: typed.map(lowerAligned),
		typed,
		...(line === undefined ? {} : { line }),
		...(column === undefined ? {} : { column }),
	};
}

/**
 * Lowers text one UTF-16 unit at a time, so offsets in the lowered text are offsets in the text. A
 * unit whose lower case takes more than one unit stays as it is.
 */
export function lowerAligned(text: string): string {
	const lowered = text.toLowerCase();
	if (lowered.length === text.length) return lowered;
	let aligned = '';
	for (let index = 0; index < text.length; index += 1) {
		const unit = text.charAt(index);
		const lower = unit.toLowerCase();
		aligned += lower.length === 1 ? lower : unit;
	}
	return aligned;
}

/** Whether every character of `needle` appears in `text` in order; both already lowered. */
function isSubsequence(needle: string, text: string): boolean {
	let at = 0;
	for (let index = 0; index < needle.length; index += 1) {
		at = text.indexOf(needle.charAt(index), at);
		if (at < 0) return false;
		at += 1;
	}
	return true;
}

/** Whether every piece of `query` is a subsequence of the lowered path. */
export function mayMatch(query: FileQuery, loweredPath: string): boolean {
	return query.pieces.every((piece) => isSubsequence(piece, loweredPath));
}

/**
 * Whether every path that answers `next` also answers `previous`, so a search for `next` can start
 * from what `previous` kept: each earlier piece must lie, in order, inside some piece now.
 */
export function narrows(previous: FileQuery, next: FileQuery): boolean {
	return previous.pieces.every((earlier) =>
		next.pieces.some((piece) => isSubsequence(earlier, piece)),
	);
}

// Weights after fzf's: a match is worth most at the start of a word and next to the one before.
const SCORE_MATCH = 16;
const GAP_START = -3;
const GAP_EXTENSION = -1;
/** A long gap costs no more than this, so letters at word starts far apart still rank well. */
const GAP_LIMIT = -5;
const BONUS_BOUNDARY = 8;
const BONUS_PATH_SEPARATOR = 9;
const BONUS_CAMEL = 7;
const BONUS_CONSECUTIVE = 4;
const BONUS_FIRST_MULTIPLIER = 2;
const BONUS_CASE = 1;
/** What a piece matched within the file's own name gains over one matched across the path. */
const BONUS_NAME = 32;
const NONE = Number.NEGATIVE_INFINITY;

function bonusAt(text: string, index: number): number {
	if (index === 0) return BONUS_BOUNDARY;
	const previous = text.charAt(index - 1);
	if (previous === '/') return BONUS_PATH_SEPARATOR;
	if (previous === '_' || previous === '-' || previous === '.' || previous === ' ') {
		return BONUS_BOUNDARY;
	}
	const current = text.charAt(index);
	const previousLower = previous !== previous.toUpperCase();
	const currentUpper = current !== current.toLowerCase();
	if (previousLower && currentUpper) return BONUS_CAMEL;
	const previousDigit = previous >= '0' && previous <= '9';
	const currentDigit = current >= '0' && current <= '9';
	return currentDigit && !previousDigit ? BONUS_CAMEL : 0;
}

/** Rows of scores kept between calls; they grow to the longest text scored. */
let previousRow = new Float64Array(256);
let currentRow = new Float64Array(256);

function ensureRows(length: number): void {
	if (previousRow.length >= length) return;
	const size = Math.max(length, previousRow.length * 2);
	previousRow = new Float64Array(size);
	currentRow = new Float64Array(size);
}

/**
 * How well `piece` (lowered) matches `text`, whose lowered form is `lowered`; null when it does
 * not. The best placement of the piece's characters is found in time proportional to the piece's
 * length times the text's.
 */
function scorePiece(piece: string, original: string, text: string, lowered: string): number | null {
	const width = text.length;
	if (piece.length === 0 || piece.length > width) return null;
	ensureRows(width);
	let previous = previousRow;
	let current = currentRow;
	for (let row = 0; row < piece.length; row += 1) {
		const wanted = piece.charAt(row);
		const wantedOriginal = original.charAt(row);
		let carried = NONE;
		let farthest = NONE;
		let any = false;
		for (let column = 0; column < width; column += 1) {
			if (row > 0 && column >= 2) {
				const before = previous[column - 2] ?? NONE;
				farthest = Math.max(farthest, before);
				carried = Math.max(carried + GAP_EXTENSION, before + GAP_START);
			}
			if (lowered.charAt(column) !== wanted) {
				current[column] = NONE;
				continue;
			}
			const bonus = bonusAt(text, column);
			const caseBonus = text.charAt(column) === wantedOriginal ? BONUS_CASE : 0;
			if (row === 0) {
				current[column] = SCORE_MATCH + bonus * BONUS_FIRST_MULTIPLIER + caseBonus;
				any = true;
				continue;
			}
			const adjacent = column >= 1 ? (previous[column - 1] ?? NONE) + BONUS_CONSECUTIVE : NONE;
			const best = Math.max(adjacent, carried, farthest + GAP_LIMIT);
			if (best === NONE) {
				current[column] = NONE;
				continue;
			}
			current[column] = best + SCORE_MATCH + bonus + caseBonus;
			any = true;
		}
		if (!any) return null;
		const swap = previous;
		previous = current;
		current = swap;
	}
	let best = NONE;
	for (let column = 0; column < width; column += 1) best = Math.max(best, previous[column] ?? NONE);
	return best === NONE ? null : best;
}

/**
 * Where the best placement of `piece` falls in `text`: the offsets of its characters. It keeps the
 * whole table, so it is worked out only for the paths that show.
 */
function placePiece(piece: string, original: string, text: string, lowered: string): number[] {
	const width = text.length;
	const table: Float64Array[] = [];
	for (let row = 0; row < piece.length; row += 1) {
		const scores = new Float64Array(width).fill(NONE);
		const above = table[row - 1];
		let carried = NONE;
		let farthest = NONE;
		for (let column = 0; column < width; column += 1) {
			if (above !== undefined && column >= 2) {
				const before = above[column - 2] ?? NONE;
				farthest = Math.max(farthest, before);
				carried = Math.max(carried + GAP_EXTENSION, before + GAP_START);
			}
			if (lowered.charAt(column) !== piece.charAt(row)) continue;
			const bonus = bonusAt(text, column);
			const caseBonus = text.charAt(column) === original.charAt(row) ? BONUS_CASE : 0;
			if (above === undefined) {
				scores[column] = SCORE_MATCH + bonus * BONUS_FIRST_MULTIPLIER + caseBonus;
				continue;
			}
			const adjacent = column >= 1 ? (above[column - 1] ?? NONE) + BONUS_CONSECUTIVE : NONE;
			const best = Math.max(adjacent, carried, farthest + GAP_LIMIT);
			if (best !== NONE) scores[column] = best + SCORE_MATCH + bonus + caseBonus;
		}
		table.push(scores);
	}
	const positions: number[] = [];
	const last = table[piece.length - 1];
	if (last === undefined) return positions;
	let column = 0;
	for (let index = 1; index < width; index += 1) {
		if ((last[index] ?? NONE) > (last[column] ?? NONE)) column = index;
	}
	for (let row = piece.length - 1; row >= 0; row -= 1) {
		positions.push(column);
		const above = table[row - 1];
		if (above === undefined) break;
		const here = table[row]?.[column] ?? NONE;
		const bonus = bonusAt(text, column);
		const caseBonus = text.charAt(column) === original.charAt(row) ? BONUS_CASE : 0;
		const reached = here - SCORE_MATCH - bonus - caseBonus;
		if (column >= 1 && (above[column - 1] ?? NONE) + BONUS_CONSECUTIVE === reached) {
			column -= 1;
			continue;
		}
		// The nearest earlier placement whose gap accounts for the score reached.
		let chosen = column - 2;
		for (let candidate = column - 2; candidate >= 0; candidate -= 1) {
			const gap = column - candidate - 1;
			const value =
				(above[candidate] ?? NONE) + Math.max(GAP_START + (gap - 1) * GAP_EXTENSION, GAP_LIMIT);
			if (value === reached) {
				chosen = candidate;
				break;
			}
		}
		column = chosen;
	}
	return positions.reverse();
}

/** Where each piece of a query matches in one path, by name or across the path. */
interface PiecePlan {
	readonly piece: string;
	readonly original: string;
	readonly inName: boolean;
}

/** How well a path answers a query, or null when some piece does not match. */
export function scorePath(query: FileQuery, path: string, loweredPath: string): number | null {
	const slash = path.lastIndexOf('/');
	const name = path.slice(slash + 1);
	const loweredName = loweredPath.slice(slash + 1);
	let total = 0;
	for (const [index, piece] of query.pieces.entries()) {
		const original = query.typed[index] ?? piece;
		if (!piece.includes('/')) {
			const named = scorePiece(piece, original, name, loweredName);
			if (named !== null) {
				total += named + BONUS_NAME;
				continue;
			}
		}
		const across = scorePiece(piece, original, path, loweredPath);
		if (across === null) return null;
		total += across;
	}
	return total;
}

/** What a path that answered a query shows: its name and folder, with what matched marked. */
export function describeMatch(query: FileQuery, path: string, loweredPath: string): FileNameMatch {
	const slash = path.lastIndexOf('/');
	const nameStart = slash + 1;
	const name = path.slice(nameStart);
	const loweredName = loweredPath.slice(nameStart);
	const marked = new Set<number>();
	const plans: PiecePlan[] = query.pieces.map((piece, index) => ({
		inName:
			!piece.includes('/') &&
			scorePiece(piece, query.typed[index] ?? piece, name, loweredName) !== null,
		original: query.typed[index] ?? piece,
		piece,
	}));
	for (const plan of plans) {
		const positions = plan.inName
			? placePiece(plan.piece, plan.original, name, loweredName).map(
					(position) => position + nameStart,
				)
			: placePiece(plan.piece, plan.original, path, loweredPath);
		for (const position of positions) marked.add(position);
	}
	const sorted = [...marked].sort((left, right) => left - right);
	const folder = slash < 0 ? '' : path.slice(0, slash);
	return {
		folder,
		folderMatches: rangesOf(
			sorted.filter((position) => position < slash),
			0,
		),
		name,
		nameMatches: rangesOf(
			sorted.filter((position) => position >= nameStart),
			nameStart,
		),
		path,
	};
}

/** Ascending offsets, less `offset`, joined into ranges where they run together. */
function rangesOf(positions: readonly number[], offset: number): TextRange[] {
	const ranges: TextRange[] = [];
	for (const position of positions) {
		const at = position - offset;
		const last = ranges.at(-1);
		if (last !== undefined && last.end === at) {
			ranges[ranges.length - 1] = { end: at + 1, start: last.start };
		} else {
			ranges.push({ end: at + 1, start: at });
		}
	}
	return ranges;
}
