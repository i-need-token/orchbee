import type { VirtualTreeFindMatchType, VirtualTreeHighlight } from './virtual-tree-types';

/**
 * How a find's pattern matched a label, recorded as VS Code's `FuzzyScore` is: the score, the label
 * offset the match started from, which is always 0 here, and the matched positions from the last to
 * the first.
 */
export type FindScore = readonly [score: number, start: number, ...positions: number[]];

/** How many characters of a pattern or label the fuzzy match reads, as in VS Code. */
const MAX_LENGTH = 128;
const WIDTH = MAX_LENGTH + 1;

/** Where the best path into each cell comes from. */
const DIAGONAL = 1;
const LEFT = 2;
const LEFT_LEFT = 3;

// The dynamic programming tables, reused by every match: one row per pattern character and one
// column per label character, both counted from 1.
const totals = new Int32Array(WIDTH * WIDTH);
const runs = new Int32Array(WIDTH * WIDTH);
const arrows = new Uint8Array(WIDTH * WIDTH);
/** The first and last label positions each pattern character can match at. */
const firstPositions = new Int32Array(WIDTH);
const lastPositions = new Int32Array(WIDTH);

/**
 * VS Code's fuzzy find match, `fuzzyScore` with `firstMatchCanBeWeak` and `boostFullMatch`, as its
 * tree find calls it: the pattern's characters must appear in the label in order, ignoring case, and
 * the score prefers matches at the label's start, at word starts and in contiguous runs. `patternLow`
 * and `labelLow` are the lowercase forms. Only the first 128 characters of either take part.
 */
function fuzzyScore(
	pattern: string,
	patternLow: string,
	label: string,
	labelLow: string,
): FindScore | undefined {
	const patternLength = Math.min(pattern.length, MAX_LENGTH);
	const labelLength = Math.min(label.length, MAX_LENGTH);
	if (patternLength === 0 || labelLength === 0 || patternLength > labelLength) return undefined;
	if (!findFirstPositions(patternLow, patternLength, labelLow, labelLength)) return undefined;
	findLastPositions(patternLow, patternLength, labelLow, labelLength);

	let column = 0;
	for (let row = 1; row <= patternLength; row += 1) {
		const patternPosition = row - 1;
		const first = firstPositions[patternPosition] ?? 0;
		const last = lastPositions[patternPosition] ?? 0;
		const reach = row < patternLength ? (lastPositions[row] ?? 0) : labelLength;
		const cell = row * WIDTH;
		const above = cell - WIDTH;
		for (let position = first; position < reach; position += 1) {
			column = position + 1;
			let score = Number.MIN_SAFE_INTEGER;
			if (position <= last) {
				score = characterScore(
					pattern,
					patternLow,
					patternPosition,
					label,
					labelLow,
					position,
					labelLength,
					runs[above + column - 1] === 0,
				);
			}
			const fromDiagonal = score !== Number.MIN_SAFE_INTEGER;
			const diagonal = fromDiagonal ? score + (totals[above + column - 1] ?? 0) : 0;
			const fromLeft = position > first;
			const left = fromLeft
				? (totals[cell + column - 1] ?? 0) + ((runs[cell + column - 1] ?? 0) > 0 ? -5 : 0)
				: 0;
			const fromLeftLeft = position > first + 1 && (runs[cell + column - 1] ?? 0) > 0;
			const leftLeft = fromLeftLeft
				? (totals[cell + column - 2] ?? 0) + ((runs[cell + column - 2] ?? 0) > 0 ? -5 : 0)
				: 0;
			// Skipping a run leaves the match earlier in the label, which wins a tie.
			if (
				fromLeftLeft &&
				(!fromLeft || leftLeft >= left) &&
				(!fromDiagonal || leftLeft >= diagonal)
			) {
				totals[cell + column] = leftLeft;
				arrows[cell + column] = LEFT_LEFT;
				runs[cell + column] = 0;
			} else if (fromLeft && (!fromDiagonal || left >= diagonal)) {
				totals[cell + column] = left;
				arrows[cell + column] = LEFT;
				runs[cell + column] = 0;
			} else {
				totals[cell + column] = diagonal;
				arrows[cell + column] = DIAGONAL;
				runs[cell + column] = (runs[above + column - 1] ?? 0) + 1;
			}
		}
	}
	return tracePath(patternLow, patternLength, label, labelLow, labelLength);
}

