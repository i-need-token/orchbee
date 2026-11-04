import type { WorkbenchSearchFilter } from '../../../types/workbench-search';
import type {
	WorkbenchSearchArgumentOption,
	WorkbenchSearchCatalog,
	WorkbenchSearchResult,
	WorkbenchSearchSection,
} from './workbench-search-result';

interface SelectWorkbenchSearchSectionsOptions {
	readonly catalog: WorkbenchSearchCatalog;
	readonly filter: WorkbenchSearchFilter;
	readonly providerSections?: readonly WorkbenchSearchSection[] | undefined;
	readonly query: string;
	readonly recentResultIds?: readonly string[] | undefined;
}

export function selectWorkbenchSearchSections({
	catalog,
	filter,
	providerSections = [],
	query,
	recentResultIds = [],
}: SelectWorkbenchSearchSectionsOptions): readonly WorkbenchSearchSection[] {
	const normalizedQuery = normalizeSearchText(query);
	let localResults: readonly WorkbenchSearchResult[];
	if (filter === 'settings') {
		localResults = catalog.settingsResults;
	} else if (filter === 'actions') {
		localResults = catalog.actionResults;
	} else if (filter === 'agents') {
		localResults = [];
	} else if (normalizedQuery.length === 0) {
		localResults = [
			...catalog.actionResults
				.filter((result) => result.recommended)
				.map((result) => ({
					...result,
					groupId: 'recommended-actions',
					groupLabel: catalog.recommendedGroupLabel,
					groupOrder: 10,
				})),
			...catalog.settingsResults,
		];
	} else {
		localResults = [
			...catalog.actionResults.filter(
				(result) =>
					!catalog.settingsResults.some((setting) => setting.actionId === result.actionId),
			),
			...catalog.settingsResults,
		];
	}

	const results = [...localResults, ...providerSections.flatMap((section) => section.results)];
	const ordinaryMatching = results
		.filter((result) => result.visibility !== 'when-no-results')
		.filter((result) => matchSearchResult(result, normalizedQuery) !== null)
		.sort((left, right) => compareSearchResults(left, right, normalizedQuery, recentResultIds));
	const matching =
		ordinaryMatching.length > 0
			? ordinaryMatching
			: results
					.filter((result) => result.visibility === 'when-no-results')
					.sort((left, right) =>
						compareSearchResults(left, right, normalizedQuery, recentResultIds),
					);
	const groups = new Map<string, WorkbenchSearchSection>();
	for (const result of matching) {
		const current = groups.get(result.groupId);
		groups.set(result.groupId, {
			id: result.groupId,
			label: result.groupLabel,
			order: result.groupOrder,
			results: current === undefined ? [result] : [...current.results, result],
		});
	}
	const sections = [...groups.values()];
	return normalizedQuery.length === 0
		? sections.sort(
				(left, right) => left.order - right.order || compareStableText(left.id, right.id),
			)
		: sections;
}

export function filterWorkbenchSearchArgumentOptions(
	options: readonly WorkbenchSearchArgumentOption[],
	query: string,
): readonly WorkbenchSearchArgumentOption[] {
	const normalizedQuery = normalizeSearchText(query);
	if (normalizedQuery.length === 0) {
		return options;
	}
	return options
		.slice()
		.filter((option) =>
			[option.label, ...option.keywords].some((candidate) =>
				normalizeSearchText(candidate).includes(normalizedQuery),
			),
		)
		.sort((left, right) => compareStableText(left.id, right.id));
}

export function isWorkbenchSearchResultEnabled(result: WorkbenchSearchResult): boolean {
	return result.availability.kind === 'enabled' && result.surfaceDisabledReason === undefined;
}

export function resolveWorkbenchSearchHighlight(
	currentId: string | null,
	resultIds: readonly string[],
	enabledResultIds: readonly string[],
): string | null {
	if (currentId !== null && resultIds.includes(currentId)) {
		return currentId;
	}
	return enabledResultIds[0] ?? resultIds[0] ?? null;
}

function compareSearchResults(
	left: WorkbenchSearchResult,
	right: WorkbenchSearchResult,
	query: string,
	recentResultIds: readonly string[],
): number {
	const tierDifference =
		(matchSearchResult(left, query) ?? Number.POSITIVE_INFINITY) -
		(matchSearchResult(right, query) ?? Number.POSITIVE_INFINITY);
	if (tierDifference !== 0) {
		return tierDifference;
	}
	if (left.recommended !== right.recommended) {
		return left.recommended ? -1 : 1;
	}
	const leftRecency = recentResultIds.indexOf(left.id);
	const rightRecency = recentResultIds.indexOf(right.id);
	if (leftRecency !== rightRecency) {
		if (leftRecency === -1) return 1;
		if (rightRecency === -1) return -1;
		return leftRecency - rightRecency;
	}
	const priorityDifference = (left.providerPriority ?? 0) - (right.providerPriority ?? 0);
	if (priorityDifference !== 0) {
		return priorityDifference;
	}
	return compareStableText(left.id, right.id);
}

function matchSearchResult(result: WorkbenchSearchResult, normalizedQuery: string): number | null {
	if (normalizedQuery.length === 0) {
		return 0;
	}
	const label = normalizeSearchText(result.label);
	if (label === normalizedQuery) {
		return 0;
	}
	if (label.startsWith(normalizedQuery)) {
		return 1;
	}
	if (label.split(' ').some((token) => token.startsWith(normalizedQuery))) {
		return 2;
	}
	if (result.keywords.some((keyword) => normalizeSearchText(keyword).startsWith(normalizedQuery))) {
		return 3;
	}
	const canonicalLabel = normalizeSearchText(result.canonicalLabel);
	if (canonicalLabel !== label && canonicalLabel.includes(normalizedQuery)) {
		return 4;
	}
	if (
		isSubsequence(normalizedQuery, label) ||
		result.keywords.some((keyword) => isSubsequence(normalizedQuery, normalizeSearchText(keyword)))
	) {
		return 5;
	}
	return null;
}

function normalizeSearchText(value: string): string {
	return value.normalize('NFKC').toLocaleLowerCase().trim().replaceAll(/\s+/gu, ' ');
}

function isSubsequence(needle: string, candidate: string): boolean {
	let needleIndex = 0;
	for (const character of candidate) {
		if (character === needle[needleIndex]) {
			needleIndex += 1;
		}
		if (needleIndex === needle.length) {
			return true;
		}
	}
	return false;
}

function compareStableText(left: string, right: string): number {
	if (left < right) return -1;
	if (left > right) return 1;
	return 0;
}
