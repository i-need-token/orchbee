import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type { WorkspaceFilesWatchState } from '@orchbee/workbench-client-protocol';
import { Button, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchWorkspaceFilesSource } from '../../../workbench';
import type { FloatTarget } from '../../float-tabs';
import type { SearchMatchTarget } from '../../search-results';
import type { SearchSessions } from '../../SearchSessions';
import { changeRows, type Changes } from './changeRows';
import { ChangesView } from './components/changes-view/ChangesView';
import type { OpenMatchHow } from './components/search-view/components/search-result-list/SearchResultList';
import { flattenTree, type Listing, parentPath, type TreeRow } from './file-tree';
import { useWindowedRows } from './hooks/useWindowedRows';
import type { WorkspaceFilesChange } from '../../hooks/useWorkspaceFilesWatch';
import { readWorkspaceFiles } from '../../readWorkspaceFiles';

import styles from './RightBar.module.css';

const LazySearchView = React.lazy(() =>
	import('./components/search-view/SearchView').then((module) => ({
		default: module.SearchView,
	})),
);

export interface RightBarProps {
	readonly source: WorkbenchWorkspaceFilesSource;
	/** The Workspace whose files show; null shows a note. */
	readonly workspaceId: string | null;
	readonly onClose: () => void;
	readonly closeShortcutLabel?: string | undefined;
	/** What the float window shows, which the tree and the change list mark. */
	readonly activeTarget: FloatTarget | null;
	/** Opens a file or change in the float window; a preview gives way to the next one. */
	readonly onOpen: (target: FloatTarget, preview: boolean) => void;
	/** Tells the float window that the user asked to see the files again. */
	readonly onRefresh: () => void;
	/** A new count when the files likely changed; the shown view reads again if it shows the change. */
	readonly autoRefresh: AutoRefresh;
	/** Whether changes are followed; null while nothing is watched yet. */
	readonly watchState: WorkspaceFilesWatchState | null;
	/** The open folders, kept for the Workspace while the bar is closed. */
	readonly expanded: ReadonlySet<string>;
	readonly onExpandedChange: (expanded: ReadonlySet<string>) => void;
	/** Whether the note that changes are not followed was dismissed for this Workspace. */
	readonly noteDismissed: boolean;
	readonly onDismissNote: () => void;
	/** The Workspace's commit message, kept while the bar is closed. */
	readonly commitMessage: string;
	readonly onCommitMessageChange: (message: string) => void;
	/** Messages the Workspace committed in this window, oldest first. */
	readonly commitHistory: readonly string[];
	readonly onCommitted: (workspaceId: string, message: string) => void;
	readonly shortcutPlatform: KeyboardShortcutPlatform;
	/** Every Workspace's search, which outlives the bar. */
	readonly searches: SearchSessions;
	/** The files the float window has open, which a search can keep to. */
	readonly openPaths: readonly string[];
	/** Opens a match in the float window and puts the editor's selection on it. */
	readonly onOpenMatch: (target: SearchMatchTarget, how: OpenMatchHow) => void;
	/** Hands the keyboard back to the terminal. */
	readonly onReturnFocus: () => void;
}

/** One automatic refresh: its count, and what may have changed. */
export interface AutoRefresh extends WorkspaceFilesChange {
	readonly count: number;
}

type View = 'files' | 'search' | 'changes';

/**
 * The right bar: the Workspace's files as a tree, a search of their text, and its Git changes, read
 * on demand, with the staging and committing of those changes. The tree and the change list read
 * again on Refresh, on switching to them, and when a change they show arrives; the search runs again
 * only on Refresh. While changes are not followed, coming back to the window reads again too, and a
 * note says Refresh is needed; a change list cut short is not followed either. Nothing polls.
 */
