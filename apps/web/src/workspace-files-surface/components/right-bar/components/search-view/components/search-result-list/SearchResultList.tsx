import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import {
	type KeyboardShortcutChord,
	type KeyboardShortcutPlatform,
	matchesKeyboardShortcutChord,
} from '@orchbee/keyboard-shortcuts';
import {
	Button,
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuTrigger,
	ProductIcon,
} from '@orchbee/ui/components';
import * as React from 'react';

import { splitPath } from '../../../../../../change-groups';
import {
	previewParts,
	type ReplaceTarget,
	type SearchActive,
	type SearchResultFile,
	type SearchResultLine,
	type SearchRow,
	searchLineText,
	searchResultsText,
} from '../../../../../../search-results';
import { useWindowedRows } from '../../../../hooks/useWindowedRows';

import styles from './SearchResultList.module.css';

/** How a match opens: as a preview the next one replaces, and whether the editor takes the keyboard. */
export interface OpenMatchHow {
	readonly preview: boolean;
	readonly focus: boolean;
}

export interface SearchResultListProps {
	readonly files: readonly SearchResultFile[];
	readonly rows: readonly SearchRow[];
	readonly active: SearchActive | null;
	readonly shortcutPlatform: KeyboardShortcutPlatform;
	readonly listReference: React.RefObject<HTMLDivElement | null>;
	readonly onActiveChange: (active: SearchActive) => void;
	readonly onToggleFile: (path: string) => void;
	readonly onOpen: (path: string, line: SearchResultLine, match: number, how: OpenMatchHow) => void;
	/** Removes a file, or one of its lines, from the results. */
	readonly onDismiss: (path: string, key: string | null) => void;
	/** Adds a glob to the files to exclude or include, and searches again. */
	readonly onExclude: (glob: string) => void;
	readonly onInclude: (glob: string) => void;
	readonly onReveal: (path: string) => void;
	/** Hands the keyboard back to the search box. */
	readonly onEscape: () => void;
	/** Whether the replace box shows, which offers replacing on each row and marks what goes. */
	readonly replacing: boolean;
	/** Whether a row's matches can be replaced now. */
	readonly canReplace: boolean;
	/** What a match becomes, where that is known without the file. */
	readonly replacementOf: ((matched: string) => string) | null;
	readonly onReplace: (target: ReplaceTarget) => void;
}

/** Cmd+Backspace on a Mac dismisses, as Delete does elsewhere. */
const DISMISS_SHORTCUT = {
	code: 'Backspace',
	modifiers: ['mod'],
} as const satisfies KeyboardShortcutChord;
/** Replaces the matches of the row the keyboard is on, as an editor's search results do. */
const REPLACE_SHORTCUT = {
	code: 'Digit1',
	modifiers: ['mod', 'shift'],
} as const satisfies KeyboardShortcutChord;

/** Where the keyboard is when it is on `row`, keeping the match F4 reached on its line. */
function activeOf(row: SearchRow, active: SearchActive | null): SearchActive {
	if (row.kind === 'file') return { key: null, match: 0, path: row.file.path };
	const match = active?.key === row.line.key ? active.match : 0;
	return { key: row.line.key, match, path: row.file.path };
}

function isActive(row: SearchRow, active: SearchActive | null): boolean {
	if (active === null || row.file.path !== active.path) return false;
	return row.kind === 'file' ? active.key === null : row.line.key === active.key;
}

/** The rows a dismissal removes: a file with its lines, or a line, and its file when it is the last. */
function dismissedSpan(rows: readonly SearchRow[], index: number): { start: number; end: number } {
	const row = rows[index];
	if (row === undefined) return { end: index, start: index };
	const path = row.file.path;
	const whole = row.kind === 'file' || row.file.lines.length === 1;
	if (!whole) return { end: index + 1, start: index };
	let start = index;
	while (start > 0 && rows[start - 1]?.file.path === path) start -= 1;
	let end = index + 1;
	while (end < rows.length && rows[end]?.file.path === path) end += 1;
	return { end, start };
}

