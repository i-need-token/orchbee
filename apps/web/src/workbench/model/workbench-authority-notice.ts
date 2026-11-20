import type { ActionResult } from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import {
	collectPanelLeaves,
	findPanel,
	getDisplayedTerminal,
	getTerminalDisplayTitle,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type {
	WorkbenchAuthorityClosure,
	WorkbenchAuthorityCreation,
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
	'source-exited': workbenchMessages.configuredCommandSourceExited,
	'stale-source': workbenchMessages.configuredCommandStaleSource,
	unavailable: workbenchMessages.configuredCommandUnavailable,
} as const satisfies Record<
	WorkbenchConfiguredCommandRefusal,
	WorkbenchAuthorityNotice['descriptor']
>;

/**
 * How a configured command ended, naming it, for as long as the TUI's status line says so. A command
 * that started says nothing: success has no notice.
 */
export function workbenchConfiguredCommandNotice(
	command: string,
	outcome: WorkbenchConfiguredCommandOutcome,
): WorkbenchAuthorityNotice | null {
	if (outcome.kind === 'accepted') return null;
	const descriptors = {
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
	| { readonly kind: 'panel'; readonly label: string }
	| { readonly kind: 'project'; readonly label: string }
	| { readonly kind: 'tab'; readonly label: string }
	| { readonly kind: 'tabs' }
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
	};
}

function panelSubject(
	runtime: WorkbenchProjectWorkspaceRuntime,
	terminalContextId: string,
	tabId: string,
	panelId: string,
): WorkbenchCloseSubject {
	const tab = runtime
		.getTerminalContext(terminalContextId)
		?.panelWorkspaceStore.getSnapshot()
		.tabs.find((candidate) => candidate.tabId === tabId);
	const panel = tab === undefined ? null : findPanel(tab.root, panelId);
	return {
		kind: 'panel',
		label: panel === null ? '' : getTerminalDisplayTitle(getDisplayedTerminal(panel)),
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
				: panelSubject(runtime, closure.terminalContextId, closure.tabId, closure.panelId);
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
				return { kind: 'tabs' };
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

/**
 * What to say while a close runs. A Panel that is not its Tab's last settles on acceptance, so it
 * never shows this.
 */
export function workbenchCloseProgressNotice(
	subject: WorkbenchCloseSubject,
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
			return { descriptor: workbenchMessages.tabCloseSetProgress, lifetime };
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
			closure.type === 'tabs-close' ||
			closure.type === 'tabs-close-if-idle'
				? workbenchMessages.tabCloseCancelled
				: workbenchMessages.workspaceCloseChanged,
		lifetime: 'timed',
	};
}

/**
 * A close that ended any other way than its removal: Orchbee refused or gave up, or its status could
 * no longer be read. One sentence says so, without a reason, until the next press; what was closing
 * is back, and closing it again asks as usual.
 */
function closeFailedNotice(label: string): WorkbenchAuthorityNotice {
	return {
		descriptor: workbenchMessages.tabCloseFailed,
		lifetime: 'until-press',
		values: { label },
	};
}

/**
 * What a finished close says. A request for confirmation says nothing, because the question follows
 * it, and a finished close says nothing either: what closed has left the screen.
 */
export function workbenchCloseResultNotice(
	subject: WorkbenchCloseSubject,
	result: ActionResult<void>,
): WorkbenchAuthorityNotice | null {
	if (
		result.kind === 'completed' ||
		result.kind === 'cancelled' ||
		result.kind === 'unchanged' ||
		result.kind === 'unsupported' ||
		(result.kind === 'blocked' && result.code === 'confirmation-required')
	) {
		return null;
	}
	if (subject.kind === 'tabs') {
		// A set the authority did not take closed nothing; one it took may still be closing.
		return result.kind === 'blocked'
			? { descriptor: workbenchMessages.tabCloseNotApplied, lifetime: 'timed' }
			: { descriptor: workbenchMessages.tabCloseSetStopped, lifetime: 'until-press' };
	}
	if (subject.kind === 'project' || subject.kind === 'workspace') {
		if (result.kind === 'blocked' && result.code === 'scope-changed') {
			return { descriptor: workbenchMessages.workspaceCloseChanged, lifetime: 'timed' };
		}
		if (result.kind === 'failed' && result.failure.code === 'outcome-unknown') {
			return {
				descriptor: workbenchMessages.workspaceSetupOutcomeUnknown,
				lifetime: 'until-press',
			};
		}
	}
	// A Tab or Panel whose close lost its outcome is back, so it settled that the close did not happen.
	return closeFailedNotice(subject.label);
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
	// The Session's terminal bound frees only as terminals close, so the words name that step.
	if (result.kind === 'failed' && result.failure.code === 'terminal-limit-reached') {
		return { descriptor: workbenchMessages.terminalLimitReached, lifetime: 'until-press' };
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

/** Refusals another surface already explains: a question asks first, and a repeated press or a newer navigation owns the outcome. */
const ACTION_OUTCOMES_SAID_ELSEWHERE: ReadonlySet<string> = new Set([
	'confirmation-required',
	'duplicate-action-execution',
	'route-acceptance-replaced',
]);

/** Blocks that mean the Action is not offered right now, not that a change did not apply. */
const ACTION_UNAVAILABLE_CODES: ReadonlySet<string> = new Set([
	'action-unavailable',
	'host-capability-unavailable',
]);

/**
 * What an Action invoked from a control, menu, or shortcut says when it was refused or failed and no
 * surface has said so. Cancelled, completed, and unchanged results say nothing: the person chose the
 * outcome, or the Action intends it.
 */
export function workbenchActionOutcomeNotice(
	result: ActionResult<unknown>,
): WorkbenchAuthorityNotice | null {
	if (
		result.kind === 'unsupported' ||
		(result.kind === 'blocked' && ACTION_UNAVAILABLE_CODES.has(result.code))
	) {
		return { descriptor: workbenchMessages.actionUnavailable, lifetime: 'until-press' };
	}
	if (result.kind === 'blocked' && ACTION_OUTCOMES_SAID_ELSEWHERE.has(result.code)) return null;
	return workbenchFailureNotice(result);
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
				descriptor: workbenchMessages.panelResizeFailed,
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

/** A duplication whose last terminal may still be starting has an unknown outcome. */
export function workbenchTabDuplicationStoppedNotice(
	stop: WorkbenchTabDuplicationStop,
): WorkbenchAuthorityNotice {
	return {
		descriptor:
			stop === 'terminal-not-started'
				? workbenchMessages.workspaceSetupOutcomeUnknown
				: workbenchMessages.tabDuplicationNotCreated,
		lifetime: 'timed',
	};
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
 * What a settled Worktree command says. An opened Worktree shows as its Workspace and a removal as
 * the Workspace leaving, so success says nothing. A failure stays until a press, so a stopped
 * removal keeps its cause in view.
 */
export function workbenchWorktreeOutcomeNotice(
	command: WorkbenchWorktreeCommand,
	outcome: WorkbenchWorktreeOutcome,
): WorkbenchAuthorityNotice | null {
	switch (outcome.kind) {
		case 'opened':
		case 'removed':
			return null;
		case 'unknown':
			return { descriptor: workbenchMessages.worktreeOutcomeUnknown, lifetime: 'until-press' };
		case 'failed':
			return {
				descriptor: workbenchWorktreeFailureDescriptor(command.kind, outcome.cause),
				lifetime: 'until-press',
			};
	}
}