export function RightBar({
	source,
	workspaceId,
	onClose,
	closeShortcutLabel,
	activeTarget,
	autoRefresh,
	watchState,
	expanded,
	onExpandedChange,
	noteDismissed,
	onDismissNote,
	onOpen,
	onRefresh,
	commitMessage,
	onCommitMessageChange,
	commitHistory,
	onCommitted,
	shortcutPlatform,
	searches,
	openPaths,
	onOpenMatch,
	onReturnFocus,
}: RightBarProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const [view, setView] = React.useState<View>('files');
	const [listings, setListings] = React.useState<ReadonlyMap<string, Listing>>(() => new Map());
	const [changes, setChanges] = React.useState<Changes>({ state: 'loading' });
	// The newest read for each folder and for the change list; an older answer is dropped.
	const reads = React.useRef(new Map<string, number>());
	const nextRead = React.useRef(0);
	const treeReference = React.useRef<HTMLDivElement | null>(null);
	// Refresh in the Search view searches again; nothing else does.
	const [searchRefreshCount, setSearchRefreshCount] = React.useState(0);
	// The file the tree shows and moves the keyboard to, as Reveal in File Tree asks.
	const [treeReveal, setTreeReveal] = React.useState<TreeReveal | null>(null);

	const begin = (key: string): (() => boolean) => {
		nextRead.current += 1;
		const token = nextRead.current;
		reads.current.set(key, token);
		return () => reads.current.get(key) === token;
	};
	const loadFolder = useMemoizedFn((folder: string) => {
		if (workspaceId === null) return;
		const current = begin(`folder:${folder}`);
		setListings((previous) => {
			if (previous.get(folder)?.state === 'loaded') return previous;
			return new Map(previous).set(folder, { state: 'loading' });
		});
		void readWorkspaceFiles(source, workspaceId, { kind: 'list-directory', path: folder }).then(
			(outcome) => {
				if (!current()) return;
				const listing: Listing =
					outcome.kind === 'directory'
						? { entries: outcome.entries, state: 'loaded', truncated: outcome.truncated }
						: { reason: outcome.kind === 'failed' ? outcome.reason : 'malformed', state: 'failed' };
				setListings((previous) => new Map(previous).set(folder, listing));
			},
		);
	});
	const loadChanges = useMemoizedFn(() => {
		if (workspaceId === null) return;
		const current = begin('changes');
		void readWorkspaceFiles(source, workspaceId, { kind: 'git-status' }).then((outcome) => {
			if (!current()) return;
			setChanges(
				outcome.kind === 'git-status'
					? changeRows(outcome)
					: { reason: outcome.kind === 'failed' ? outcome.reason : 'malformed', state: 'failed' },
			);
		});
	});
	const changesCut = changes.state === 'loaded' && changes.truncated;
	const reload = useMemoizedFn(() => {
		if (view === 'files') {
			loadFolder('');
			for (const folder of expanded) loadFolder(folder);
		} else if (view === 'changes') {
			loadChanges();
		}
	});
	/** Reads again what shows, and the float window's file, as coming back to the window does. */
	const refreshShown = useMemoizedFn(() => {
		reload();
		onRefresh();
	});
	const refresh = useMemoizedFn(() => {
		if (view === 'search') setSearchRefreshCount((count) => count + 1);
		refreshShown();
	});
	React.useEffect(() => {
		reload();
	}, [reload, view, workspaceId]);
	const autoRefreshed = React.useRef(autoRefresh.count);
	React.useEffect(() => {
		if (autoRefreshed.current === autoRefresh.count) return;
		autoRefreshed.current = autoRefresh.count;
		if (
			view === 'files'
				? autoRefresh.files
				: view === 'changes' && autoRefresh.repository && !changesCut
		) {
			reload();
		}
	}, [autoRefresh, changesCut, reload, view]);
	const followed = watchState === 'watching' || watchState === 'starting';
	React.useEffect(() => {
		if (followed) return;
		window.addEventListener('focus', refreshShown);
		return () => window.removeEventListener('focus', refreshShown);
	}, [followed, refreshShown]);

	const toggleFolder = useMemoizedFn((path: string) => {
		const next = new Set(expanded);
		if (next.has(path)) {
			next.delete(path);
		} else {
			next.add(path);
			// An opened folder is read again, so it never shows what it held when last closed.
			loadFolder(path);
		}
		onExpandedChange(next);
	});

	/** Shows a file in the tree, opening the folders above it. */
	const revealInTree = useMemoizedFn((path: string) => {
		const next = new Set(expanded);
		for (let folder = parentPath(path); folder.length > 0; folder = parentPath(folder)) {
			next.add(folder);
		}
		onExpandedChange(next);
		setTreeReveal((previous) => ({ path, serial: (previous?.serial ?? 0) + 1 }));
		setView('files');
	});

	const rows = React.useMemo(() => flattenTree(listings, expanded), [expanded, listings]);

	let body: React.ReactNode;
	if (workspaceId === null) {
		body = (
			<p className={styles['note']}>{intl.formatMessage(workbenchMessages.rightBarNoWorkspace)}</p>
		);
	} else if (view === 'files') {
		body = (
			<FileTree
				expanded={expanded}
				onOpenFile={(path, preview) => onOpen({ kind: 'file', path }, preview)}
				onToggleFolder={toggleFolder}
				reveal={treeReveal}
				rows={rows}
				selectedPath={activeTarget?.kind === 'file' ? activeTarget.path : null}
				treeReference={treeReference}
			/>
		);
	} else if (view === 'search') {
		body = (
			<React.Suspense
				fallback={
					<p className={styles['note']}>{intl.formatMessage(workbenchMessages.rightBarLoading)}</p>
				}
			>
				<LazySearchView
					onOpenMatch={onOpenMatch}
					onReturnFocus={onReturnFocus}
					onReveal={revealInTree}
					openPaths={openPaths}
					refreshCount={searchRefreshCount}
					searches={searches}
					shortcutPlatform={shortcutPlatform}
					source={source}
					workspaceId={workspaceId}
				/>
			</React.Suspense>
		);
	} else {
		body = (
			<ChangesView
				changes={changes}
				history={commitHistory}
				message={commitMessage}
				onChanged={refresh}
				onCommitted={onCommitted}
				onMessageChange={onCommitMessageChange}
				onOpen={onOpen}
				selectedKey={activeTarget?.kind === 'diff' ? activeTarget.item.key : null}
				shortcutPlatform={shortcutPlatform}
				source={source}
				workspaceId={workspaceId}
			/>
		);
	}

	const tab = (value: View, label: string): React.ReactNode => (
		<button
			aria-selected={view === value}
			className={styles['viewTab']}
			onClick={() => setView(value)}
			role='tab'
			type='button'
		>
			{label}
		</button>
	);
	const closeLabel = intl.formatMessage(workbenchMessages.rightBarClose);
	return (
		<aside
			aria-label={intl.formatMessage(workbenchMessages.rightBarLabel)}
			className={styles['root']}
			data-slot='workbench-right-bar'
		>
			<div className={styles['header']} data-slot='workbench-right-bar-header'>
				<div className={styles['viewTabs']} role='tablist'>
					{tab('files', intl.formatMessage(workbenchMessages.rightBarFiles))}
					{tab('search', intl.formatMessage(workbenchMessages.rightBarSearch))}
					{tab('changes', intl.formatMessage(workbenchMessages.rightBarChanges))}
				</div>
				<Button
					aria-label={intl.formatMessage(workbenchMessages.rightBarRefresh)}
					className={styles['headerButton']}
					disabled={workspaceId === null}
					onClick={refresh}
					size='icon-xs'
					title={intl.formatMessage(workbenchMessages.rightBarRefresh)}
					variant='ghost'
				>
					<ProductIcon name='refresh' />
				</Button>
				<Button
					aria-label={closeLabel}
					className={styles['headerButton']}
					onClick={onClose}
					size='icon-xs'
					title={
						closeShortcutLabel === undefined ? closeLabel : `${closeLabel} (${closeShortcutLabel})`
					}
					variant='ghost'
				>
					<ProductIcon name='close' />
				</Button>
				<div className={styles['windowControls']} data-slot='workbench-right-bar-trailing' />
			</div>
			{workspaceId !== null &&
			!noteDismissed &&
			(watchState === 'unavailable' || (view === 'changes' && changesCut)) ? (
				<div className={styles['autoRefreshNote']} role='status'>
					<p className={styles['autoRefreshNoteText']}>
						{intl.formatMessage(workbenchMessages.rightBarAutoRefreshOff)}
					</p>
					<Button
						aria-label={intl.formatMessage(workbenchMessages.rightBarAutoRefreshOffDismiss)}
						className={styles['headerButton']}
						onClick={onDismissNote}
						size='icon-xs'
						title={intl.formatMessage(workbenchMessages.rightBarAutoRefreshOffDismiss)}
						variant='ghost'
					>
						<ProductIcon name='close' />
					</Button>
				</div>
			) : null}
			<div className={styles['body']} role='tabpanel'>
				{body}
			</div>
		</aside>
	);
}

