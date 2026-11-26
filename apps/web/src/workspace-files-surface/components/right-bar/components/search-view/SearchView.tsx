import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import {
	Button,
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
	ProductIcon,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchWorkspaceFilesSource } from '../../../../../workbench';
import {
	matchRange,
	type SearchMatchTarget,
	type SearchOutcome,
	type SearchResultLine,
	searchRows,
} from '../../../../search-results';
import type { SearchDraft, SearchSessions } from '../../../../SearchSessions';
import {
	type OpenMatchHow,
	SearchResultList,
} from './components/search-result-list/SearchResultList';
import { SearchForm } from './components/search-form/SearchForm';
import { prepareSearch, searchKey } from './prepare-search';

import styles from './SearchView.module.css';

export interface SearchViewProps {
	readonly source: WorkbenchWorkspaceFilesSource;
	readonly workspaceId: string;
	/** Every Workspace's search, which outlives the view. */
	readonly searches: SearchSessions;
	/** The files the float window has open, for searching only those. */
	readonly openPaths: readonly string[];
	readonly shortcutPlatform: KeyboardShortcutPlatform;
	/** A new count when the person asks to search again. */
	readonly refreshCount: number;
	readonly onOpenMatch: (target: SearchMatchTarget, how: OpenMatchHow) => void;
	readonly onReveal: (path: string) => void;
	/** Hands the keyboard back to the terminal. */
	readonly onReturnFocus: () => void;
}

/** How long typing pauses before it searches. */
const TYPING_PAUSE_MS = 300;

/** The draft fields a change to which searches at once, as a click on an option does. */
const IMMEDIATE: readonly (keyof SearchDraft)[] = [
	'matchCase',
	'onlyChanged',
	'onlyOpen',
	'regex',
	'useIgnoreFiles',
	'wholeWord',
];

const NOTES = {
	cancelled: workbenchMessages.rightBarSearchCancelled,
	'changed-unavailable': workbenchMessages.rightBarSearchChangedUnavailable,
	'invalid-glob': workbenchMessages.rightBarSearchInvalidGlob,
	'invalid-pattern': workbenchMessages.rightBarSearchInvalidPattern,
	'limit-reached': workbenchMessages.rightBarSearchLimitReached,
	'too-long': workbenchMessages.rightBarSearchTooLong,
	unavailable: workbenchMessages.rightBarSearchUnavailable,
} as const satisfies Partial<
	Record<SearchOutcome, (typeof workbenchMessages)[keyof typeof workbenchMessages]>
>;

function withGlob(list: string, glob: string): string {
	return list.trim().length === 0 ? glob : `${list.trim()}, ${glob}`;
}

/**
 * The Search view: the boxes, a line saying what the search found, and the files and lines it
 * found. Typing searches once it pauses, Enter and the options at once, and Refresh again. F4 and
 * Shift+F4 open the next and previous match without leaving the view.
 */
