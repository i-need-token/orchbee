import type { TerminalBufferRow } from './buffer-rows';
import type { TerminalOpenTarget } from './terminal-contracts';
import {
	buildLogicalTerminalTexts,
	detectTerminalRowLinks,
	isPermittedExternalTerminalTarget,
	terminalLinkContains,
} from './terminal-row-links';
import type { ILinkRange } from './types';

const HASH_REGEX = /(?:^|[^0-9a-f])([0-9a-f]{7,64})(?![0-9a-f])/gu;
const MAXIMUM_VISIBLE_TERMINAL_HINTS = 512;
const TERMINAL_HINT_ALPHABET = '0123456789abcdefghijklmnopqrstuvwxyz';

export type TerminalHintKind = 'file-line' | 'hash' | 'path' | 'url';

export interface TerminalHint {
	readonly kind: TerminalHintKind;
	readonly range: ILinkRange;
	readonly target: TerminalOpenTarget;
	readonly text: string;
}

export type TerminalHintChoiceResult =
	| { readonly hint: TerminalHint; readonly kind: 'selected' }
	| { readonly kind: 'cancelled' | 'empty' | 'stale' | 'unavailable' };

/** Detect bounded URL, path, file-location and hash targets in screen order. */
export function detectTerminalHints(
	rows: readonly TerminalBufferRow[],
	currentDirectory = '',
): readonly TerminalHint[] {
	const links = detectTerminalRowLinks(rows, currentDirectory);
	const hints: TerminalHint[] = [];
	for (const link of links) {
		const target = link.target;
		if (target?.kind === 'url') {
			if (!isPermittedExternalTerminalTarget(link)) continue;
			hints.push({ kind: 'url', range: link.range, target, text: link.text });
			continue;
		}
		if (target?.kind === 'file') {
			hints.push({
				kind: target.line === undefined ? 'path' : 'file-line',
				range: link.range,
				target,
				text: link.text,
			});
		}
	}
	for (const logical of buildLogicalTerminalTexts(rows)) {
		HASH_REGEX.lastIndex = 0;
		let match: RegExpExecArray | null = HASH_REGEX.exec(logical.text);
		while (match !== null) {
			const text = match[1];
			const prefixLength = match[0].length - (text?.length ?? 0);
			const startIndex = match.index + prefixLength;
			const start = logical.positions[startIndex];
			const end = text === undefined ? undefined : logical.positions[startIndex + text.length - 1];
			if (text !== undefined && start !== undefined && end !== undefined) {
				const range = { end, start };
				if (!hints.some((hint) => rangesOverlap(range, hint.range))) {
					hints.push({
						kind: 'hash',
						range,
						target: { kind: 'unknown', value: text },
						text,
					});
				}
			}
			match = HASH_REGEX.exec(logical.text);
		}
	}
	return hints
		.sort((left, right) =>
			left.range.start.y === right.range.start.y
				? left.range.start.x - right.range.start.x
				: left.range.start.y - right.range.start.y,
		)
		.slice(0, MAXIMUM_VISIBLE_TERMINAL_HINTS);
}

/** Remove logical lines clipped by the top or bottom edge of the current viewport. */
export function completeTerminalHintRows(
	rows: readonly TerminalBufferRow[],
): readonly TerminalBufferRow[] {
	let start = 0;
	if (rows[0]?.wrapContinuation === true) {
		while (start < rows.length) {
			const row = rows[start];
			start += 1;
			if (row?.wrap === false) break;
		}
	}
	let end = rows.length;
	if (end > start && rows[end - 1]?.wrap === true) {
		let logicalStart = end - 1;
		while (logicalStart > start && rows[logicalStart]?.wrapContinuation === true) {
			logicalStart -= 1;
		}
		end = logicalStart;
	}
	return rows.slice(start, end);
}

/** Generate equal-width, prefix-free base-36 labels in screen order. */
export function terminalHintLabels(count: number): readonly string[] {
	if (!Number.isSafeInteger(count) || count < 0 || count > MAXIMUM_VISIBLE_TERMINAL_HINTS) {
		throw new RangeError('Terminal hint count is outside the bounded visible-hint contract.');
	}
	if (count === 0) return [];
	let width = 1;
	let capacity = TERMINAL_HINT_ALPHABET.length;
	while (capacity < count) {
		width += 1;
		capacity *= TERMINAL_HINT_ALPHABET.length;
	}
	return Array.from({ length: count }, (_value, index) => encodeHint(index, width));
}

function encodeHint(index: number, width: number): string {
	const encoded = Array.from({ length: width }, () => TERMINAL_HINT_ALPHABET[0] ?? '0');
	let remaining = index;
	for (let position = width - 1; position >= 0; position -= 1) {
		encoded[position] = TERMINAL_HINT_ALPHABET[remaining % TERMINAL_HINT_ALPHABET.length] ?? '0';
		remaining = Math.floor(remaining / TERMINAL_HINT_ALPHABET.length);
	}
	return encoded.join('');
}

function rangesOverlap(left: ILinkRange, right: ILinkRange): boolean {
	return (
		terminalLinkContains({ range: left, text: '' }, right.start.x, right.start.y) ||
		terminalLinkContains({ range: left, text: '' }, right.end.x, right.end.y) ||
		terminalLinkContains({ range: right, text: '' }, left.start.x, left.start.y)
	);
}
