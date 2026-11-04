export function matchesPanelLayoutTopology(
	layout: Readonly<Record<string, number>>,
	childIds: readonly string[],
): boolean {
	const layoutIds = Object.keys(layout);
	return (
		layoutIds.length === childIds.length &&
		childIds.every((childId) => Number.isFinite(layout[childId]))
	);
}
