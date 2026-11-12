import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	agentStartMessages,
	type MessageDescriptor,
	useOrchbeeIntl,
	workbenchMessages,
} from '@orchbee/i18n';
import type { WorkbenchAgentStartProvider } from '@orchbee/workbench-client-protocol';
import * as React from 'react';

import type {
	WorkbenchAgentCatalogSnapshot,
	WorkbenchAgentCatalogSource,
	WorkbenchAgentStartInput,
	WorkbenchAgentStartPlace,
} from '../../../types/workbench-agent-start';
import {
	projectWorkbenchAgentStartRows,
	type WorkbenchAgentStartHolds,
	type WorkbenchAgentStartGroup,
	type WorkbenchAgentStartRows,
} from '../model/projectWorkbenchAgentStartRows';

interface UseWorkbenchAgentStartDialogOptions {
	readonly catalog: WorkbenchAgentCatalogSource;
	readonly onStart: (input: WorkbenchAgentStartInput) => void;
	readonly places: readonly WorkbenchAgentStartPlace[];
}

type Highlight = WorkbenchAgentStartProvider | 'fold';

interface OptionView {
	readonly elementId: string;
	readonly elementRef: React.RefObject<HTMLDivElement | null> | undefined;
	readonly highlighted: boolean;
	readonly onClick: () => void;
	readonly onDoubleClick: () => void;
}

export interface WorkbenchAgentStartProviderRowView extends OptionView {
	readonly detail: string;
	readonly kind: 'provider';
	readonly name: string;
	readonly providerId: WorkbenchAgentStartProvider;
}

interface WorkbenchAgentStartSectionView {
	readonly headingId: string;
	readonly key: string;
	readonly kind: 'section';
	readonly label: string;
	readonly rows: readonly WorkbenchAgentStartProviderRowView[];
}

interface WorkbenchAgentStartFoldView extends OptionView {
	readonly key: 'fold';
	readonly kind: 'fold';
	readonly label: string;
}

interface WorkbenchAgentStartDialogView {
	readonly activeDescendant: string | undefined;
	readonly canStart: boolean;
	readonly cancelLabel: string;
	readonly closeLabel: string;
	readonly gradientPrefix: string;
	readonly listId: string;
	readonly listLabel: string;
	readonly onPlaceChange: (event: React.ChangeEvent<HTMLSelectElement>) => void;
	readonly onQueryChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
	readonly onSearchKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => void;
	readonly opensInLabel: string;
	readonly place: WorkbenchAgentStartPlace | '';
	readonly placeId: string;
	readonly places: readonly { readonly label: string; readonly value: WorkbenchAgentStartPlace }[];
	readonly query: string;
	readonly searchLabel: string;
	readonly searchRef: React.RefObject<HTMLInputElement | null>;
	readonly sections: readonly (WorkbenchAgentStartFoldView | WorkbenchAgentStartSectionView)[];
	readonly start: () => void;
	readonly startLabel: string;
	readonly status: { readonly message: string; readonly warning: boolean } | null;
	readonly title: string;
}

const GROUP_MESSAGES: Readonly<
	Record<Exclude<WorkbenchAgentStartGroup, 'not-found'>, MessageDescriptor>
> = {
	installed: agentStartMessages.groupInstalled,
	'not-checked': agentStartMessages.groupNotChecked,
	recent: agentStartMessages.groupRecent,
};

/** The Not found group's heading, named by the rows it lists as the fold row is. */
const NOT_FOUND_HEADINGS: Readonly<Record<WorkbenchAgentStartHolds, MessageDescriptor>> = {
	both: agentStartMessages.groupNotFoundOrCantCheck,
	'cant-check': agentStartMessages.groupCantCheck,
	'not-found': agentStartMessages.groupNotFound,
};

const FOLD_LABELS: Readonly<Record<WorkbenchAgentStartHolds, MessageDescriptor>> = {
	both: agentStartMessages.foldBoth,
	'cant-check': agentStartMessages.foldCantCheck,
	'not-found': agentStartMessages.foldNotFound,
};

const PLACE_MESSAGES: Readonly<Record<WorkbenchAgentStartPlace, MessageDescriptor>> = {
	'new-tab': agentStartMessages.placeNewTab,
	'new-worktree': agentStartMessages.placeNewWorktree,
	'split-down': agentStartMessages.placeSplitDown,
	'split-right': agentStartMessages.placeSplitRight,
};

