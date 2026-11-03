import type { ActionResult } from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import type { WorkbenchTabDuplicationStop } from '../actions/workbench-shared-tab-creation';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityClosure,
	WorkbenchTabCloseStop,
} from '../types/workbench-authority';
import type {
	WorkbenchWorktreeCommand,
	WorkbenchWorktreeOutcome,
} from '../types/workbench-worktrees';
import type { WorkbenchProjectWorkspaceRuntime } from './WorkbenchProjectWorkspaceRuntime';
import { workbenchWorktreeFailureDescriptor } from './workbench-worktree-failure';

export interface WorkbenchAuthorityNotice {
	readonly descriptor: (typeof workbenchMessages)[keyof typeof workbenchMessages];
	/**
	 * `until-press` is what the person has to act on. `until-replaced` describes work still running,
	 * so a press does not end it. A `timed` notice never replaces one that stays until a press.
	 */
	readonly lifetime: 'timed' | 'until-press' | 'until-replaced';
	readonly values?: Readonly<Record<string, number | string>>;
}

/** What is closing, read before it closes, since the words outlive the thing they name. */
type WorkbenchCloseSubject =
	| { readonly kind: 'panel' }
	| { readonly kind: 'project'; readonly label: string }
	| {
			readonly kind: 'tab';
			readonly label: string;
			readonly tabId: string;
			readonly terminalContextId: string;
	  }
	| {
			readonly kind: 'tabs';
			readonly tabIds: readonly string[];
			readonly terminalContextId: string;
	  }
	| { readonly kind: 'workspace'; readonly label: string; readonly workspaceId: string };

function tabSubject(
	runtime: WorkbenchProjectWorkspaceRuntime,
	terminalContextId: string,
	tabId: string,
): WorkbenchCloseSubject {
	return {
		kind: 'tab',
		label:
			runtime
				.getTerminalContext(terminalContextId)
				?.tabModel.getSnapshot()
				.tabs.find((tab) => tab.id === tabId)?.title ?? '',
		tabId,
		terminalContextId,
	};
}