/**
 * VS Code's contiguous find match: the label contains the pattern, ignoring case, and the match is
 * its first occurrence. Both arguments are lowercase.
 */
function contiguousScore(patternLow: string, labelLow: string): FindScore | undefined {
	const start = labelLow.indexOf(patternLow);
	if (start < 0) return undefined;
	const score: [number, number, ...number[]] = [Number.MAX_SAFE_INTEGER, 0];
	for (let position = start + patternLow.length - 1; position >= start; position -= 1) {
		score.push(position);
	}
	return score;
}

/**
 * Whether VS Code's tree find matches a label: what `findMatcher` scores, without scoring it. A fuzzy
 * match needs only the pattern's characters in order within the first 128, and a contiguous one the
 * pattern itself.
 */
export function findTester(
	pattern: string,
	matchType: VirtualTreeFindMatchType,
): (label: string) => boolean {
	const patternLow = pattern.toLowerCase();
	if (matchType === 'contiguous') return (label) => label.toLowerCase().includes(patternLow);
	const patternLength = Math.min(pattern.length, MAX_LENGTH);
	return (label) => {
		const labelLength = Math.min(label.length, MAX_LENGTH);
		if (patternLength === 0 || patternLength > labelLength) return false;
		// Where `findFirstPositions` finds each character, found by the engine's own search.
		const labelLow = label.toLowerCase();
		let position = 0;
		for (let index = 0; index < patternLength; index += 1) {
			const character = patternLow[index];
			if (character === undefined) return false;
			position = labelLow.indexOf(character, position) + 1;
			if (position === 0 || position > labelLength) return false;
		}
		return true;
	};
}

/** Matches labels against one pattern as VS Code's tree find does: a score, or undefined for a miss. */
export function findMatcher(
	pattern: string,
	matchType: VirtualTreeFindMatchType,
): (label: string) => FindScore | undefined {
	const patternLow = pattern.toLowerCase();
	if (matchType === 'contiguous')
		return (label) => contiguousScore(patternLow, label.toLowerCase());
	return (label) => fuzzyScore(pattern, patternLow, label, label.toLowerCase());
}

/** The label ranges a score matched, in order, joining adjacent characters into one range. */
export function scoreHighlights(score: FindScore): VirtualTreeHighlight[] {
	const highlights: { start: number; end: number }[] = [];
	const start = score[1];
	for (let index = score.length - 1; index > 1; index -= 1) {
		const position = (score[index] ?? 0) + start;
		const last = highlights.at(-1);
		if (last?.end === position) last.end = position + 1;
		else highlights.push({ end: position + 1, start: position });
	}
	return highlights;
}

/** Records where each pattern character first matches; false when the label lacks the pattern. */
function findFirstPositions(
	patternLow: string,
	patternLength: number,
	labelLow: string,
	labelLength: number,
): boolean {
	let patternPosition = 0;
	for (let position = 0; position < labelLength && patternPosition < patternLength; position += 1) {
		if (patternLow[patternPosition] !== labelLow[position]) continue;
		firstPositions[patternPosition] = position;
		patternPosition += 1;
	}
	return patternPosition === patternLength;
}

/** Records where each pattern character last matches while the characters after it still fit. */
function findLastPositions(
	patternLow: string,
	patternLength: number,
	labelLow: string,
	labelLength: number,
): void {
	let patternPosition = patternLength - 1;
	for (let position = labelLength - 1; position >= 0 && patternPosition >= 0; position -= 1) {
		if (patternLow[patternPosition] !== labelLow[position]) continue;
		lastPositions[patternPosition] = position;
		patternPosition -= 1;
	}
}

/**
 * Walks the best path back from the last cell, preferring to keep a run of three or more matched
 * characters together over the earlier position the forward pass chose, and scores the match.
 */
