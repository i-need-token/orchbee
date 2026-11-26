import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { WorkspaceFilesOutcome } from '@orchbee/workbench-client-protocol';
import { Button, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchWorkspaceFilesSource } from '../../../workbench';
import { type ChangeGroup, type ChangeItem, groupChanges, splitPath } from '../../change-groups';
import { flattenTree, type Listing, parentPath, type TreeRow } from './file-tree';
import { useWindowedRows } from './hooks/useWindowedRows';
import type { FloatTarget } from '../../float-tabs';
import { readWorkspaceFiles } from '../../readWorkspaceFiles';

import styles from './RightBar.module.css';

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
	/** Tells the float window that the files may have changed. */
	readonly onRefresh: () => void;
}

type View = 'files' | 'changes';
type Failure = Extract<WorkspaceFilesOutcome, { kind: 'failed' }>['reason'];
type Changes =
	| { readonly state: 'loading' }
	| { readonly state: 'failed'; readonly reason: Failure }
	| {
			readonly state: 'loaded';
			readonly branch: string | null;
			readonly items: readonly ChangeRow[];
			readonly empty: boolean;
			readonly truncated: boolean;
	  };
type ChangeRow =
	| { readonly kind: 'header'; readonly group: ChangeGroup; readonly count: number }
	| { readonly kind: 'item'; readonly item: ChangeItem };

const GROUP_MESSAGES = {
	conflicts: workbenchMessages.rightBarConflicts,
	staged: workbenchMessages.rightBarStaged,
	unstaged: workbenchMessages.rightBarUnstaged,
} as const;

function changeRows(outcome: Extract<WorkspaceFilesOutcome, { kind: 'git-status' }>): Changes {
	const groups = groupChanges(outcome.entries);
	const items: ChangeRow[] = [];
	for (const group of ['conflicts', 'staged', 'unstaged'] as const) {
		const members = groups[group];
		if (members.length === 0) continue;
		items.push({ count: members.length, group, kind: 'header' });
		for (const item of members) items.push({ item, kind: 'item' });
	}
	return {
		branch: outcome.branch,
		empty: items.length === 0,
		items,
		state: 'loaded',
		truncated: outcome.truncated,
	};
}

/**
 * The right bar: the Workspace's files as a tree and its Git changes, both read on demand and read
 * again on Refresh, on switching views and when the window comes back to the front. Nothing polls.
 */