/** Before the first catalog arrives the dialog says the lookup runs and lists nothing. */
const NO_CATALOG_ROWS: WorkbenchAgentStartRows<WorkbenchAgentStartProvider> = {
	rows: [],
	status: 'looking',
};

/**
 * Start Agent as the TUI's picker keeps it: the search field holds the keyboard, the highlight
 * follows its Agent as rows change and starts on the first row, Enter starts the highlighted Agent
 * or unfolds, and Escape clears a query before it closes. Opening asks the Server for a new lookup.
 */
export function useWorkbenchAgentStartDialog({
	catalog,
	onStart,
	places,
}: UseWorkbenchAgentStartDialogOptions): WorkbenchAgentStartDialogView {
	const intl = useOrchbeeIntl();
	const idPrefix = React.useId();
	const searchRef = React.useRef<HTMLInputElement>(null);
	const highlightedElement = React.useRef<HTMLDivElement>(null);
	const snapshot = React.useSyncExternalStore(
		catalog.subscribe,
		catalog.getSnapshot,
		catalog.getSnapshot,
	);
	const [query, setQuery] = React.useState('');
	const [expanded, setExpanded] = React.useState(false);
	const [chosen, setChosen] = React.useState<Highlight | null>(null);
	const [chosenPlace, setChosenPlace] = React.useState<WorkbenchAgentStartPlace | null>(null);

	React.useEffect(() => {
		catalog.refresh();
	}, [catalog]);

	const model = React.useMemo(
		() =>
			snapshot === null
				? NO_CATALOG_ROWS
				: projectWorkbenchAgentStartRows(snapshot, query, expanded),
		[expanded, query, snapshot],
	);
	const highlights = model.rows.flatMap<Highlight>((row) => {
		if (row.kind === 'provider') return [row.providerId];
		return row.kind === 'fold' ? ['fold'] : [];
	});
	const highlightedIndex = Math.max(0, chosen === null ? 0 : highlights.indexOf(chosen));
	const highlighted = highlights[highlightedIndex] ?? null;
	const place =
		chosenPlace !== null && places.includes(chosenPlace) ? chosenPlace : (places[0] ?? null);
	const startable = snapshot?.startable === true;
	const canStart = startable && place !== null && highlighted !== null && highlighted !== 'fold';

	React.useEffect(() => {
		highlightedElement.current?.scrollIntoView?.({ block: 'nearest' });
	}, [highlighted]);

	const elementId = (key: Highlight): string => `${idPrefix}-${key}`;
	const startProvider = useMemoizedFn((providerId: WorkbenchAgentStartProvider) => {
		if (startable && place !== null) onStart({ place, providerId });
	});
	const unfold = useMemoizedFn(() => {
		setExpanded(true);
		// The first Agent the fold held takes the highlight the fold row had.
		const opened =
			snapshot === null ? [] : projectWorkbenchAgentStartRows(snapshot, query, true).rows;
		const first = opened.find((row) => row.kind === 'provider' && row.group === 'not-found');
		setChosen(first?.kind === 'provider' ? first.providerId : null);
	});
	const activate = useMemoizedFn((key: Highlight) => {
		if (key === 'fold') unfold();
		else startProvider(key);
	});
	const start = useMemoizedFn(() => {
		if (highlighted !== null && highlighted !== 'fold') startProvider(highlighted);
	});
	const move = (offset: number): void => {
		const next =
			highlights[Math.min(Math.max(highlightedIndex + offset, 0), highlights.length - 1)];
		if (next !== undefined) setChosen(next);
	};
	const onSearchKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLInputElement>) => {
		if (event.altKey || event.ctrlKey || event.metaKey || event.nativeEvent.isComposing) return;
		switch (event.key) {
			case 'ArrowDown':
				move(1);
				break;
			case 'ArrowUp':
				move(-1);
				break;
			case 'Enter':
				if (highlighted !== null) activate(highlighted);
				break;
			case 'Escape':
				if (query === '') return;
				// A query goes first; only an empty field lets Escape close the dialog.
				event.stopPropagation();
				setQuery('');
				break;
			default:
				return;
		}
		event.preventDefault();
	});

	const entries = new Map<
		WorkbenchAgentStartProvider,
		WorkbenchAgentCatalogSnapshot['agents'][number]
	>(snapshot?.agents.map((entry) => [entry.providerId, entry]) ?? []);
	const option = (key: Highlight): OptionView => ({
		elementId: elementId(key),
		elementRef: key === highlighted ? highlightedElement : undefined,
		highlighted: key === highlighted,
		onClick: () => (key === 'fold' ? unfold() : setChosen(key)),
		onDoubleClick: () => activate(key),
	});
	const sections: (WorkbenchAgentStartFoldView | WorkbenchAgentStartSectionView)[] = [];
	// Each heading opens the group its provider rows join.
	let groupRows: WorkbenchAgentStartProviderRowView[] = [];
	for (const row of model.rows) {
		switch (row.kind) {
			case 'heading':
				groupRows = [];
				sections.push({
					headingId: `${idPrefix}-heading-${row.group}`,
					key: row.group,
					kind: 'section',
					label: intl.formatMessage(
						row.group === 'not-found' ? NOT_FOUND_HEADINGS[row.holds] : GROUP_MESSAGES[row.group],
					),
					rows: groupRows,
				});
				break;
			case 'provider': {
				const entry = entries.get(row.providerId);
				if (entry === undefined) break;
				groupRows.push({
					...option(row.providerId),
					detail:
						row.detail === 'command'
							? entry.command
							: intl.formatMessage(
									row.detail === 'not-found'
										? agentStartMessages.detailNotFound
										: agentStartMessages.detailCantCheck,
								),
					kind: 'provider',
					name: entry.productName,
					providerId: row.providerId,
				});
				break;
			}
			case 'fold':
				sections.push({
					...option('fold'),
					key: 'fold',
					kind: 'fold',
					label: intl.formatMessage(FOLD_LABELS[row.holds], { count: row.count }),
				});
				break;
		}
	}
	const providerShown = model.rows.some((row) => row.kind === 'provider');

	return {
		activeDescendant: highlighted === null ? undefined : elementId(highlighted),
		canStart,
		cancelLabel: intl.formatMessage(agentStartMessages.cancel),
		closeLabel: intl.formatMessage(workbenchMessages.resourceClose),
		gradientPrefix: idPrefix,
		listId: `${idPrefix}-list`,
		listLabel: intl.formatMessage(agentStartMessages.list),
		onPlaceChange: (event) => {
			const next = places.find((candidate) => candidate === event.target.value);
			if (next !== undefined) setChosenPlace(next);
		},
		onQueryChange: (event) => setQuery(event.target.value),
		onSearchKeyDown,
		opensInLabel: intl.formatMessage(agentStartMessages.opensIn),
		place: place ?? '',
		placeId: `${idPrefix}-place`,
		places: places.map((value) => ({ label: intl.formatMessage(PLACE_MESSAGES[value]), value })),
		query,
		searchLabel: intl.formatMessage(agentStartMessages.search),
		searchRef,
		sections,
		start,
		startLabel: intl.formatMessage(agentStartMessages.start),
		status: projectStatus({
			format: (descriptor) => intl.formatMessage(descriptor),
			noMatch: query.trim() !== '' && !providerShown,
			startable: snapshot === null || startable,
			status: model.status,
		}),
		title: intl.formatMessage(agentStartMessages.title),
	};
}

/** One line, highest first: no start on this platform, the lookup's state, then no match. */
function projectStatus({
	format,
	noMatch,
	startable,
	status,
}: {
	readonly format: (descriptor: MessageDescriptor) => string;
	readonly noMatch: boolean;
	readonly startable: boolean;
	readonly status: WorkbenchAgentStartRows['status'];
}): WorkbenchAgentStartDialogView['status'] {
	if (!startable) return { message: format(agentStartMessages.statusUnsupported), warning: true };
	if (status === 'looking')
		return { message: format(agentStartMessages.statusLooking), warning: false };
	if (status === 'couldnt-check') {
		return { message: format(agentStartMessages.statusCouldntCheck), warning: true };
	}
	if (noMatch) return { message: format(agentStartMessages.statusNoMatch), warning: false };
	return null;
}
