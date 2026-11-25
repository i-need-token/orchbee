import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { WorkspaceFilesOutcome } from '@orchbee/workbench-client-protocol';
import { Button, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchWorkspaceFilesSource } from '../../../workbench';
import type { ChangeItem } from '../../change-groups';
import { splitPath } from '../../change-groups';
import { useWindowedRows } from '../../hooks';
import { type DiffRow, diffLines, presentDiff, splitLines } from '../../line-diff';
import { readWorkspaceFiles } from '../../readWorkspaceFiles';

import styles from './FilePreview.module.css';

/** What the preview shows: a working-tree file, or a change's two sides compared. */
export type FilePreviewTarget =
	| { readonly kind: 'file'; readonly path: string }
	| { readonly kind: 'diff'; readonly item: ChangeItem };

interface FilePreviewProps {
	readonly source: WorkbenchWorkspaceFilesSource;
	readonly workspaceId: string;
	readonly target: FilePreviewTarget;
	/** Changes when the right bar refreshes, which reads the file again. */
	readonly refreshCount: number;
	readonly onClose: () => void;
}

type Body =
	| { readonly kind: 'loading' }
	| { readonly kind: 'note'; readonly note: 'binary' | 'too-large' | 'absent' | 'failed' }
	| { readonly kind: 'rows'; readonly rows: readonly DiffRow[] };

const NO_TEXT: WorkspaceFilesOutcome = { byteLength: 0, content: { kind: 'absent' }, kind: 'file' };

/** A failed read, or a file that cannot show as text, as the note the preview shows instead. */
function noteFor(outcome: WorkspaceFilesOutcome): Body | null {
	if (outcome.kind !== 'file') return { kind: 'note', note: 'failed' };
	if (outcome.content.kind === 'binary') return { kind: 'note', note: 'binary' };
	if (outcome.content.kind === 'too-large') return { kind: 'note', note: 'too-large' };
	return null;
}

function textOf(outcome: WorkspaceFilesOutcome): string {
	return outcome.kind === 'file' && outcome.content.kind === 'text' ? outcome.content.text : '';
}

async function readBody(
	source: WorkbenchWorkspaceFilesSource,
	workspaceId: string,
	target: FilePreviewTarget,
): Promise<Body> {
	if (target.kind === 'file') {
		const outcome = await readWorkspaceFiles(source, workspaceId, {
			kind: 'read-file',
			path: target.path,
			source: 'worktree',
		});
		const note = noteFor(outcome);
		if (note !== null) return note;
		if (outcome.kind === 'file' && outcome.content.kind === 'absent') {
			return { kind: 'note', note: 'absent' };
		}
		return {
			kind: 'rows',
			rows: splitLines(textOf(outcome)).map((text, index) => ({
				kind: 'same',
				newNumber: index + 1,
				oldNumber: index + 1,
				text,
			})),
		};
	}
	const { item } = target;
	const [before, after] = await Promise.all([
		item.before === null
			? Promise.resolve(NO_TEXT)
			: readWorkspaceFiles(source, workspaceId, {
					kind: 'read-file',
					path: item.beforePath,
					source: item.before,
				}),
		readWorkspaceFiles(source, workspaceId, {
			kind: 'read-file',
			path: item.path,
			source: item.after,
		}),
	]);
	const note = noteFor(after) ?? noteFor(before);
	if (note !== null) return note;
	return {
		kind: 'rows',
		rows: presentDiff(diffLines(splitLines(textOf(before)), splitLines(textOf(after)))),
	};
}

/**
 * The read-only preview that floats over the terminals: a file with line numbers, or a change as a
 * unified diff with its unchanged runs folded. Esc or the close button puts it away.
 */