/**
 * The files and lines a search found, as a tree: a file's header with its count, then each line with
 * its matches marked. A click previews a match and keeps the keyboard here; Enter opens it in the
 * editor and a double click keeps its tab. Arrow keys move through the rows, ← and → fold and
 * unfold a file, Delete removes a row, Mod+Shift+1 replaces its matches while replacing, and Esc
 * goes back to the search box.
 */
export function SearchResultList({
	files,
	rows,
	active,
	shortcutPlatform,
	listReference,
	onActiveChange,
	onToggleFile,
	onOpen,
	onDismiss,
	onExclude,
	onInclude,
	onReveal,
	onEscape,
	replacing,
	canReplace,
	replacementOf,
	onReplace,
}: SearchResultListProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const identity = React.useId();
	const windowed = useWindowedRows(rows.length);
	const activeIndex = rows.findIndex((row) => isActive(row, active));
	const canCopy = window.isSecureContext;

	// A match the window moved to, by F4 or from the editor, comes into view.
	const revealed = React.useRef(active);
	const { revealRow } = windowed;
	React.useEffect(() => {
		if (revealed.current === active) return;
		revealed.current = active;
		if (activeIndex >= 0) revealRow(activeIndex);
	}, [active, activeIndex, revealRow]);

	const moveTo = (index: number): void => {
		const row = rows[index];
		if (row === undefined) return;
		const next = activeOf(row, active);
		revealed.current = next;
		onActiveChange(next);
		revealRow(index);
	};
	const openRow = (row: SearchRow, how: OpenMatchHow): void => {
		if (row.kind === 'file') onToggleFile(row.file.path);
		else onOpen(row.file.path, row.line, activeOf(row, active).match, how);
	};
	/** Takes a row out of the results, by dismissing it or replacing its matches. */
	const remove = (index: number, how: 'dismiss' | 'replace'): void => {
		const row = rows[index];
		if (row === undefined) return;
		// The keyboard moves to the row after what goes, or before it at the end.
		const span = dismissedSpan(rows, index);
		const next = rows[span.end] ?? rows[span.start - 1];
		if (next !== undefined && index === activeIndex) {
			const nextActive = activeOf(next, null);
			revealed.current = nextActive;
			onActiveChange(nextActive);
		}
		const key = row.kind === 'file' ? null : row.line.key;
		if (how === 'dismiss') onDismiss(row.file.path, key);
		else onReplace({ key, path: row.file.path });
	};
	const dismiss = (index: number): void => remove(index, 'dismiss');
	const replace = (index: number): void => {
		if (replacing && canReplace) remove(index, 'replace');
	};

	const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
		if (event.target !== event.currentTarget) return;
		const row = activeIndex < 0 ? undefined : rows[activeIndex];
		if (
			row !== undefined &&
			((shortcutPlatform === 'apple' &&
				matchesKeyboardShortcutChord(event.nativeEvent, DISMISS_SHORTCUT, shortcutPlatform)) ||
				(event.key === 'Delete' && !event.altKey && !event.ctrlKey && !event.metaKey))
		) {
			event.preventDefault();
			dismiss(activeIndex);
			return;
		}
		if (
			row !== undefined &&
			replacing &&
			matchesKeyboardShortcutChord(event.nativeEvent, REPLACE_SHORTCUT, shortcutPlatform)
		) {
			event.preventDefault();
			replace(activeIndex);
			return;
		}
		if (event.altKey || event.ctrlKey || event.metaKey) return;
		let handled = true;
		switch (event.key) {
			case 'ArrowDown':
				moveTo(Math.min(rows.length - 1, activeIndex + 1));
				break;
			case 'ArrowUp':
				moveTo(activeIndex < 0 ? rows.length - 1 : Math.max(0, activeIndex - 1));
				break;
			case 'Home':
				moveTo(0);
				break;
			case 'End':
				moveTo(rows.length - 1);
				break;
			case 'ArrowRight':
				if (row?.kind === 'file') {
					if (row.collapsed) onToggleFile(row.file.path);
					else moveTo(activeIndex + 1);
				}
				break;
			case 'ArrowLeft':
				if (row?.kind === 'file' && !row.collapsed) onToggleFile(row.file.path);
				else if (row?.kind === 'line') {
					moveTo(
						rows.findIndex((candidate) => candidate.kind === 'file' && candidate.file === row.file),
					);
				}
				break;
			case 'Enter':
				if (row !== undefined) openRow(row, { focus: true, preview: true });
				break;
			case ' ':
				if (row !== undefined) openRow(row, { focus: false, preview: true });
				break;
			case 'Escape':
				onEscape();
				break;
			default:
				handled = false;
		}
		if (handled) {
			event.preventDefault();
			event.stopPropagation();
		}
	};

	const copy = (text: string): void => {
		void navigator.clipboard.writeText(text).catch(() => undefined);
	};
	/** A file's matches are all replaced, a line's are replaced together. */
	const replaceLabel = (row: SearchRow): string =>
		intl.formatMessage(
			row.kind === 'file'
				? workbenchMessages.rightBarSearchReplaceAll
				: workbenchMessages.rightBarSearchReplace,
		);
	const menuItem = (
		label: string,
		icon: React.ComponentProps<typeof ProductIcon>['name'],
		onClick: () => void,
	): React.ReactNode => (
		<ContextMenuItem key={label} onClick={onClick}>
			<ProductIcon name={icon} size='menu-leading' />
			{label}
		</ContextMenuItem>
	);
	/** The frequent actions on a file or line; opening the menu leaves the keyboard where it was. */
	const menu = (row: SearchRow, index: number): React.ReactNode => {
		const { name, folder } = splitPath(row.file.path);
		const dot = name.lastIndexOf('.');
		const type = dot > 0 ? `*${name.slice(dot)}` : null;
		const filters: React.ReactNode[] = [];
		if (folder.length > 0) {
			filters.push(
				menuItem(intl.formatMessage(workbenchMessages.rightBarSearchExcludeFolder), 'exclude', () =>
					onExclude(`./${folder}/`),
				),
			);
		}
		if (type !== null) {
			filters.push(
				menuItem(
					intl.formatMessage(workbenchMessages.rightBarSearchExcludeType, { pattern: type }),
					'exclude',
					() => onExclude(type),
				),
				menuItem(
					intl.formatMessage(workbenchMessages.rightBarSearchIncludeType, { pattern: type }),
					'filter',
					() => onInclude(type),
				),
			);
		}
		return (
			<ContextMenuContent finalFocus={listReference}>
				{replacing ? (
					<ContextMenuItem disabled={!canReplace} onClick={() => replace(index)}>
						<ProductIcon
							name={row.kind === 'file' ? 'replace-all' : 'replace'}
							size='menu-leading'
						/>
						{replaceLabel(row)}
					</ContextMenuItem>
				) : null}
				{menuItem(intl.formatMessage(workbenchMessages.rightBarSearchDismiss), 'close', () =>
					dismiss(index),
				)}
				{filters.length > 0 ? <ContextMenuSeparator /> : null}
				{filters}
				{canCopy ? (
					<>
						<ContextMenuSeparator />
						{menuItem(intl.formatMessage(workbenchMessages.rightBarSearchCopy), 'copy', () =>
							copy(row.kind === 'file' ? searchResultsText([row.file]) : searchLineText(row.line)),
						)}
						{menuItem(
							intl.formatMessage(workbenchMessages.rightBarSearchCopyPath),
							'clipboard',
							() => copy(row.file.path),
						)}
						{menuItem(
							intl.formatMessage(workbenchMessages.rightBarSearchCopyAll),
							'select-all',
							() => copy(searchResultsText(files)),
						)}
					</>
				) : null}
				<ContextMenuSeparator />
				{menuItem(intl.formatMessage(workbenchMessages.rightBarSearchReveal), 'folder-opened', () =>
					onReveal(row.file.path),
				)}
			</ContextMenuContent>
		);
	};

	const replaceButton = (row: SearchRow, index: number): React.ReactNode =>
		replacing ? (
			<Button
				aria-label={replaceLabel(row)}
				className={styles['action']}
				disabled={!canReplace}
				onClick={(event) => {
					event.stopPropagation();
					replace(index);
				}}
				onDoubleClick={(event) => event.stopPropagation()}
				size='icon-xs'
				tabIndex={-1}
				title={replaceLabel(row)}
				variant='ghost'
			>
				<ProductIcon name={row.kind === 'file' ? 'replace-all' : 'replace'} />
			</Button>
		) : null;
	const dismissLabel = intl.formatMessage(workbenchMessages.rightBarSearchDismiss);
	const dismissButton = (index: number): React.ReactNode => (
		<Button
			aria-label={dismissLabel}
			className={styles['action']}
			onClick={(event) => {
				event.stopPropagation();
				dismiss(index);
			}}
			onDoubleClick={(event) => event.stopPropagation()}
			size='icon-xs'
			tabIndex={-1}
			title={dismissLabel}
			variant='ghost'
		>
			<ProductIcon name='close' />
		</Button>
	);

	const drawn: React.ReactNode[] = [];
	for (let index = windowed.first; index < windowed.last; index += 1) {
		const row = rows[index];
		if (row === undefined) continue;
		let content: React.ReactNode;
		if (row.kind === 'file') {
			const { name, folder } = splitPath(row.file.path);
			content = (
				<>
					<span className={styles['disclosure']}>
						<ProductIcon name={row.collapsed ? 'chevron-right' : 'chevron-down'} />
					</span>
					<span className={styles['entryIcon']}>
						<ProductIcon name='file' />
					</span>
					<span className={styles['entryName']}>{name}</span>
					<span className={styles['folder']}>{folder}</span>
					<span className={styles['actions']}>
						{replaceButton(row, index)}
						{dismissButton(index)}
					</span>
					<span className={styles['count']}>{row.file.matches}</span>
				</>
			);
		} else {
			content = (
				<>
					<span className={styles['preview']}>
						{previewParts(row.line, replacing ? replacementOf : null).map((part, at) => {
							if (!part.match) return <React.Fragment key={at}>{part.text}</React.Fragment>;
							if (!replacing) {
								return (
									<mark className={styles['match']} key={at}>
										{part.text}
									</mark>
								);
							}
							// What goes is struck through, and what comes in follows it where it is known.
							return (
								<React.Fragment key={at}>
									<del className={styles['removed']}>{part.text}</del>
									{part.replacement === undefined || part.replacement.length === 0 ? null : (
										<ins className={styles['inserted']}>
											{part.replacement.replaceAll('\n', '↵')}
										</ins>
									)}
								</React.Fragment>
							);
						})}
					</span>
					<span className={styles['actions']}>
						{replaceButton(row, index)}
						{dismissButton(index)}
					</span>
				</>
			);
		}
		const element = (
			<div
				aria-expanded={row.kind === 'file' ? !row.collapsed : undefined}
				aria-level={row.kind === 'file' ? 1 : 2}
				aria-selected={index === activeIndex}
				className={styles[row.kind === 'file' ? 'fileRow' : 'lineRow']}
				data-active={index === activeIndex || undefined}
				id={`${identity}-${index}`}
				onClick={() => {
					moveTo(index);
					openRow(row, { focus: false, preview: true });
				}}
				onDoubleClick={
					row.kind === 'line'
						? () =>
								onOpen(row.file.path, row.line, activeOf(row, active).match, {
									focus: true,
									preview: false,
								})
						: undefined
				}
				ref={index === windowed.first ? windowed.measureRow : undefined}
				role='treeitem'
				style={
					{
						'--ob-comp-workbench-right-bar-row-translate-y': `${index * windowed.rowHeight}px`,
					} satisfies React.CSSProperties
				}
				title={
					row.kind === 'file'
						? row.file.path
						: intl.formatMessage(workbenchMessages.rightBarSearchLineTitle, { line: row.line.line })
				}
			/>
		);
		drawn.push(
			<ContextMenu key={row.kind === 'file' ? `file:${row.file.path}` : row.line.key}>
				<ContextMenuTrigger render={element}>{content}</ContextMenuTrigger>
				{menu(row, index)}
			</ContextMenu>,
		);
	}
	return (
		<div
			aria-activedescendant={activeIndex < 0 ? undefined : `${identity}-${activeIndex}`}
			aria-label={intl.formatMessage(workbenchMessages.rightBarSearchResults)}
			className={styles['viewport']}
			onKeyDown={onKeyDown}
			onScroll={windowed.onScroll}
			ref={(element) => {
				windowed.viewportReference.current = element;
				listReference.current = element;
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
