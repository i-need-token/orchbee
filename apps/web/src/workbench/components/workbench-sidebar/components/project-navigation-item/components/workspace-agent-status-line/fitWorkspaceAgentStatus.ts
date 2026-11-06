/** What a folded Workspace's status line measures, in CSS pixels. */
export interface WorkspaceAgentStatusMetrics {
	/** The width the line may take. */
	readonly available: number;
	/** The width of one state glyph. */
	readonly glyph: number;
	/** The gap between neighboring items on the line. */
	readonly gap: number;
	/** The width of each `+N` remainder with its leading margin, at index N - 1. */
	readonly remainders: readonly number[];
}

/** How many of a Workspace's Agent states the line shows, and how many its `+N` counts. */
export interface WorkspaceAgentStatusFit {
	/** Glyphs from the line's start, most urgent first. */
	readonly shown: number;
	/**
	 * Agents behind the `+N`. Zero when every state shows, and also when not even the remainder fits,
	 * so the line shows nothing.
	 */
	readonly hidden: number;
}

/**
 * Fits `total` Agent states, most urgent first, into the line. All of them show when their glyphs
 * fit with the gaps between them. Otherwise the line shows the largest count whose glyphs and the
 * `+N` remainder of the rest fit together, with one gap before the remainder.
 */
export function fitWorkspaceAgentStatus(
	total: number,
	metrics: WorkspaceAgentStatusMetrics,
): WorkspaceAgentStatusFit {
	const { available, gap, glyph, remainders } = metrics;
	const run = (count: number): number => count * glyph + Math.max(count - 1, 0) * gap;
	if (run(total) <= available) {
		return { hidden: 0, shown: total };
	}
	for (let shown = total - 1; shown >= 0; shown -= 1) {
		const hidden = total - shown;
		// A remainder that was not measured never fits, so the line shows fewer rather than overflow.
		const remainder = remainders[hidden - 1] ?? Number.POSITIVE_INFINITY;
		const beside = shown === 0 ? remainder : run(shown) + gap + remainder;
		if (beside <= available) {
			return { hidden, shown };
		}
	}
	return { hidden: 0, shown: 0 };
}