export function RightBar({
	source,
	workspaceId,
	onClose,
	closeShortcutLabel,
	activeTarget,
	onOpen,
	onRefresh,
}: RightBarProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const [view, setView] = React.useState<View>('files');
	const [listings, setListings] = React.useState<ReadonlyMap<string, Listing>>(() => new Map());
	const [expanded, setExpanded] = React.useState<ReadonlySet<string>>(() => new Set());
	const [changes, setChanges] = React.useState<Changes>({ state: 'loading' });
	// The newest read for each folder and for the change list; an older answer is dropped.
	const reads = React.useRef(new Map<string, number>());
	const nextRead = React.useRef(0);
	const treeReference = React.useRef<HTMLDivElement | null>(null);

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
	const refresh = useMemoizedFn(() => {
		if (view === 'files') {
			loadFolder('');
			for (const folder of expanded) loadFolder(folder);
		} else {
			loadChanges();
		}
		onRefresh();
	});
	React.useEffect(() => {
		refresh();
	}, [refresh, view, workspaceId]);
	React.useEffect(() => {
		window.addEventListener('focus', refresh);
		return () => window.removeEventListener('focus', refresh);
	}, [refresh]);

	const toggleFolder = useMemoizedFn((path: string) => {
		const next = new Set(expanded);
		if (next.has(path)) {
			next.delete(path);
		} else {
			next.add(path);
			// An opened folder is read again, so it never shows what it held when last closed.
			loadFolder(path);
		}
		setExpanded(next);
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
				rows={rows}
				selectedPath={activeTarget?.kind === 'file' ? activeTarget.path : null}
				treeReference={treeReference}
			/>
		);
	} else {
		body = (
			<ChangeList
				changes={changes}
				onOpen={(item, preview) => onOpen({ item, kind: 'diff' }, preview)}
				selectedKey={activeTarget?.kind === 'diff' ? activeTarget.item.key : null}
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
			{view === 'changes' && changes.state === 'loaded' && changes.branch !== null ? (
				<div className={styles['branch']}>
					<ProductIcon name='git-branch' />
					<span className={styles['branchName']}>{changes.branch}</span>
				</div>
			) : null}
			<div className={styles['body']} role='tabpanel'>
				{body}
			</div>
		</aside>
	);
}

interface FileTreeProps {
	readonly rows: readonly TreeRow[];
	readonly expanded: ReadonlySet<string>;
	readonly selectedPath: string | null;
	readonly onToggleFolder: (path: string) => void;
	readonly onOpenFile: (path: string, preview: boolean) => void;
	readonly treeReference: React.RefObject<HTMLDivElement | null>;
}

function FileTree({
	rows,
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

interface ChangeListProps {
	readonly changes: Changes;
	readonly selectedKey: string | null;
	readonly onOpen: (item: ChangeItem, preview: boolean) => void;
}

function ChangeList({ changes, selectedKey, onOpen }: ChangeListProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const items = changes.state === 'loaded' ? changes.items : [];
	const windowed = useWindowedRows(
		items.length + (changes.state === 'loaded' && changes.truncated ? 1 : 0),
	);
	let note: string | null = null;
	if (changes.state === 'loading') note = intl.formatMessage(workbenchMessages.rightBarLoading);
	else if (changes.state === 'failed') {
		note = intl.formatMessage(
			changes.reason === 'not-repository'
				? workbenchMessages.rightBarNotRepository
				: workbenchMessages.rightBarFailed,
		);
	} else if (changes.empty) note = intl.formatMessage(workbenchMessages.rightBarNoChanges);
	const drawn: React.ReactNode[] = [];
	for (let index = windowed.first; index < windowed.last; index += 1) {
		const placement = `${index * windowed.rowHeight}px`;
		const measure = index === windowed.first ? windowed.measureRow : undefined;
		const row = items[index];
		if (row === undefined) {
			if (index === items.length && changes.state === 'loaded' && changes.truncated) {
				drawn.push(
					<div
						className={styles['treeNote']}
						key='truncated'
						ref={measure}
						style={
							{
								'--ob-comp-workbench-right-bar-row-translate-y': placement,
							} satisfies React.CSSProperties
						}
					>
						{intl.formatMessage(workbenchMessages.rightBarTruncated, {
							count: items.filter((candidate) => candidate.kind === 'item').length,
						})}
					</div>,
				);
			}
			continue;
		}
		if (row.kind === 'header') {
			drawn.push(
				<div
					className={styles['groupHeader']}
					key={`group:${row.group}`}
					ref={measure}
					style={
						{
							'--ob-comp-workbench-right-bar-row-translate-y': placement,
						} satisfies React.CSSProperties
					}
				>
					<span className={styles['entryName']}>
						{intl.formatMessage(GROUP_MESSAGES[row.group])}
					</span>
					<span className={styles['count']}>{row.count}</span>
				</div>,
			);
			continue;
		}
		const { item } = row;
		const { name, folder } = splitPath(item.path);
		drawn.push(
			<button
				aria-pressed={item.key === selectedKey}
				className={styles['changeRow']}
				key={item.key}
				onClick={() => onOpen(item, true)}
				onDoubleClick={() => onOpen(item, false)}
				ref={measure}
				style={
					{
						'--ob-comp-workbench-right-bar-row-translate-y': placement,
					} satisfies React.CSSProperties
				}
				title={item.originalPath === null ? item.path : `${item.originalPath} → ${item.path}`}
				type='button'
			>
				<span className={styles['entryIcon']}>
					<ProductIcon name='file' />
				</span>
				<span className={styles['entryName']}>{name}</span>
				<span className={styles['folder']}>{folder}</span>
				<span className={styles['letter']} data-letter={item.letter}>
					{item.letter}
				</span>
			</button>,
		);
	}
	return (
		<div
			aria-label={intl.formatMessage(workbenchMessages.rightBarChanges)}
			className={styles['viewport']}
			onScroll={windowed.onScroll}
			ref={windowed.viewportReference}
		>
			{note === null ? (
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
			) : (
				<p className={styles['note']}>{note}</p>
			)}
		</div>
	);
}