/** A file the tree is asked to show; each ask has its own serial. */
interface TreeReveal {
	readonly path: string;
	readonly serial: number;
}

interface FileTreeProps {
	readonly rows: readonly TreeRow[];
	readonly reveal: TreeReveal | null;
	readonly expanded: ReadonlySet<string>;
	readonly selectedPath: string | null;
	readonly onToggleFolder: (path: string) => void;
	readonly onOpenFile: (path: string, preview: boolean) => void;
	readonly treeReference: React.RefObject<HTMLDivElement | null>;
}

function FileTree({
	rows,
	reveal,
	expanded,
	selectedPath,
	onToggleFolder,
	onOpenFile,
	treeReference,
}: FileTreeProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const identity = React.useId();
	const [activePath, setActivePath] = React.useState<string | null>(null);
	const windowed = useWindowedRows(rows.length);
	const activeIndex = Math.max(
		0,
		rows.findIndex((row) => row.kind === 'entry' && row.path === activePath),
	);
	// A revealed file is shown once the folders above it are read, and the keyboard moves to it.
	const revealed = React.useRef(0);
	const { revealRow } = windowed;
	React.useEffect(() => {
		if (reveal === null || reveal.serial === revealed.current) return;
		const index = rows.findIndex((row) => row.kind === 'entry' && row.path === reveal.path);
		// Once nothing is being read, a file still missing is gone, and the ask with it.
		if (index < 0 && rows.some((row) => row.kind === 'note' && row.note === 'loading')) return;
		revealed.current = reveal.serial;
		if (index < 0) return;
		setActivePath(reveal.path);
		revealRow(index);
		treeReference.current?.focus();
	}, [reveal, revealRow, rows, treeReference]);
	const activate = (row: TreeRow, preview: boolean): void => {
		if (row.kind !== 'entry') return;
		if (row.entry.entryKind === 'directory') onToggleFolder(row.path);
		else if (row.entry.entryKind === 'file') onOpenFile(row.path, preview);
	};
	const moveTo = (index: number): void => {
		const row = rows[index];
		if (row?.kind !== 'entry') return;
		setActivePath(row.path);
		windowed.revealRow(index);
	};
	const step = (from: number, direction: 1 | -1): void => {
		for (let index = from + direction; index >= 0 && index < rows.length; index += direction) {
			if (rows[index]?.kind === 'entry') {
				moveTo(index);
				return;
			}
		}
	};
	const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
		const row = rows[activeIndex];
		let handled = true;
		switch (event.key) {
			case 'ArrowDown':
				step(activeIndex, 1);
				break;
			case 'ArrowUp':
				step(activeIndex, -1);
				break;
			case 'Home':
				step(-1, 1);
				break;
			case 'End':
				step(rows.length, -1);
				break;
			case 'ArrowRight':
				if (row?.kind === 'entry' && row.entry.entryKind === 'directory') {
					if (expanded.has(row.path)) step(activeIndex, 1);
					else onToggleFolder(row.path);
				}
				break;
			case 'ArrowLeft':
				if (
					row?.kind === 'entry' &&
					row.entry.entryKind === 'directory' &&
					expanded.has(row.path)
				) {
					onToggleFolder(row.path);
				} else if (row?.kind === 'entry' && row.depth > 0) {
					const parent = parentPath(row.path);
					moveTo(
						rows.findIndex((candidate) => candidate.kind === 'entry' && candidate.path === parent),
					);
				}
				break;
			case 'Enter':
			case ' ':
				if (row !== undefined) activate(row, true);
				break;
			default:
				handled = false;
		}
		if (handled) event.preventDefault();
	};
	const drawn: React.ReactNode[] = [];
	for (let index = windowed.first; index < windowed.last; index += 1) {
		const row = rows[index];
		if (row === undefined) continue;
		const offset = `${index * windowed.rowHeight}px`;
		const measure = index === windowed.first ? windowed.measureRow : undefined;
		if (row.kind === 'note') {
			const text =
				row.note === 'truncated'
					? intl.formatMessage(workbenchMessages.rightBarTruncated, { count: row.count })
					: intl.formatMessage(
							{
								empty: workbenchMessages.rightBarEmpty,
								failed: workbenchMessages.rightBarFailed,
								loading: workbenchMessages.rightBarLoading,
							}[row.note],
						);
			drawn.push(
				<div
					className={styles['treeNote']}
					key={row.key}
					ref={measure}
					role='none'
					style={
						{
							'--ob-comp-workbench-right-bar-row-depth': row.depth,
							'--ob-comp-workbench-right-bar-row-translate-y': offset,
						} satisfies React.CSSProperties
					}
				>
					{text}
				</div>,
			);
			continue;
		}
		const directory = row.entry.entryKind === 'directory';
		drawn.push(
			<div
				aria-expanded={directory ? row.expanded : undefined}
				aria-level={row.depth + 1}
				aria-selected={row.path === selectedPath}
				className={styles['treeRow']}
				data-active={index === activeIndex || undefined}
				data-ignored={row.entry.ignored || undefined}
				id={`${identity}-${index}`}
				key={row.key}
				onClick={() => {
					setActivePath(row.path);
					activate(row, true);
				}}
				onDoubleClick={() => {
					if (row.entry.entryKind === 'file') onOpenFile(row.path, false);
				}}
				ref={measure}
				role='treeitem'
				style={
					{
						'--ob-comp-workbench-right-bar-row-depth': row.depth,
						'--ob-comp-workbench-right-bar-row-translate-y': offset,
					} satisfies React.CSSProperties
				}
			>
				<span className={styles['disclosure']}>
					{directory ? (
						<ProductIcon name={row.expanded ? 'chevron-down' : 'chevron-right'} />
					) : null}
				</span>
				<span className={styles['entryIcon']}>
					<ProductIcon name={directory ? (row.expanded ? 'folder-opened' : 'folder') : 'file'} />
				</span>
				<span className={styles['entryName']}>{row.entry.name}</span>
				{row.entry.symbolicLink ? <span className={styles['linkMark']}>↗</span> : null}
			</div>,
		);
	}
	return (
		<div
			aria-activedescendant={rows.length === 0 ? undefined : `${identity}-${activeIndex}`}
			aria-label={intl.formatMessage(workbenchMessages.rightBarFiles)}
			className={styles['viewport']}
			onKeyDown={onKeyDown}
			onScroll={windowed.onScroll}
			ref={(element) => {
				windowed.viewportReference.current = element;
				treeReference.current = element;
			}}
			role='tree'
			tabIndex={0}
		>
			<div
				className={styles['rows']}
				style={
					{
						'--ob-comp-workbench-right-bar-list-block-size': `${windowed.totalHeight}px`,
					} satisfies React.CSSProperties
				}
			>
				{drawn}
			</div>
		</div>
	);
}
