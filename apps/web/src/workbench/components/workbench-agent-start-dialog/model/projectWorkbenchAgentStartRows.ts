/** What the dialog needs of one catalog row; the parsed wire catalog satisfies it. */
interface WorkbenchAgentStartEntry<Provider extends string = string> {
	readonly command: string;
	readonly productName: string;
	readonly providerId: Provider;
	readonly state: 'cant-check' | 'checking' | 'found' | 'not-found' | 'unknown';
}

/** What the dialog needs of one catalog; the parsed wire catalog satisfies it. */
export interface WorkbenchAgentStartCatalogView<Provider extends string = string> {
	readonly agents: readonly WorkbenchAgentStartEntry<Provider>[];
	readonly lookup: {
		readonly checkedAt: number | null;
		readonly status: 'complete' | 'failed' | 'running' | 'unsupported';
	};
	readonly recent: readonly Provider[];
}

export type WorkbenchAgentStartGroup = 'installed' | 'not-checked' | 'not-found' | 'recent';

type WorkbenchAgentStartRow<Provider extends string = string> =
	| { readonly group: WorkbenchAgentStartGroup; readonly kind: 'heading' }
	| {
			readonly detail: 'cant-check' | 'command' | 'not-found';
			readonly group: WorkbenchAgentStartGroup;
			readonly kind: 'provider';
			readonly providerId: Provider;
	  }
	| { readonly count: number; readonly kind: 'fold' };

export interface WorkbenchAgentStartRows<Provider extends string = string> {
	readonly rows: readonly WorkbenchAgentStartRow<Provider>[];
	/** The lookup's state when it shapes the list: the first lookup runs, or no lookup gave a result. */
	readonly status: 'couldnt-check' | 'looking' | null;
}

/** As many Recent Agents as the TUI's Start Agent picker shows. */
const RECENT_LIMIT = 3;

/**
 * The rows of Start Agent, as the TUI's picker lists them and the shared Agent start rows corpus
 * states them. A found Agent is listed first among Recent, then among Installed; the rest wait
 * behind one fold row, which a query or an opened fold replaces with their own group. Without any
 * lookup result every Agent stays startable, listed as not checked. A query matches the product
 * name, command or provider id, ignoring case and surrounding space.
 */
export function projectWorkbenchAgentStartRows<Provider extends string>(
	catalog: WorkbenchAgentStartCatalogView<Provider>,
	query: string,
	expanded: boolean,
): WorkbenchAgentStartRows<Provider> {
	const needle = query.trim().toLowerCase();
	const matches = (entry: WorkbenchAgentStartEntry<Provider>): boolean =>
		needle === '' ||
		entry.productName.toLowerCase().includes(needle) ||
		entry.command.toLowerCase().includes(needle) ||
		entry.providerId.toLowerCase().includes(needle);
	const byId = new Map(catalog.agents.map((entry) => [entry.providerId, entry]));
	const looking = catalog.agents.some((entry) => entry.state === 'checking');
	const checked = !looking && catalog.lookup.checkedAt !== null;
	const unchecked =
		!looking &&
		catalog.lookup.checkedAt === null &&
		(catalog.lookup.status === 'failed' || catalog.lookup.status === 'unsupported');

	const recent: WorkbenchAgentStartEntry<Provider>[] = [];
	for (const providerId of catalog.recent) {
		if (recent.length === RECENT_LIMIT) break;
		const entry = byId.get(providerId);
		if (entry === undefined || recent.includes(entry)) continue;
		if (unchecked ? entry.state !== 'not-found' : entry.state === 'found') recent.push(entry);
	}
	const rest = alphabetical(catalog.agents.filter((entry) => !recent.includes(entry)));
	const rows: WorkbenchAgentStartRow<Provider>[] = [];
	const list = (
		group: WorkbenchAgentStartGroup,
		entries: readonly WorkbenchAgentStartEntry<Provider>[],
		detail: (entry: WorkbenchAgentStartEntry<Provider>) => 'cant-check' | 'command' | 'not-found',
	): void => {
		const shown = entries.filter(matches);
		if (shown.length === 0) return;
		rows.push({ group, kind: 'heading' });
		for (const entry of shown) {
			rows.push({ detail: detail(entry), group, kind: 'provider', providerId: entry.providerId });
		}
	};

	list('recent', recent, () => 'command');
	if (unchecked) {
		list(
			'not-checked',
			rest.filter((entry) => entry.state !== 'not-found'),
			() => 'command',
		);
		list(
			'not-found',
			rest.filter((entry) => entry.state === 'not-found'),
			() => 'not-found',
		);
		return { rows, status: 'couldnt-check' };
	}

	list(
		'installed',
		rest.filter((entry) => entry.state === 'found'),
		() => 'command',
	);
	const folded = rest.filter((entry) => entry.state !== 'found' && entry.state !== 'checking');
	if (checked && needle === '' && !expanded) {
		if (folded.length > 0) rows.push({ count: folded.length, kind: 'fold' });
	} else if (!checked && needle === '' && !expanded) {
		// The first lookup runs: its fold holds rows still to come, so it is not offered yet.
	} else {
		list('not-found', folded, (entry) =>
			entry.state === 'not-found' ? 'not-found' : 'cant-check',
		);
	}
	return { rows, status: checked ? null : 'looking' };
}

function alphabetical<Entry extends { readonly productName: string }>(
	entries: readonly Entry[],
): Entry[] {
	return [...entries].sort((left, right) => {
		const a = left.productName.toLowerCase();
		const b = right.productName.toLowerCase();
		if (a === b) return 0;
		return a < b ? -1 : 1;
	});
}
