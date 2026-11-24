/** An order is either absent or a complete permutation; identity order is the display order then. */
export function inDisplayOrder<Item>(
	items: readonly Item[],
	order: readonly string[] | undefined,
	identityOf: (item: Item) => string,
): readonly Item[] {
	if (order === undefined || order.length === 0) return items;
	const positions = new Map(order.map((identity, position) => [identity, position]));
	const positionOf = (item: Item): number => positions.get(identityOf(item)) ?? order.length;
	return [...items].sort((left, right) => positionOf(left) - positionOf(right));
}
