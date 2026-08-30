import type { TerminalSettingsSearchEntry } from '../types/terminal-settings-surface';

interface ResolveTerminalSettingsSearchOptions {
	readonly entries: readonly TerminalSettingsSearchEntry[];
	readonly locale: string;
	readonly query: string;
}

export function resolveTerminalSettingsSearch({
	entries,
	locale,
	query,
}: ResolveTerminalSettingsSearchOptions): readonly TerminalSettingsSearchEntry[] {
	const normalize = (value: string): string =>
		value.normalize('NFKC').trim().toLocaleLowerCase(locale);
	const normalizedQuery = normalize(query);
	if (normalizedQuery.length === 0) return [];
	const queryTerms = normalizedQuery.split(/\s+/u);
	const exactCategoryMatches = entries.filter(
		(entry) => entry.kind === 'category' && normalize(entry.label) === normalizedQuery,
	);
	if (exactCategoryMatches.length > 0) return exactCategoryMatches;

	return entries
		.map((entry, order) => {
			const normalizedLabel = normalize(entry.label);
			const normalizedSection = entry.sectionLabel === null ? '' : normalize(entry.sectionLabel);
			const normalizedDescription = normalize(entry.description);
			const normalizedKeywords = entry.keywords.map(normalize);
			const searchableValues =
				entry.kind === 'category'
					? [normalizedLabel]
					: [normalizedLabel, normalizedSection, normalizedDescription, ...normalizedKeywords];
			const matches = queryTerms.every((term) =>
				searchableValues.some((value) => value.includes(term)),
			);
			if (!matches) return null;

			let score = 6;
			if (normalizedLabel === normalizedQuery) score = 0;
			else if (normalizedLabel.startsWith(normalizedQuery)) score = 1;
			else if (normalizedLabel.includes(normalizedQuery)) score = 2;
			else if (queryTerms.every((term) => normalizedLabel.includes(term))) score = 3;
			else if (normalizedSection.includes(normalizedQuery)) score = 4;
			else if (normalizedKeywords.some((value) => value.includes(normalizedQuery))) score = 5;
			return { entry, order, score };
		})
		.filter((match) => match !== null)
		.sort((left, right) => left.score - right.score || left.order - right.order)
		.map((match) => match.entry);
}