export function readWorkbenchCloseSubject(
	closure: WorkbenchAuthorityClosure,
	runtime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchCloseSubject {
	switch (closure.type) {
		case 'panel-close':
			return { kind: 'panel' };
		case 'tab-close-resume':
			return tabSubject(runtime, closure.terminalContextId, closure.tabId);
		case 'project-workspaces-close':
			return {
				kind: 'project',
				label:
					runtime.getSnapshot().projects.find((project) => project.id === closure.projectId)
						?.name ?? '',
			};
		case 'tabs-close': {
			const [only, ...others] = closure.tabIds;
			if (only === undefined || others.length > 0) {
				return {
					kind: 'tabs',
					tabIds: closure.tabIds,
					terminalContextId: closure.terminalContextId,
				};
			}
			return tabSubject(runtime, closure.terminalContextId, only);
		}
		case 'workspace-close':
		case 'workspace-close-if-idle':
			return {
				kind: 'workspace',
				label:
					runtime.getSnapshot().workspaces.find((workspace) => workspace.id === closure.workspaceId)
						?.name ?? '',
				workspaceId: closure.workspaceId,
			};
	}
}

function tabSetCounts(
	subject: Extract<WorkbenchCloseSubject, { readonly kind: 'tabs' }>,
	runtime: WorkbenchProjectWorkspaceRuntime,
): { readonly completed: number; readonly remaining: number } {
	const present = new Set(
		runtime
			.getTerminalContext(subject.terminalContextId)
			?.tabModel.getSnapshot()
			.tabs.map((tab) => tab.id),
	);
	const remaining = subject.tabIds.filter((tabId) => present.has(tabId)).length;
	return { completed: subject.tabIds.length - remaining, remaining };
}

/** What to say while a close runs. A Panel settles on acceptance, so it never shows this. */
export function workbenchCloseProgressNotice(
	subject: WorkbenchCloseSubject,
	runtime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchAuthorityNotice | null {
	const lifetime = 'until-replaced';
	switch (subject.kind) {
		case 'panel':
			return null;
		case 'project':
			return {
				descriptor: workbenchMessages.projectCloseClosing,
				lifetime,
				values: { label: subject.label },
			};
		case 'tab':
			return {
				descriptor: workbenchMessages.tabCloseClosing,
				lifetime,
				values: { label: subject.label },
			};
		case 'tabs':
			return {
				descriptor: workbenchMessages.tabCloseSetProgress,
				lifetime,
				values: tabSetCounts(subject, runtime),
			};
		case 'workspace':
			return {
				descriptor: workbenchMessages.workspaceCloseClosing,
				lifetime,
				values: { label: subject.label },
			};
	}
}

/** The question was withdrawn because what it described changed. */
export function workbenchCloseWithdrawnNotice(
	closure: WorkbenchAuthorityClosure,
): WorkbenchAuthorityNotice {
	return {
		descriptor:
			closure.type === 'panel-close' ||
			closure.type === 'tab-close-resume' ||
			closure.type === 'tabs-close'
				? workbenchMessages.tabCloseCancelled
				: workbenchMessages.workspaceCloseChanged,
		lifetime: 'timed',
	};
}

/** A close that may continue says so, so the person knows Resume is there. */
function tabCloseFailureDescriptor(
	code: string,
	stop: WorkbenchTabCloseStop | null,
): WorkbenchAuthorityNotice['descriptor'] {
	switch (stop) {
		case 'awaiting-removal':
			return workbenchMessages.tabCloseAwaitingRemovalResumable;
		case 'cleanup-blocked':
			return workbenchMessages.tabCloseCleanupBlockedResumable;
		case null:
			break;
	}
	switch (code) {
		case 'awaiting-removal':
			return workbenchMessages.tabCloseAwaitingRemoval;
		case 'cleanup-blocked':
			return workbenchMessages.tabCloseCleanupBlocked;
		case 'outcome-unknown':
			return workbenchMessages.tabCloseOutcomeUnknown;
		default:
			return workbenchMessages.tabCloseStatusUnavailable;
	}
}

function refusedOrChanged(
	code: string,
	refused: WorkbenchAuthorityNotice,
): WorkbenchAuthorityNotice {
	return code === 'scope-changed'
		? { descriptor: workbenchMessages.workspaceCloseChanged, lifetime: 'timed' }
		: refused;
}

/**
 * What a finished close says. A request for confirmation says nothing, because the question follows
 * it. `sawClosed` is false when the person has since gone elsewhere, where Closed would interrupt.
 */
export function workbenchCloseResultNotice(
	subject: WorkbenchCloseSubject,
	result: ActionResult<void>,
	runtime: WorkbenchProjectWorkspaceRuntime,
	authority: Pick<WorkbenchAuthority, 'stoppedTabClose'>,
	sawClosed: boolean,
): WorkbenchAuthorityNotice | null {
	if (
		result.kind === 'cancelled' ||
		result.kind === 'unchanged' ||
		result.kind === 'unsupported' ||
		(result.kind === 'blocked' && result.code === 'confirmation-required')
	) {
		return null;
	}
	switch (subject.kind) {
		case 'panel':
			if (result.kind === 'completed') return null;
			return result.kind === 'failed' && result.failure.code === 'outcome-unknown'
				? { descriptor: workbenchMessages.workspaceSetupOutcomeUnknown, lifetime: 'until-press' }
				: { descriptor: workbenchMessages.panelCloseNotApplied, lifetime: 'timed' };
		case 'project': {
			const values = { label: subject.label };
			if (result.kind === 'completed') {
				return sawClosed
					? { descriptor: workbenchMessages.projectCloseClosed, lifetime: 'timed', values }
					: null;
			}
			if (result.kind === 'blocked') {
				return refusedOrChanged(result.code, {
					descriptor: workbenchMessages.projectCloseRefused,
					lifetime: 'until-press',
					values,
				});
			}
			return result.failure.code === 'outcome-unknown'
				? {
						descriptor: workbenchMessages.projectCloseOutcomeUnknown,
						lifetime: 'until-press',
						values: {
							...values,
							operationId: typeof result.failure.details === 'string' ? result.failure.details : '',
						},
					}
				: { descriptor: workbenchMessages.projectCloseStopped, lifetime: 'until-press', values };
		}
		case 'tab': {
			const values = { label: subject.label };
			if (result.kind === 'completed') {
				return { descriptor: workbenchMessages.tabCloseClosed, lifetime: 'timed', values };
			}
			return result.kind === 'blocked'
				? { descriptor: workbenchMessages.tabCloseNotApplied, lifetime: 'timed' }
				: {
						descriptor: tabCloseFailureDescriptor(
							result.failure.code,
							authority.stoppedTabClose(subject.terminalContextId, subject.tabId),
						),
						lifetime: 'until-press',
						values,
					};
		}
		case 'tabs': {
			const values = tabSetCounts(subject, runtime);
			return result.kind === 'completed'
				? { descriptor: workbenchMessages.tabCloseSetProgress, lifetime: 'timed', values }
				: { descriptor: workbenchMessages.tabCloseSetStopped, lifetime: 'until-press', values };
		}
		case 'workspace': {
			const values = { label: subject.label };
			if (result.kind === 'completed') {
				return sawClosed
					? { descriptor: workbenchMessages.workspaceCloseClosed, lifetime: 'timed', values }
					: null;
			}
			if (result.kind === 'blocked') {
				return refusedOrChanged(result.code, {
					descriptor: workbenchMessages.workspaceCloseRefused,
					lifetime: 'until-press',
					values,
				});
			}
			return result.failure.code === 'outcome-unknown'
				? {
						descriptor: workbenchMessages.workspaceCloseOutcomeUnknown,
						lifetime: 'until-press',
						values: { ...values, workspaceId: subject.workspaceId },
					}
				: {
						descriptor: workbenchMessages.workspaceCloseStopped,
						lifetime: 'until-press',
						values,
					};
		}
	}
}

/** Only a refusal or an unknown outcome needs words: a committed change shows in the Workbench. */
function failureNotice(
	result: ActionResult<unknown>,
	notApplied: WorkbenchAuthorityNotice['descriptor'],
	stale: WorkbenchAuthorityNotice['descriptor'],
): WorkbenchAuthorityNotice | null {
	switch (result.kind) {
		case 'blocked':
			return {
				descriptor: result.code === 'revision-mismatch' ? stale : notApplied,
				lifetime: 'until-press',
			};
		case 'failed':
			return {
				descriptor:
					result.failure.code === 'outcome-unknown'
						? workbenchMessages.workspaceSetupOutcomeUnknown
						: notApplied,
				lifetime: 'until-press',
			};
		case 'cancelled':
		case 'completed':
		case 'unchanged':
		case 'unsupported':
			return null;
	}
}

export function workbenchFailureNotice(
	result: ActionResult<unknown>,
): WorkbenchAuthorityNotice | null {
	return failureNotice(
		result,
		workbenchMessages.workspaceSetupNotApplied,
		workbenchMessages.workspaceSetupStale,
	);
}

export function workbenchTabsFailureNotice(
	result: ActionResult<unknown>,
): WorkbenchAuthorityNotice | null {
	return failureNotice(
		result,
		workbenchMessages.tabsNotApplied,
		workbenchMessages.workspaceSetupStale,
	);
}

export function workbenchTabCreationFailureNotice(
	result: ActionResult<unknown>,
): WorkbenchAuthorityNotice | null {
	// A changed, removed or unsupported launch profile is fixed in Settings, never by another program.
	if (result.kind === 'blocked' && result.code === 'launch-profile-unavailable') {
		return { descriptor: workbenchMessages.tabCreateProfileUnavailable, lifetime: 'until-press' };
	}
	return failureNotice(
		result,
		workbenchMessages.tabCreateNotCreated,
		workbenchMessages.tabCreateNotCreated,
	);
}

export function workbenchPanelsFailureNotice(
	result: ActionResult<unknown>,
): WorkbenchAuthorityNotice | null {
	return failureNotice(
		result,
		workbenchMessages.panelsNotApplied,
		workbenchMessages.panelsNotApplied,
	);
}

export function workbenchSplitRatioFailureNotice(
	result: ActionResult<unknown>,
): WorkbenchAuthorityNotice | null {
	switch (result.kind) {
		case 'blocked':
			return {
				descriptor:
					result.code === 'ratio-contended'
						? workbenchMessages.panelResizeContended
						: workbenchMessages.panelResizeFailed,
				lifetime: 'until-press',
			};
		case 'failed':
			return {
				descriptor:
					result.failure.code === 'outcome-unknown'
						? workbenchMessages.panelResizeOutcomeUnknown
						: workbenchMessages.panelResizeFailed,
				lifetime: 'until-press',
			};
		case 'cancelled':
		case 'completed':
		case 'unchanged':
		case 'unsupported':
			return null;
	}
}

const DUPLICATION_STOP_DESCRIPTORS: Readonly<
	Record<WorkbenchTabDuplicationStop, WorkbenchAuthorityNotice['descriptor']>
> = {
	changed: workbenchMessages.tabDuplicationChanged,
	deadline: workbenchMessages.tabDuplicationDeadline,
	'not-created': workbenchMessages.tabDuplicationNotCreated,
	'terminal-not-started': workbenchMessages.tabDuplicationUncertain,
};

export function workbenchTabDuplicationStoppedNotice(
	stop: WorkbenchTabDuplicationStop,
	counts: { readonly created: number; readonly total: number },
): WorkbenchAuthorityNotice {
	return { descriptor: DUPLICATION_STOP_DESCRIPTORS[stop], lifetime: 'timed', values: counts };
}

/** What a Worktree command says while it runs, until its outcome replaces it. */
export function workbenchWorktreeProgressNotice(
	command: WorkbenchWorktreeCommand,
): WorkbenchAuthorityNotice {
	const descriptors = {
		create: workbenchMessages.worktreeCreating,
		open: workbenchMessages.worktreeOpening,
		remove: workbenchMessages.worktreeRemoving,
		resume: workbenchMessages.worktreeRemoving,
	} as const;
	return { descriptor: descriptors[command.kind], lifetime: 'until-replaced' };
}

/**
 * What a settled Worktree command says. An opened Worktree shows as its Workspace. A failure stays
 * until a press, so a removal whose Resume question is declined keeps its receipt. A removal says
 * that the branch stays, which nothing shows.
 */
export function workbenchWorktreeOutcomeNotice(
	command: WorkbenchWorktreeCommand,
	outcome: WorkbenchWorktreeOutcome,
): WorkbenchAuthorityNotice | null {
	switch (outcome.kind) {
		case 'opened':
			return null;
		case 'removed':
			return { descriptor: workbenchMessages.worktreeRemoved, lifetime: 'timed' };
		case 'unknown':
			return {
				descriptor: workbenchMessages.worktreeOutcomeUnknown,
				lifetime: 'until-press',
				values: { operationId: outcome.operationId },
			};
		case 'failed':
			return {
				descriptor: workbenchWorktreeFailureDescriptor(command.kind, outcome.cause),
				lifetime: 'until-press',
			};
	}
}
