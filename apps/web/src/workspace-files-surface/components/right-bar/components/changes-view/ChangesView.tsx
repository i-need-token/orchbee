import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import {
	MAXIMUM_WORKSPACE_GIT_MESSAGE_BYTES,
	MAXIMUM_WORKSPACE_GIT_PATHS,
	type WorkspaceFilesOutcome,
} from '@orchbee/workbench-client-protocol';
import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchWorkspaceFilesSource, WorkspaceGitChange } from '../../../../../workbench';
import {
	canDiscard,
	isUntracked,
	type ChangeGroup,
	type ChangeItem,
	splitPath,
	unstagePaths,
} from '../../../../change-groups';
import type { FloatTarget } from '../../../../float-tabs';
import type { Changes } from '../../changeRows';
import { ChangeList, type GroupAction } from './components/change-list/ChangeList';
import {
	type CommitBoxReport,
	CommitBox,
	type CommitRequest,
} from './components/commit-box/CommitBox';
import {
	type ChangeQuestion,
	ConfirmChangeDialog,
} from './components/confirm-change-dialog/ConfirmChangeDialog';

import styles from './ChangesView.module.css';

/** What a change to the repository was doing, which names what went wrong. */
type Doing = 'commit' | 'stage' | 'unstage' | 'discard';

/** A question waiting for the person, and what a yes does. */
interface Pending {
	readonly question: ChangeQuestion;
	readonly proceed: () => void;
}

export interface ChangesViewProps {
	readonly source: WorkbenchWorkspaceFilesSource;
	readonly workspaceId: string;
	readonly changes: Changes;
	readonly selectedKey: string | null;
	readonly onOpen: (target: FloatTarget, preview: boolean) => void;
	/** Reads the repository again, and the float window's view of it, after a change. */
	readonly onChanged: () => void;
	readonly message: string;
	readonly onMessageChange: (message: string) => void;
	readonly history: readonly string[];
	/**
	 * Remembers a message a Workspace committed, for ↑ and ↓ in the box, and empties the box unless
	 * something else was typed meanwhile. It is told even when the view closed while Git ran.
	 */
	readonly onCommitted: (workspaceId: string, message: string) => void;
	readonly shortcutPlatform: KeyboardShortcutPlatform;
}

const FAILED_HEADINGS = {
	commit: workbenchMessages.rightBarGitFailedCommit,
	discard: workbenchMessages.rightBarGitFailedDiscard,
	stage: workbenchMessages.rightBarGitFailedStage,
	unstage: workbenchMessages.rightBarGitFailedUnstage,
} as const;

const FAILURE_REASONS = {
	denied: workbenchMessages.rightBarGitReasonDenied,
	malformed: workbenchMessages.rightBarGitReasonUnconfirmed,
	'not-found': workbenchMessages.rightBarGitReasonNotFound,
	'not-repository': workbenchMessages.rightBarGitReasonNotRepository,
	timeout: workbenchMessages.rightBarGitReasonTimeout,
	'trash-unavailable': workbenchMessages.rightBarGitReasonTrashUnavailable,
	unavailable: workbenchMessages.rightBarGitReasonUnavailable,
} as const;

/** Splits paths into requests the protocol carries. */
function chunks(paths: readonly string[]): readonly (readonly string[])[] {
	const parts: string[][] = [];
	for (let start = 0; start < paths.length; start += MAXIMUM_WORKSPACE_GIT_PATHS) {
		parts.push(paths.slice(start, start + MAXIMUM_WORKSPACE_GIT_PATHS));
	}
	return parts;
}

function byteLength(text: string): number {
	return new TextEncoder().encode(text).length;
}

/**
 * The Changes view: the branch, the commit box and the change list, and the changes to the
 * repository they ask for. One change runs at a time; while it runs the view waits for it, and the
 * list reads the repository again when it ends, whatever its outcome. A view that closes while Git
 * runs reports nothing, but a commit that lands is still remembered.
 */
