interface AdjacentPanelLayoutProjection {
	readonly leadingWeight: number;
	readonly layout: Readonly<Record<string, number>>;
}

export function projectAdjacentPanelLayout(
	childIds: readonly string[],
	separatorIndex: number,
	initialLayout: Readonly<Record<string, number>>,
	candidateLayout: Readonly<Record<string, number>>,
	minimumChildWeight: number,
): AdjacentPanelLayoutProjection | null {
	const leadingId = childIds[separatorIndex];
	const trailingId = childIds[separatorIndex + 1];
	const initialLeading = leadingId === undefined ? undefined : initialLayout[leadingId];
	const initialTrailing = trailingId === undefined ? undefined : initialLayout[trailingId];
	const candidateLeading = leadingId === undefined ? undefined : candidateLayout[leadingId];
	if (
		leadingId === undefined ||
		trailingId === undefined ||
		initialLeading === undefined ||
		initialTrailing === undefined ||
		candidateLeading === undefined ||
		!Number.isFinite(initialLeading) ||
		!Number.isFinite(initialTrailing) ||
		!Number.isFinite(candidateLeading)
	) {
		return null;
	}
	const pairWeight = initialLeading + initialTrailing;
	const effectiveMinimumChildWeight = Math.min(Math.max(minimumChildWeight, 0), pairWeight / 2);
	const leadingWeight = Math.min(
		Math.max(candidateLeading, effectiveMinimumChildWeight),
		pairWeight - effectiveMinimumChildWeight,
	);
	return {
		leadingWeight,
		layout: {
			...initialLayout,
			[leadingId]: leadingWeight,
			[trailingId]: pairWeight - leadingWeight,
		},
	};
}