function tracePath(
	patternLow: string,
	patternLength: number,
	label: string,
	labelLow: string,
	labelLength: number,
): FindScore {
	let row = patternLength;
	let column = labelLength;
	const score: [number, number, ...number[]] = [totals[row * WIDTH + column] ?? 0, 0];
	let backwardRun = 0;
	let lastMatchColumn = 0;
	while (row >= 1) {
		const cell = row * WIDTH;
		let diagonalColumn = column;
		do {
			const arrow = arrows[cell + diagonalColumn];
			if (arrow === LEFT_LEFT) diagonalColumn -= 2;
			else if (arrow === LEFT) diagonalColumn -= 1;
			else break;
		} while (diagonalColumn >= 1);
		if (
			backwardRun > 1 &&
			patternLow[row - 1] === labelLow[column - 1] &&
			!isUpperCaseAt(label, labelLow, diagonalColumn - 1) &&
			backwardRun + 1 > (runs[cell + diagonalColumn] ?? 0)
		) {
			diagonalColumn = column;
		}
		backwardRun = diagonalColumn === column ? backwardRun + 1 : 1;
		if (lastMatchColumn === 0) lastMatchColumn = diagonalColumn;
		row -= 1;
		column = diagonalColumn - 1;
		score.push(column);
	}
	// A pattern that matches every character of the label comes ahead of the others.
	if (labelLength === patternLength) score[0] += 2;
	// One point off for each label character skipped before the last match.
	score[0] -= lastMatchColumn - patternLength;
	return score;
}

/** One pattern character matched at one label position, scored as VS Code's `_doScore` does. */
function characterScore(
	pattern: string,
	patternLow: string,
	patternPosition: number,
	label: string,
	labelLow: string,
	position: number,
	labelLength: number,
	startsRun: boolean,
): number {
	if (patternLow[patternPosition] !== labelLow[position]) return Number.MIN_SAFE_INTEGER;
	let score = 1;
	let atGap = false;
	if (position === patternPosition) {
		// A common prefix: `foobar` in `foobaz`.
		score = pattern[patternPosition] === label[position] ? 7 : 5;
	} else if (
		isUpperCaseAt(label, labelLow, position) &&
		(position === 0 || !isUpperCaseAt(label, labelLow, position - 1))
	) {
		// The start of a capitalized word: `foo` in `forOthers`.
		score = pattern[patternPosition] === label[position] ? 7 : 5;
		atGap = true;
	} else if (
		isSeparatorAt(labelLow, position) &&
		(position === 0 || !isSeparatorAt(labelLow, position - 1))
	) {
		// A separator itself: `.` in `foo.bar`.
		score = 5;
	} else if (isSeparatorAt(labelLow, position - 1) || isWhitespaceAt(labelLow, position - 1)) {
		// Right after a separator: `foo` in `bar_foo`.
		score = 5;
		atGap = true;
	}
	if (!atGap) {
		atGap =
			isUpperCaseAt(label, labelLow, position) ||
			isSeparatorAt(labelLow, position - 1) ||
			isWhitespaceAt(labelLow, position - 1);
	}
	if (patternPosition === 0) {
		// The first pattern character pays for the label characters before it.
		if (position > 0) score -= atGap ? 3 : 5;
	} else if (startsRun) {
		score += atGap ? 2 : 0;
	} else {
		score += atGap ? 0 : 1;
	}
	// Every match pays for the gap after it, so a match on the label's last character pays too.
	if (position + 1 === labelLength) score -= atGap ? 3 : 5;
	return score;
}

function isUpperCaseAt(label: string, labelLow: string, position: number): boolean {
	return label[position] !== labelLow[position];
}

function isWhitespaceAt(text: string, position: number): boolean {
	const code = text.charCodeAt(position);
	return code === 0x20 || code === 0x09;
}

/** The characters that separate words, and emoji, as VS Code's fuzzy match counts them. */
function isSeparatorAt(text: string, position: number): boolean {
	if (position < 0 || position >= text.length) return false;
	const code = text.codePointAt(position);
	if (code === undefined) return false;
	return SEPARATORS.has(code) || isEmoji(code);
}

const SEPARATORS: ReadonlySet<number> = new Set(
	[...'_-. /\\\'":$<>()[]{}'].map((character) => character.charCodeAt(0)),
);

/** VS Code's `isEmojiImprecise`. */
function isEmoji(code: number): boolean {
	return (
		(code >= 0x1f1e6 && code <= 0x1f1ff) ||
		code === 8986 ||
		code === 8987 ||
		code === 9200 ||
		code === 9203 ||
		(code >= 9728 && code <= 10175) ||
		code === 11088 ||
		code === 11093 ||
		(code >= 127744 && code <= 128591) ||
		(code >= 128640 && code <= 128764) ||
		(code >= 128992 && code <= 129008) ||
		(code >= 129280 && code <= 129535) ||
		(code >= 129648 && code <= 129782)
	);
}
