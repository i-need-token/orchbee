import type { ActionResult } from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import { collectPanelLeaves } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityClosure,
	WorkbenchAuthorityCreation,
	WorkbenchEarlierLayoutOutcome,
	WorkbenchTabCloseStop,
	WorkbenchTabDuplicationStop,
} from '../types/workbench-authority';
import type {
	WorkbenchConfiguredCommandOutcome,
	WorkbenchConfiguredCommandRefusal,
} from '../types/workbench-configured-commands';
import type {
	WorkbenchWorktreeCommand,
	WorkbenchWorktreeOutcome,
} from '../types/workbench-worktrees';
import type { WorkbenchProjectWorkspaceRuntime } from './WorkbenchProjectWorkspaceRuntime';
import {
	readWorkbenchDirectoryRefusal,
	type WorkbenchDirectoryRefusal,
} from './readWorkbenchDirectoryRefusal';
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

const CONFIGURED_COMMAND_REFUSALS = {
	busy: workbenchMessages.configuredCommandBusy,
	conflict: workbenchMessages.configuredCommandConflict,
	denied: workbenchMessages.configuredCommandDenied,
	'invalid-intent': workbenchMessages.configuredCommandInvalidIntent,
	'not-found': workbenchMessages.configuredCommandNotFound,
	'session-mismatch': workbenchMessages.configuredCommandSessionMismatch,
	'stale-source': workbenchMessages.configuredCommandStaleSource,
	unavailable: workbenchMessages.configuredCommandUnavailable,
} as const satisfies Record<
	WorkbenchConfiguredCommandRefusal,
	WorkbenchAuthorityNotice['descriptor']
>;

/**
 * How a configured command ended, naming it, for as long as the TUI's status line says so. A command
 * that started says so too, since a shell command shows nowhere in the Workbench.
 */