export function FilePreview({
	source,
	workspaceId,
	target,
	refreshCount,
	onClose,
}: FilePreviewProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const [body, setBody] = React.useState<Body>({ kind: 'loading' });
	const rootReference = React.useRef<HTMLDivElement | null>(null);
	React.useEffect(() => {
		let current = true;
		void readBody(source, workspaceId, target).then((next) => {
			if (current) setBody(next);
		});
		return () => {
			current = false;
		};
	}, [refreshCount, source, target, workspaceId]);
	React.useEffect(() => {
		rootReference.current?.focus({ preventScroll: true });
	}, [target]);
	const rows = body.kind === 'rows' ? body.rows : [];
	const windowed = useWindowedRows(rows.length);
	const onKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLDivElement>) => {
		if (event.key === 'Escape' && !event.defaultPrevented) {
			event.preventDefault();
			event.stopPropagation();
			onClose();
		}
	});
	const path = target.kind === 'file' ? target.path : target.item.path;
	const { name, folder } = splitPath(path);
	const title =
		target.kind === 'file'
			? name
			: intl.formatMessage(workbenchMessages.rightBarPreviewDiff, { name });
	const diff = target.kind === 'diff';
	let content: React.ReactNode;
	if (body.kind === 'loading') {
		content = (
			<p className={styles['note']}>{intl.formatMessage(workbenchMessages.rightBarLoading)}</p>
		);
	} else if (body.kind === 'note') {
		const message = {
			absent: workbenchMessages.rightBarPreviewAbsent,
			binary: workbenchMessages.rightBarPreviewBinary,
			failed: workbenchMessages.rightBarFailed,
			'too-large': workbenchMessages.rightBarPreviewTooLarge,
		}[body.note];
		content = <p className={styles['note']}>{intl.formatMessage(message)}</p>;
	} else {
		const drawn: React.ReactNode[] = [];
		for (let index = windowed.first; index < windowed.last; index += 1) {
			const row = rows[index];
			if (row === undefined) continue;
			const top = index * windowed.rowHeight;
			if (row.kind === 'gap') {
				drawn.push(
					<div
						className={styles['line']}
						data-kind='gap'
						key={index}
						ref={index === windowed.first ? windowed.measureRow : undefined}
						style={
							{
								'--ob-comp-workbench-right-bar-row-translate-y': `${top}px`,
							} satisfies React.CSSProperties
						}
					>
						<span className={styles['number']} />
						{diff ? <span className={styles['number']} /> : null}
						<span className={styles['text']}>⋯</span>
					</div>,
				);
				continue;
			}
			drawn.push(
				<div
					className={styles['line']}
					data-kind={diff ? row.kind : undefined}
					key={index}
					ref={index === windowed.first ? windowed.measureRow : undefined}
					style={
						{
							'--ob-comp-workbench-right-bar-row-translate-y': `${top}px`,
						} satisfies React.CSSProperties
					}
				>
					{diff ? (
						<span className={styles['number']}>{row.kind === 'added' ? '' : row.oldNumber}</span>
					) : null}
					<span className={styles['number']}>{row.kind === 'removed' ? '' : row.newNumber}</span>
					<span className={styles['text']}>{row.text}</span>
				</div>,
			);
		}
		content = (
			<div
				className={styles['lines']}
				style={
					{
						'--ob-comp-workbench-right-bar-list-block-size': `${windowed.totalHeight}px`,
					} satisfies React.CSSProperties
				}
			>
				{drawn}
			</div>
		);
	}
	return (
		<div
			aria-label={title}
			className={styles['root']}
			data-slot='workspace-file-preview'
			onKeyDown={onKeyDown}
			ref={rootReference}
			role='dialog'
			tabIndex={-1}
		>
			<div className={styles['header']}>
				<span className={styles['title']}>{title}</span>
				{folder === '' ? null : <span className={styles['folder']}>{folder}</span>}
				<Button
					aria-label={intl.formatMessage(workbenchMessages.rightBarPreviewClose)}
					className={styles['close']}
					onClick={onClose}
					size='icon-xs'
					variant='ghost'
				>
					<ProductIcon name='close' />
				</Button>
			</div>
			<div
				className={styles['viewport']}
				onScroll={windowed.onScroll}
				ref={windowed.viewportReference}
				tabIndex={0}
			>
				{content}
			</div>
		</div>
	);
}
