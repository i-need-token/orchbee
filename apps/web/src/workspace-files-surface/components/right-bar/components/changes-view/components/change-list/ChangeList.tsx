import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import { Button, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import {
	canDiscard,
	type ChangeGroup,
	type ChangeItem,
	opensFile,
	splitPath,
} from '../../../../../../change-groups';
import type { ChangeRow, Changes } from '../../../../changeRows';
import { useWindowedRows } from '../../../../hooks/useWindowedRows';

import styles from './ChangeList.module.css';

/** What a group header offers for every change in its group. */
export type GroupAction = 'stage' | 'unstage' | 'discard';

interface ChangeListProps {
	readonly changes: Changes;
	readonly selectedKey: string | null;
	/** While Git changes the repository, every action waits for it. */
	readonly busy: boolean;
	readonly onOpen: (item: ChangeItem, preview: boolean) => void;
	readonly onOpenFile: (item: ChangeItem) => void;
	/** Stages an unstaged change or a conflict, or unstages a staged one. */
	readonly onToggle: (item: ChangeItem) => void;
	readonly onDiscard: (items: readonly ChangeItem[]) => void;
	readonly onGroupAction: (group: ChangeGroup, action: GroupAction) => void;
}

const GROUP_MESSAGES = {
	conflicts: workbenchMessages.rightBarConflicts,
	staged: workbenchMessages.rightBarStaged,
	unstaged: workbenchMessages.rightBarUnstaged,
} as const;

/** The actions each group's header offers, in the order they show. */
const GROUP_ACTIONS = {
	conflicts: [
		{ action: 'stage', icon: 'add', message: workbenchMessages.rightBarGitStageConflicts },
	],
	staged: [{ action: 'unstage', icon: 'remove', message: workbenchMessages.rightBarGitUnstageAll }],
	unstaged: [
		{ action: 'discard', icon: 'restore', message: workbenchMessages.rightBarGitDiscardAll },
		{ action: 'stage', icon: 'add', message: workbenchMessages.rightBarGitStageAll },
	],
} as const satisfies Readonly<
	Record<
		ChangeGroup,
		readonly {
			readonly action: GroupAction;
			readonly icon: React.ComponentProps<typeof ProductIcon>['name'];
			readonly message: (typeof workbenchMessages)[keyof typeof workbenchMessages];
		}[]
	>
>;

/**
 * The Changes view's list: conflicts, staged and unstaged changes under their headers. A click
 * previews a change's diff and a double click keeps it. Arrow keys move between changes, Enter
 * opens the diff and Space stages or unstages, as the design asks; the row's own actions show on
 * hover and on the change the keyboard is on.
 */
export function ChangeList({
	changes,
	selectedKey,
	busy,
	onOpen,
	onOpenFile,
	onToggle,
	onDiscard,
	onGroupAction,
}: ChangeListProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const identity = React.useId();
	const rows: readonly ChangeRow[] = changes.state === 'loaded' ? changes.rows : [];
	const truncated = changes.state === 'loaded' && changes.truncated;
	const windowed = useWindowedRows(rows.length + (truncated ? 1 : 0));
	// The change the keyboard is on, by key; when it leaves the list, its place is kept.
	const [active, setActive] = React.useState<{ readonly key: string; readonly index: number }>({
		index: 0,
		key: '',
	});
	const activeIndex = activeIndexOf(rows, active);

	const moveTo = (index: number): void => {
		const row = rows[index];
		if (row?.kind !== 'item') return;
		setActive({ index, key: row.item.key });
		windowed.revealRow(index);
	};
	const step = (from: number, direction: 1 | -1): void => {
		for (let index = from + direction; index >= 0 && index < rows.length; index += direction) {
			if (rows[index]?.kind === 'item') {
				moveTo(index);
				return;
			}
		}
	};
	const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
		if (event.target !== event.currentTarget) return;
		const row = activeIndex === null ? undefined : rows[activeIndex];
		const item = row?.kind === 'item' ? row.item : null;
		let handled = true;
		switch (event.key) {
			case 'ArrowDown':
				step(activeIndex ?? -1, 1);
				break;
			case 'ArrowUp':
				step(activeIndex ?? rows.length, -1);
				break;
			case 'Home':
				step(-1, 1);
				break;
			case 'End':
				step(rows.length, -1);
				break;
			case 'Enter':
				if (item !== null) onOpen(item, true);
				break;
			case ' ':
				if (item !== null && !busy) onToggle(item);
				break;
			default:
				handled = false;
		}
		if (handled) event.preventDefault();
	};

	let note: string | null = null;
	if (changes.state === 'loading') note = intl.formatMessage(workbenchMessages.rightBarLoading);
	else if (changes.state === 'failed') {
		note = intl.formatMessage(
			changes.reason === 'not-repository'
				? workbenchMessages.rightBarNotRepository
				: workbenchMessages.rightBarFailed,
		);
	} else if (rows.length === 0) note = intl.formatMessage(workbenchMessages.rightBarNoChanges);

	/** One of a row's or header's actions; the click acts on it, never on the row under it. */
	const action = (
		label: string,
		icon: React.ComponentProps<typeof ProductIcon>['name'],
		onClick: () => void,
	): React.ReactNode => (
		<Button
			aria-label={label}
			className={styles['action']}
			disabled={busy}
			key={label}
			onClick={(event) => {
				event.stopPropagation();
				onClick();
			}}
			onDoubleClick={(event) => event.stopPropagation()}
			size='icon-xs'
			tabIndex={-1}
			title={label}
			variant='ghost'
		>
			<ProductIcon name={icon} />
		</Button>
	);

	const drawn: React.ReactNode[] = [];
	for (let index = windowed.first; index < windowed.last; index += 1) {
		const offset = `${index * windowed.rowHeight}px`;
		const measure = index === windowed.first ? windowed.measureRow : undefined;
		const row = rows[index];
		if (row === undefined) {
			if (index === rows.length && truncated) {
				drawn.push(
					<div
						className={styles['note']}
						key='truncated'
						ref={measure}
						style={
							{
								'--ob-comp-workbench-right-bar-row-translate-y': offset,
							} satisfies React.CSSProperties
						}
					>
						{intl.formatMessage(workbenchMessages.rightBarTruncated, {
							count: rows.filter((candidate) => candidate.kind === 'item').length,
						})}
					</div>,
				);
			}
			continue;
		}
		if (row.kind === 'header') {
			const { group, items } = row;
			drawn.push(
				<div
					className={styles['groupHeader']}
					key={`group:${group}`}
					ref={measure}
					role='presentation'
					style={
						{
							'--ob-comp-workbench-right-bar-row-translate-y': offset,
						} satisfies React.CSSProperties
					}
				>
					<span className={styles['groupName']}>{intl.formatMessage(GROUP_MESSAGES[group])}</span>
					<span className={styles['actions']}>
						{GROUP_ACTIONS[group]
							.filter(({ action: kind }) => kind !== 'discard' || items.some(canDiscard))
							.map(({ action: kind, icon, message }) =>
								action(intl.formatMessage(message), icon, () => onGroupAction(group, kind)),
							)}
					</span>
					<span className={styles['count']}>{items.length}</span>
				</div>,
			);
			continue;
		}
		const { item } = row;
		const { name, folder } = splitPath(item.path);
		drawn.push(
			<div
				aria-selected={item.key === selectedKey}
				className={styles['changeRow']}
				data-active={index === activeIndex || undefined}
				id={`${identity}-${index}`}
				key={item.key}
				onClick={() => {
					setActive({ index, key: item.key });
					onOpen(item, true);
				}}
				onDoubleClick={() => onOpen(item, false)}
				ref={measure}
				role='option'
				style={
					{ '--ob-comp-workbench-right-bar-row-translate-y': offset } satisfies React.CSSProperties
				}
				title={item.originalPath === null ? item.path : `${item.originalPath} → ${item.path}`}
			>
				<span className={styles['entryIcon']}>
					<ProductIcon name='file' />
				</span>
				<span className={styles['entryName']}>{name}</span>
				<span className={styles['folder']}>{folder}</span>
				<span className={styles['actions']}>
					{opensFile(item)
						? action(intl.formatMessage(workbenchMessages.rightBarGitOpenFile), 'go-to-file', () =>
								onOpenFile(item),
							)
						: null}
					{canDiscard(item)
						? action(intl.formatMessage(workbenchMessages.rightBarGitDiscard), 'restore', () =>
								onDiscard([item]),
							)
						: null}
					{item.group === 'staged'
						? action(intl.formatMessage(workbenchMessages.rightBarGitUnstage), 'remove', () =>
								onToggle(item),
							)
						: action(intl.formatMessage(workbenchMessages.rightBarGitStage), 'add', () =>
								onToggle(item),
							)}
				</span>
				<span className={styles['letter']} data-letter={item.letter}>
					{item.letter}
				</span>
			</div>,
		);
	}
	return (
		<div
			aria-activedescendant={
				activeIndex === null || note !== null ? undefined : `${identity}-${activeIndex}`
			}
			aria-label={intl.formatMessage(workbenchMessages.rightBarChanges)}
			className={styles['viewport']}
			onKeyDown={onKeyDown}
			onScroll={windowed.onScroll}
			ref={windowed.viewportReference}
			role='listbox'
			tabIndex={0}
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
				<p className={styles['empty']}>{note}</p>
			)}
		</div>
	);
}

/**
 * Where the keyboard is: the row of the remembered change, or, once that change has moved or gone
 * (staging moves it to another group), the change now nearest its old place.
 */
function activeIndexOf(
	rows: readonly ChangeRow[],
	active: { readonly key: string; readonly index: number },
): number | null {
	const kept = rows.findIndex((row) => row.kind === 'item' && row.item.key === active.key);
	if (kept >= 0) return kept;
	for (let distance = 0; distance < rows.length; distance += 1) {
		for (const index of [active.index + distance, active.index - distance]) {
			if (rows[index]?.kind === 'item') return index;
		}
	}
	return null;
}