export function workbenchConfiguredCommandNotice(
	command: string,
	outcome: WorkbenchConfiguredCommandOutcome,
): WorkbenchAuthorityNotice {
	const descriptors = {
		accepted: workbenchMessages.configuredCommandAccepted,
		failed: workbenchMessages.configuredCommandFailed,
		unknown: workbenchMessages.configuredCommandUnknown,
	} as const;
	return {
		descriptor:
			outcome.kind === 'refused'
				? CONFIGURED_COMMAND_REFUSALS[outcome.reason]
				: descriptors[outcome.kind],
		lifetime: 'timed',
		values: { command },
	};
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

/** Whether the Panel is its Tab's last, whose close takes the Tab with it. */
function closesTab(
	runtime: WorkbenchProjectWorkspaceRuntime,
	terminalContextId: string,
	tabId: string,
): boolean {
	const tab = runtime
		.getTerminalContext(terminalContextId)
		?.panelWorkspaceStore.getSnapshot()
		.tabs.find((candidate) => candidate.tabId === tabId);
	return tab !== undefined && collectPanelLeaves(tab.root).length === 1;
}

export function readWorkbenchCloseSubject(
	closure: WorkbenchAuthorityClosure,
	runtime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchCloseSubject {
	switch (closure.type) {
		case 'panel-close':
		case 'panel-close-if-idle':
			return closesTab(runtime, closure.terminalContextId, closure.tabId)
				? tabSubject(runtime, closure.terminalContextId, closure.tabId)
				: { kind: 'panel' };
		case 'tab-close-resume':
			return tabSubject(runtime, closure.terminalContextId, closure.tabId);
		case 'project-workspaces-close':
			return {
				kind: 'project',
				label:
					runtime.getSnapshot().projects.find((project) => project.id === closure.projectId)
						?.name ?? '',
			};
		case 'tabs-close':
		case 'tabs-close-if-idle': {
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

/** A member still counts while the authority holds it, though the strip leaves it out meanwhile. */
function tabSetCounts(
	subject: Extract<WorkbenchCloseSubject, { readonly kind: 'tabs' }>,
	runtime: WorkbenchProjectWorkspaceRuntime,
): { readonly completed: number; readonly remaining: number } {
	const remaining = subject.tabIds.filter((tabId) =>
		runtime.holdsTab(subject.terminalContextId, tabId),
	).length;
	return { completed: subject.tabIds.length - remaining, remaining };
}

/**
 * What to say while a close runs. A Panel that is not its Tab's last settles on acceptance, so it
 * never shows this.
 */
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
			closure.type === 'panel-close-if-idle' ||
			closure.type === 'tab-close-resume' ||
			closure.type === 'tabs-close' ||
			closure.type === 'tabs-close-if-idle'
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
 * A closed Tab says nothing, as in the TUI: the Tab leaving the strip already showed it.
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
			if (result.kind === 'completed') return null;
			const values = { label: subject.label };
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
			// A set the authority did not take closed nothing.
			if (result.kind === 'blocked') {
				return { descriptor: workbenchMessages.tabCloseNotApplied, lifetime: 'timed' };
			}
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
	if (result.kind === 'blocked' && result.code === 'launch-request-too-large') {
		return { descriptor: workbenchMessages.launchRequestTooLarge, lifetime: 'until-press' };
	}
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

const DIRECTORY_REFUSAL_NOTICES = {
	'directory-inaccessible': workbenchMessages.workspaceSetupDirectoryRefusedNoticeInaccessible,
	'directory-missing': workbenchMessages.workspaceSetupDirectoryRefusedNoticeMissing,
	'directory-unavailable': workbenchMessages.workspaceSetupDirectoryRefusedNoticeUnavailable,
	'not-a-directory': workbenchMessages.workspaceSetupDirectoryRefusedNoticeNotADirectory,
} as const satisfies Record<WorkbenchDirectoryRefusal, WorkbenchAuthorityNotice['descriptor']>;

/**
 * A creation's refusal. A folder the person named that the Server would not use says why and which
 * folder, in the words the TUI shows under its folder field; a folder field that reopens with the
 * reason withdraws it. A creation that names no folder keeps the TUI's not-applied words.
 */
export function workbenchCreationFailureNotice(
	creation: WorkbenchAuthorityCreation,
	result: ActionResult<unknown>,
): WorkbenchAuthorityNotice | null {
	const refusal = readWorkbenchDirectoryRefusal(result);
	if (
		refusal === null ||
		(creation.type !== 'project-created' && creation.type !== 'workspace-created-at-path')
	) {
		return workbenchFailureNotice(result);
	}
	return {
		descriptor: DIRECTORY_REFUSAL_NOTICES[refusal],
		lifetime: 'until-press',
		values: { path: creation.path },
	};
}

/** Whether the notice says why a named folder was refused. */
export function isWorkbenchDirectoryRefusalNotice(notice: WorkbenchAuthorityNotice): boolean {
	return Object.values<WorkbenchAuthorityNotice['descriptor']>(DIRECTORY_REFUSAL_NOTICES).includes(
		notice.descriptor,
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

const EARLIER_LAYOUT_REFUSALS = {
	busy: workbenchMessages.earlierLayoutBusy,
	denied: workbenchMessages.earlierLayoutDenied,
	'not-found': workbenchMessages.earlierLayoutNotFound,
	'nothing-to-reopen': workbenchMessages.earlierLayoutNothingToReopen,
	'resource-limit': workbenchMessages.earlierLayoutResourceLimit,
	unavailable: workbenchMessages.earlierLayoutsUnavailable,
	unknown: workbenchMessages.earlierLayoutUnknown,
} as const;

/** What a reopen of an earlier layout did, in the TUI's words, including what it reopened. */
export function workbenchEarlierLayoutNotice(
	outcome: WorkbenchEarlierLayoutOutcome,
): WorkbenchAuthorityNotice {
	if (outcome.kind !== 'reopened') {
		return { descriptor: EARLIER_LAYOUT_REFUSALS[outcome.kind], lifetime: 'timed' };
	}
	const { tabs, workspaces } = outcome;
	let descriptor: WorkbenchAuthorityNotice['descriptor'] =
		workbenchMessages.earlierLayoutReopenedWorkspacesTabs;
	if (workspaces === 0) descriptor = workbenchMessages.earlierLayoutReopenedTabs;
	else if (tabs === 0) descriptor = workbenchMessages.earlierLayoutReopenedWorkspaces;
	return { descriptor, lifetime: 'timed', values: { tabs, workspaces } };
}
