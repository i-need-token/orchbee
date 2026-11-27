/**
 * Positions, ascending, of one longest strictly increasing subsequence of `values`, in O(n log n).
 * A refresh keeps the rows of children whose old positions form it and moves the others.
 */
export function longestIncreasingSubsequence(values: readonly number[]): number[] {
	// tails[length - 1] is the position ending the increasing run of that length with the smallest value.
	const tails: number[] = [];
	const previous = new Int32Array(values.length);
	for (let position = 0; position < values.length; position += 1) {
		const value = values[position] ?? 0;
		let low = 0;
		let high = tails.length;
		while (low < high) {
			const middle = (low + high) >>> 1;
			if ((values[tails[middle] ?? 0] ?? 0) < value) low = middle + 1;
			else high = middle;
		}
		previous[position] = low > 0 ? (tails[low - 1] ?? -1) : -1;
		tails[low] = position;
	}
	const result: number[] = Array.from({ length: tails.length }, () => 0);
	let position = tails.at(-1) ?? -1;
	for (let index = tails.length - 1; index >= 0; index -= 1) {
		result[index] = position;
		position = previous[position] ?? -1;
	}
	return result;
}
