import { workbenchMessages } from '@orchbee/i18n';

import type {
	WorkbenchWorktreeCommand,
	WorkbenchWorktreeFailureCause,
} from '../types/workbench-worktrees';

type WorktreeFailureDescriptor = (typeof workbenchMessages)[keyof typeof workbenchMessages];

const WORKTREE_FAILURE_DESCRIPTORS: Readonly<
	Record<WorkbenchWorktreeFailureCause, WorktreeFailureDescriptor>
> = {
	'checkout-in-use': workbenchMessages.worktreeFailureCheckoutInUse,
	'checkout-not-found': workbenchMessages.worktreeFailureCheckoutNotFound,
	'close-incomplete': workbenchMessages.worktreeFailureCloseIncomplete,
	'configuration-unavailable': workbenchMessages.worktreeFailureConfigurationUnavailable,
	denied: workbenchMessages.worktreeFailureDenied,
	'directory-in-use': workbenchMessages.worktreeFailureDirectoryInUse,
	dirty: workbenchMessages.worktreeFailureDirty,
	git: workbenchMessages.worktreeFailureGit,
	'git-failed': workbenchMessages.worktreeFailureGitFailed,
	internal: workbenchMessages.worktreeFailureInternal,
	'main-checkout': workbenchMessages.worktreeFailureMainCheckout,
	'not-submitted': workbenchMessages.worktreeFailureNotSubmitted,
	'session-stopping': workbenchMessages.worktreeFailureSessionStopping,
	'source-changed': workbenchMessages.worktreeFailureSourceChanged,
	unsupported: workbenchMessages.worktreeFailureUnsupported,
	'worker-unavailable': workbenchMessages.worktreeFailureWorkerUnavailable,
};

/** Causes whose words fit any Worktree command; the rest describe only a removal. */
const WORKTREE_COMMAND_FAILURES: ReadonlySet<WorkbenchWorktreeFailureCause> = new Set([
	'checkout-not-found',
	'configuration-unavailable',
	'denied',
	'internal',
	'not-submitted',
	'session-stopping',
	'source-changed',
	'unsupported',
	'worker-unavailable',
]);

export function isWorkbenchWorktreeFailureCause(
	value: unknown,
): value is WorkbenchWorktreeFailureCause {
	return typeof value === 'string' && Object.hasOwn(WORKTREE_FAILURE_DESCRIPTORS, value);
}

/** Names why a Worktree command stopped; what Git refused in a creation or opening stays general. */
export function workbenchWorktreeFailureDescriptor(
	command: WorkbenchWorktreeCommand['kind'],
	cause: WorkbenchWorktreeFailureCause,
): WorktreeFailureDescriptor {
	if (command === 'create' && !WORKTREE_COMMAND_FAILURES.has(cause)) {
		return workbenchMessages.worktreeCreateFailed;
	}
	if (command === 'open' && !WORKTREE_COMMAND_FAILURES.has(cause)) {
		return workbenchMessages.worktreeOpenFailed;
	}
	return WORKTREE_FAILURE_DESCRIPTORS[cause];
}
