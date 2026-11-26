import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import {
	formatKeyboardShortcutChord,
	matchesKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';
import {
	Button,
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
	ProductIcon,
} from '@orchbee/ui/components';
import * as React from 'react';

import styles from './CommitBox.module.css';

/** Commits from the message box, as an editor's source control view does. */
const COMMIT_SHORTCUT = {
	code: 'Enter',
	modifiers: ['mod'],
} as const satisfies KeyboardShortcutChord;

/** How a commit was asked for. */
export interface CommitRequest {
	readonly amend: boolean;
	readonly stageAll: boolean;
}

/** What went wrong with the last change to the repository, shown under the box. */
export interface CommitBoxReport {
	readonly heading: string;
	/** Git's own words, or ours when Git said nothing. */
	readonly text: string;
	/** Whether `text` is Git's output, which shows as it printed it. */
	readonly output: boolean;
	readonly truncated: boolean;
}

interface CommitBoxProps {
	readonly message: string;
	readonly onMessageChange: (message: string) => void;
	/** Messages committed before, oldest first, which ↑ and ↓ bring back. */
	readonly history: readonly string[];
	/** Whether there is anything a commit could take, staged or not. */
	readonly hasChanges: boolean;
	/** Whether the repository could be read, so a commit or an amend may be tried. */
	readonly ready: boolean;
	/** While Git changes the repository, nothing else is sent. */
	readonly busy: boolean;
	readonly committing: boolean;
	readonly report: CommitBoxReport | null;
	readonly shortcutPlatform: KeyboardShortcutPlatform;
	readonly onCommit: (request: CommitRequest) => void;
	readonly onDismissReport: () => void;
}

/**
 * The Changes view's commit box: a message of one to ten lines, Commit, and a menu with "Stage all
 * and commit" and "Amend last commit". Mod+Enter commits, and ↑ on the first line or ↓ on the last
 * walks through the messages committed before. What went wrong shows under it until dismissed or
 * the next change.
 */
export function CommitBox({
	message,
	onMessageChange,
	history,
	hasChanges,
	ready,
	busy,
	committing,
	report,
	shortcutPlatform,
	onCommit,
	onDismissReport,
}: CommitBoxProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	// Where ↑ and ↓ are in the history, and the message typed before walking into it.
	const [walk, setWalk] = React.useState<{ readonly index: number; readonly draft: string } | null>(
		null,
	);
	const canCommit = ready && !busy && hasChanges && message.trim().length > 0;
	const canAmend = ready && !busy;

	const commit = useMemoizedFn((request: CommitRequest) => {
		if (request.amend ? !canAmend : !canCommit) return;
		setWalk(null);
		onCommit(request);
	});
	const recall = (index: number, draft: string): void => {
		setWalk(index === history.length ? null : { draft, index });
		onMessageChange(index === history.length ? draft : (history[index] ?? draft));
	};
	const onKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>): void => {
		if (matchesKeyboardShortcutChord(event.nativeEvent, COMMIT_SHORTCUT, shortcutPlatform)) {
			event.preventDefault();
			commit({ amend: false, stageAll: false });
			return;
		}
		if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || history.length === 0) {
			return;
		}
		const box = event.currentTarget;
		if (box.selectionStart !== box.selectionEnd) return;
		const before = box.value.slice(0, box.selectionStart);
		const after = box.value.slice(box.selectionEnd);
		if (event.key === 'ArrowUp' && !before.includes('\n')) {
			const index = walk === null ? history.length - 1 : walk.index - 1;
			if (index < 0) return;
			event.preventDefault();
			recall(index, walk?.draft ?? message);
		} else if (event.key === 'ArrowDown' && walk !== null && !after.includes('\n')) {
			event.preventDefault();
			recall(walk.index + 1, walk.draft);
		}
	};

	const shortcut = formatKeyboardShortcutChord(COMMIT_SHORTCUT, shortcutPlatform);
	const commitLabel = intl.formatMessage(
		committing ? workbenchMessages.rightBarGitCommitting : workbenchMessages.rightBarGitCommit,
	);
	const moreLabel = intl.formatMessage(workbenchMessages.rightBarGitCommitMore);
	return (
		<div className={styles['root']}>
			<textarea
				aria-label={intl.formatMessage(workbenchMessages.rightBarGitMessage)}
				className={styles['message']}
				disabled={!ready}
				onChange={(event) => {
					// A message typed over a recalled one is the new draft.
					setWalk(null);
					onMessageChange(event.target.value);
				}}
				onKeyDown={onKeyDown}
				placeholder={intl.formatMessage(workbenchMessages.rightBarGitMessagePlaceholder, {
					shortcut,
				})}
				rows={1}
				spellCheck
				value={message}
			/>
			<div className={styles['buttons']}>
				<Button
					className={styles['commit']}
					disabled={!canCommit}
					onClick={() => commit({ amend: false, stageAll: false })}
					size='sm'
					title={`${commitLabel} (${shortcut})`}
				>
					<ProductIcon name='check' />
					{commitLabel}
				</Button>
				<DropdownMenu>
					<DropdownMenuTrigger
						render={
							<Button
								aria-label={moreLabel}
								className={styles['more']}
								disabled={!canAmend}
								size='icon-sm'
								title={moreLabel}
							/>
						}
					>
						<ProductIcon name='chevron-down' />
					</DropdownMenuTrigger>
					<DropdownMenuContent align='end'>
						<DropdownMenuItem
							disabled={!canCommit}
							onClick={() => commit({ amend: false, stageAll: true })}
						>
							<ProductIcon name='add' />
							{intl.formatMessage(workbenchMessages.rightBarGitStageAllAndCommit)}
						</DropdownMenuItem>
						<DropdownMenuItem
							disabled={!canAmend}
							onClick={() => commit({ amend: true, stageAll: false })}
						>
							<ProductIcon name='edit' />
							{intl.formatMessage(workbenchMessages.rightBarGitAmend)}
						</DropdownMenuItem>
					</DropdownMenuContent>
				</DropdownMenu>
			</div>
			{report === null ? null : (
				<div className={styles['report']} role='alert'>
					<div className={styles['reportHeader']}>
						<span className={styles['reportHeading']}>{report.heading}</span>
						<Button
							aria-label={intl.formatMessage(workbenchMessages.rightBarGitOutputDismiss)}
							className={styles['dismiss']}
							onClick={onDismissReport}
							size='icon-xs'
							title={intl.formatMessage(workbenchMessages.rightBarGitOutputDismiss)}
							variant='ghost'
						>
							<ProductIcon name='close' />
						</Button>
					</div>
					{report.output ? (
						<pre className={styles['output']}>{report.text}</pre>
					) : (
						<p className={styles['reason']}>{report.text}</p>
					)}
					{report.truncated ? (
						<p className={styles['reason']}>
							{intl.formatMessage(workbenchMessages.rightBarGitOutputTruncated)}
						</p>
					) : null}
				</div>
			)}
		</div>
	);
}
