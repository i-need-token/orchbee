interface ContiguousBlockMove {
	readonly length: number;
	readonly preRemovalTargetIndex: number;
	readonly start: number;
}

function clamp(value: number, minimum: number, maximum: number): number {
	return Math.min(Math.max(value, minimum), maximum);
}

/**
 * Moves one contiguous block using Chromium's pre-removal target convention.
 * A target inside the moving block is an intentional no-op.
 */
export function moveContiguousBlock<T>(
	items: readonly T[],
	move: Readonly<ContiguousBlockMove>,
): readonly T[] {
	if (
		!Number.isInteger(move.start) ||
		!Number.isInteger(move.length) ||
		!Number.isInteger(move.preRemovalTargetIndex) ||
		move.start < 0 ||
		move.length <= 0 ||
		move.start + move.length > items.length
	) {
		throw new RangeError('The contiguous block must be a non-empty range inside the input.');
	}

	const target = clamp(move.preRemovalTargetIndex, 0, items.length);
	const blockEnd = move.start + move.length;
	if (target >= move.start && target <= blockEnd) return [...items];

	const moving = items.slice(move.start, blockEnd);
	const remaining = [...items.slice(0, move.start), ...items.slice(blockEnd)];
	const insertion = target < move.start ? target : target - move.length;
	remaining.splice(clamp(insertion, 0, remaining.length), 0, ...moving);
	return remaining;
}
