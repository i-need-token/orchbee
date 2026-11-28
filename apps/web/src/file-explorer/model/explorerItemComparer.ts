import type { OrchbeeLocale } from '@orchbee/i18n';

/** What the Explorer's order reads of an item. */
interface ExplorerOrderItem {
	readonly directory: boolean;
	readonly name: string;
}

type ExplorerItemComparer = (one: ExplorerOrderItem, other: ExplorerOrderItem) => number;

const comparers = new Map<OrchbeeLocale, ExplorerItemComparer>();

/**
 * VS Code's default Explorer order: folders before files, then names by the locale's numeric
 * collation, the shorter name first where the collation ties them, as `foo1` and `foo01`. VS Code
 * collates in the runtime's default locale, which its desktop sets to the display language, so the
 * order follows the UI locale. Names that still tie keep their listing order.
 */
export function explorerItemComparer(locale: OrchbeeLocale): ExplorerItemComparer {
	const cached = comparers.get(locale);
	if (cached !== undefined) return cached;
	const collator = new Intl.Collator(locale, { numeric: true });
	const compare: ExplorerItemComparer = (one, other) => {
		if (one.directory !== other.directory) return one.directory ? -1 : 1;
		const order = collator.compare(one.name, other.name);
		if (order !== 0 || one.name.length === other.name.length) return order;
		return one.name.length < other.name.length ? -1 : 1;
	};
	comparers.set(locale, compare);
	return compare;
}