export function SearchView({
	source,
	workspaceId,
	searches,
	openPaths,
	shortcutPlatform,
	refreshCount,
	onOpenMatch,
	onReveal,
	onReturnFocus,
}: SearchViewProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	React.useSyncExternalStore(searches.subscribe, searches.getVersion);
	const session = searches.session(workspaceId);
	const { draft, results, collapsed, active } = session;
	const key = searchKey(draft);
	const patternReference = React.useRef<HTMLTextAreaElement | null>(null);
	const listReference = React.useRef<HTMLDivElement | null>(null);
	const rows = React.useMemo(() => searchRows(results, collapsed), [collapsed, results]);

	/** Searches for the draft now; `again` searches even when that draft was searched last. */
	const run = useMemoizedFn((again: boolean) => {
		const current = searches.session(workspaceId);
		const currentKey = searchKey(current.draft);
		if (current.draft.pattern.length === 0) {
			if (current.searchedKey !== currentKey || current.results !== null) {
				searches.discard(workspaceId, currentKey);
			}
			return;
		}
		if (!again && currentKey === current.searchedKey) return;
		searches.start(
			workspaceId,
			currentKey,
			prepareSearch(source, workspaceId, current.draft, openPaths),
		);
	});

	// Typing searches once it pauses; an emptied box forgets the results at once.
	const searchedKey = session.searchedKey;
	const empty = draft.pattern.length === 0;
	React.useEffect(() => {
		if (key === searchedKey) return undefined;
		if (empty) {
			run(false);
			return undefined;
		}
		const timer = window.setTimeout(() => run(false), TYPING_PAUSE_MS);
		return () => window.clearTimeout(timer);
	}, [empty, key, run, searchedKey]);
	const refreshed = React.useRef(refreshCount);
	React.useEffect(() => {
		if (refreshed.current === refreshCount) return;
		refreshed.current = refreshCount;
		run(true);
	}, [refreshCount, run]);
	React.useEffect(() => {
		patternReference.current?.focus();
	}, []);

	/** Changes the draft; an option, or a file pattern from the results, searches at once. */
	const edit = (change: Partial<SearchDraft>, now = false): void => {
		searches.edit(workspaceId, change);
		if (now || IMMEDIATE.some((field) => field in change)) run(false);
	};
	const openMatch = (
		path: string,
		line: SearchResultLine,
		match: number,
		how: OpenMatchHow,
	): void => {
		searches.setActive(workspaceId, { key: line.key, match, path });
		onOpenMatch({ path, range: matchRange(line, match) }, how);
	};
	const stepMatch = (direction: 1 | -1): void => {
		const next = searches.step(workspaceId, direction);
		if (next !== null) {
			onOpenMatch(
				{ path: next.path, range: matchRange(next.line, next.match) },
				{ focus: false, preview: true },
			);
		}
	};
	const toggleFile = (path: string): void => {
		const next = new Set(collapsed);
		if (!next.delete(path)) next.add(path);
		searches.setCollapsed(workspaceId, next);
	};
	const focusResults = (): boolean => {
		const first = rows[0];
		if (first === undefined) return false;
		if (active === null)
			searches.setActive(workspaceId, { key: null, match: 0, path: first.file.path });
		listReference.current?.focus();
		return true;
	};
	const clear = (): void => {
		searches.clear(workspaceId, searchKey({ ...draft, pattern: '' }));
		patternReference.current?.focus();
	};

	const files = results?.files ?? [];
	const searching = results?.state === 'searching';
	const allCollapsed = files.length > 0 && files.every((file) => collapsed.has(file.path));
	let summary: string | null = null;
	if (files.length > 0) {
		summary = intl.formatMessage(workbenchMessages.rightBarSearchSummary, {
			files: files.length,
			matches: results?.matches ?? 0,
		});
	} else if (searching) {
		summary = intl.formatMessage(workbenchMessages.rightBarSearchSearching);
	} else if (results?.state === 'complete') {
		summary = intl.formatMessage(workbenchMessages.rightBarSearchNoResults);
	}
	const noteMessage =
		results === null || results.state === 'searching' || results.state === 'complete'
			? null
			: NOTES[results.state];

	/** One of the summary line's actions. */
	const action = (
		label: string,
		icon: React.ComponentProps<typeof ProductIcon>['name'],
		onClick: () => void,
		disabled = false,
	): React.ReactNode => (
		<Button
			aria-label={label}
			className={styles['action']}
			disabled={disabled}
			onClick={onClick}
			size='icon-xs'
			title={label}
			variant='ghost'
		>
			<ProductIcon name={icon} />
		</Button>
	);
	const moreLabel = intl.formatMessage(workbenchMessages.rightBarSearchMore);
	return (
		<div
			className={styles['root']}
			onKeyDown={(event) => {
				if (event.key !== 'F4' || event.altKey || event.ctrlKey || event.metaKey) return;
				event.preventDefault();
				stepMatch(event.shiftKey ? -1 : 1);
			}}
		>
			<SearchForm
				draft={draft}
				history={session.history}
				onChange={(change) => edit(change)}
				onEscape={onReturnFocus}
				onFocusResults={focusResults}
				onRun={() => run(true)}
				patternReference={patternReference}
				shortcutPlatform={shortcutPlatform}
			/>
			<div className={styles['summary']} role='status'>
				<span className={styles['summaryText']}>{summary}</span>
				{searching
					? action(intl.formatMessage(workbenchMessages.rightBarSearchStop), 'search-stop', () =>
							searches.cancel(workspaceId),
						)
					: null}
				{action(
					intl.formatMessage(workbenchMessages.rightBarSearchClear),
					'clear-all',
					clear,
					results === null && empty,
				)}
				{allCollapsed
					? action(
							intl.formatMessage(workbenchMessages.rightBarSearchExpandAll),
							'expand-all',
							() => searches.setCollapsed(workspaceId, new Set()),
						)
					: action(
							intl.formatMessage(workbenchMessages.rightBarSearchCollapseAll),
							'collapse-all',
							() => searches.setCollapsed(workspaceId, new Set(files.map((file) => file.path))),
							files.length === 0,
						)}
				<DropdownMenu>
					<DropdownMenuTrigger
						render={
							<Button
								aria-label={moreLabel}
								className={styles['action']}
								size='icon-xs'
								title={moreLabel}
								variant='ghost'
							/>
						}
					>
						<ProductIcon name='ellipsis' />
					</DropdownMenuTrigger>
					<DropdownMenuContent align='end'>
						<DropdownMenuItem
							disabled={session.history.length === 0}
							onClick={() => searches.clearHistory(workspaceId)}
						>
							<ProductIcon name='trash' size='menu-leading' />
							{intl.formatMessage(workbenchMessages.rightBarSearchClearHistory)}
						</DropdownMenuItem>
					</DropdownMenuContent>
				</DropdownMenu>
			</div>
			{noteMessage === null ? null : (
				<p className={styles['note']}>{intl.formatMessage(noteMessage)}</p>
			)}
			<SearchResultList
				active={active}
				files={files}
				listReference={listReference}
				onActiveChange={(next) => searches.setActive(workspaceId, next)}
				onDismiss={(path, lineKey) => searches.dismiss(workspaceId, path, lineKey)}
				onEscape={() => patternReference.current?.focus()}
				onExclude={(glob) =>
					edit({ details: true, excludes: withGlob(draft.excludes, glob) }, true)
				}
				onInclude={(glob) =>
					edit({ details: true, includes: withGlob(draft.includes, glob) }, true)
				}
				onOpen={openMatch}
				onReveal={onReveal}
				onToggleFile={toggleFile}
				rows={rows}
				shortcutPlatform={shortcutPlatform}
			/>
		</div>
	);
}