export function ChangesView({
	source,
	workspaceId,
	changes,
	selectedKey,
	onOpen,
	onChanged,
	message,
	onMessageChange,
	history,
	onCommitted,
	shortcutPlatform,
}: ChangesViewProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const [running, setRunning] = React.useState<Doing | null>(null);
	const [report, setReport] = React.useState<CommitBoxReport | null>(null);
	const [pending, setPending] = React.useState<Pending | null>(null);
	// Set at once, so a second click before the next render sends nothing.
	const busy = React.useRef(false);

	/** Whether an outcome is a success; anything else is reported under the commit box. */
	const settle = (doing: Doing, outcome: WorkspaceFilesOutcome | null): boolean => {
		if (outcome?.kind === 'git-done') return true;
		const heading = intl.formatMessage(FAILED_HEADINGS[doing]);
		if (outcome?.kind === 'git-rejected') {
			setReport({ heading, output: true, text: outcome.output, truncated: outcome.truncated });
		} else {
			const reason =
				outcome?.kind === 'failed'
					? FAILURE_REASONS[outcome.reason]
					: workbenchMessages.rightBarGitReasonUnconfirmed;
			setReport({ heading, output: false, text: intl.formatMessage(reason), truncated: false });
		}
		return false;
	};
	/** Runs changes one after another, stopping at the first that does not succeed. */
	const run = useMemoizedFn(
		async (doing: Doing, steps: readonly WorkspaceGitChange[]): Promise<boolean> => {
			if (busy.current || steps.length === 0) return false;
			busy.current = true;
			setRunning(doing);
			setReport(null);
			let succeeded = true;
			for (const step of steps) {
				const outcome = await source.changeRepository(workspaceId, step);
				if (!settle(doing, outcome)) {
					succeeded = false;
					break;
				}
			}
			busy.current = false;
			setRunning(null);
			onChanged();
			return succeeded;
		},
	);

	const stage = (paths: readonly string[]): void => {
		void run(
			'stage',
			chunks(paths).map((part) => ({ kind: 'git-stage', paths: [...part] })),
		);
	};
	const toggle = useMemoizedFn((item: ChangeItem) => {
		if (item.group === 'staged') {
			void run('unstage', [{ kind: 'git-unstage', paths: [...unstagePaths(item)] }]);
		} else {
			stage([item.path]);
		}
	});
	const discard = useMemoizedFn((items: readonly ChangeItem[]) => {
		const discarded = items.filter(canDiscard);
		const paths = discarded.map((item) => item.path);
		const [first] = paths;
		if (first === undefined) return;
		const untracked = discarded.filter(isUntracked).length;
		// Only files moved to the trash can be had back, so only then is Enter allowed to confirm.
		const onlyTrash = untracked === paths.length;
		let description = intl.formatMessage(workbenchMessages.rightBarGitDiscardDescription);
		if (onlyTrash) {
			description = intl.formatMessage(workbenchMessages.rightBarGitDiscardTrashDescription, {
				count: untracked,
			});
		} else if (untracked > 0) {
			description = intl.formatMessage(workbenchMessages.rightBarGitDiscardMixedDescription, {
				count: untracked,
			});
		}
		let title = intl.formatMessage(workbenchMessages.rightBarGitDiscardTitleMany, {
			count: paths.length,
		});
		if (paths.length === 1) {
			title = intl.formatMessage(
				onlyTrash
					? workbenchMessages.rightBarGitDiscardTrashTitleOne
					: workbenchMessages.rightBarGitDiscardTitleOne,
				{ name: splitPath(first).name },
			);
		} else if (onlyTrash) {
			title = intl.formatMessage(workbenchMessages.rightBarGitDiscardTrashTitleMany, {
				count: paths.length,
			});
		}
		setPending({
			proceed: () => {
				void run(
					'discard',
					chunks(paths).map((part) => ({ kind: 'git-discard', paths: [...part] })),
				);
			},
			question: {
				confirm: intl.formatMessage(
					onlyTrash
						? workbenchMessages.rightBarGitDiscardTrashConfirm
						: workbenchMessages.rightBarGitDiscardConfirm,
				),
				description,
				destructive: !onlyTrash,
				paths,
				title,
			},
		});
	});
	const groupAction = useMemoizedFn((group: ChangeGroup, action: GroupAction) => {
		if (changes.state !== 'loaded') return;
		const items = changes.rows.flatMap((row) =>
			row.kind === 'item' && row.item.group === group ? [row.item] : [],
		);
		if (action === 'discard') discard(items);
		else if (action === 'unstage') void run('unstage', [{ kind: 'git-unstage', paths: null }]);
		// Staging every unstaged change leaves conflicts alone; staging conflicts names each one.
		else if (group === 'unstaged') void run('stage', [{ kind: 'git-stage', paths: null }]);
		else stage(items.map((item) => item.path));
	});

	const loaded = changes.state === 'loaded' ? changes : null;
	const commit = useMemoizedFn((request: CommitRequest) => {
		if (loaded === null) return;
		if (byteLength(message) > MAXIMUM_WORKSPACE_GIT_MESSAGE_BYTES) {
			setReport({
				heading: intl.formatMessage(workbenchMessages.rightBarGitFailedCommit),
				output: false,
				text: intl.formatMessage(workbenchMessages.rightBarGitReasonMessageTooLong),
				truncated: false,
			});
			return;
		}
		const send = (stageAll: boolean): void => {
			const sent = message;
			const committedIn = workspaceId;
			void run('commit', [
				{ amend: request.amend, kind: 'git-commit', message: sent, stageAll },
			]).then((succeeded) => {
				if (succeeded) onCommitted(committedIn, sent);
			});
		};
		// A commit with nothing staged asks before taking everything, as an editor does.
		if (!request.amend && !request.stageAll && loaded.staged === 0 && loaded.unstaged > 0) {
			setPending({
				proceed: () => send(true),
				question: {
					confirm: intl.formatMessage(workbenchMessages.rightBarGitStageAllAndCommit),
					description: intl.formatMessage(workbenchMessages.rightBarGitCommitAllDescription),
					destructive: false,
					paths: [],
					title: intl.formatMessage(workbenchMessages.rightBarGitCommitAllTitle),
				},
			});
			return;
		}
		send(request.stageAll);
	});

	return (
		<>
			{loaded?.branch === null || loaded === null ? null : (
				<div className={styles['branch']}>
					<ProductIcon name='git-branch' />
					<span className={styles['branchName']}>{loaded.branch}</span>
				</div>
			)}
			<CommitBox
				busy={running !== null}
				committing={running === 'commit'}
				hasChanges={loaded !== null && loaded.staged + loaded.unstaged + loaded.conflicts > 0}
				history={history}
				message={message}
				onCommit={commit}
				onDismissReport={() => setReport(null)}
				onMessageChange={onMessageChange}
				ready={loaded !== null}
				report={report}
				shortcutPlatform={shortcutPlatform}
			/>
			<ChangeList
				busy={running !== null}
				changes={changes}
				onDiscard={discard}
				onGroupAction={groupAction}
				onOpen={(item, preview) => onOpen({ item, kind: 'diff' }, preview)}
				onOpenFile={(item) => onOpen({ kind: 'file', path: item.path }, true)}
				onToggle={toggle}
				selectedKey={selectedKey}
			/>
			<ConfirmChangeDialog
				onCancel={() => setPending(null)}
				onConfirm={() => {
					if (pending === null) return;
					setPending(null);
					pending.proceed();
				}}
				question={pending?.question ?? null}
			/>
		</>
	);
}
