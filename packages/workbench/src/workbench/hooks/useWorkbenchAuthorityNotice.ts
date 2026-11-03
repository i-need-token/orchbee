import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages, type IntlShape } from '@orchbee/i18n';
import * as React from 'react';

import type { WorkbenchTabDuplicationStop } from '../actions/workbench-shared-tab-creation';
import {
	readWorkbenchCloseSubject,
	workbenchCloseProgressNotice,
	workbenchCloseResultNotice,
	workbenchCloseWithdrawnNotice,
	workbenchConfiguredCommandNotice,
	workbenchFailureNotice,
	workbenchPanelsFailureNotice,
	workbenchSplitRatioFailureNotice,
	workbenchTabCreationFailureNotice,
	workbenchTabDuplicationStoppedNotice,
	workbenchTabsFailureNotice,
	workbenchWorktreeOutcomeNotice,
	workbenchWorktreeProgressNotice,
	type WorkbenchAuthorityNotice,
} from '../model/workbench-authority-notice';
import { formatWorkbenchConfiguredCommandName } from '../model/workbench-configured-command-name';
import { workbenchActiveLocationsEqual } from '../model/workbench-terminal-location-activation';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority, WorkbenchAuthorityClosure } from '../types/workbench-authority';
import type { WorkbenchConfiguredCommands } from '../types/workbench-configured-commands';
import type { WorkbenchWorktrees } from '../types/workbench-worktrees';

export interface WorkbenchAuthorityNoticeView {
	readonly authority: WorkbenchAuthority | undefined;
	readonly notice: { readonly message: string; readonly persistent: boolean } | null;
	readonly reportDuplicationStopped: (
		stop: WorkbenchTabDuplicationStop,
		counts: { readonly created: number; readonly total: number },
	) => void;
}

/** The same lifetime a TUI status line gives a transient notice. */
const TIMED_NOTICE_MILLISECONDS = 4_000;
/** A close that settles faster than this reads as immediate, so it never says it is closing. */
const CLOSING_NOTICE_DELAY_MILLISECONDS = 200;

function closesActiveLocation(
	closure: WorkbenchAuthorityClosure,
	runtime: WorkbenchProjectWorkspaceRuntime,
): boolean {
	const state = runtime.getSnapshot();
	const location = state.activeLocation;
	if (location.kind !== 'workspace') return false;
	if (closure.type === 'workspace-close' || closure.type === 'workspace-close-if-idle') {
		return closure.workspaceId === location.workspaceId;
	}
	return (
		closure.type === 'project-workspaces-close' &&
		state.workspaces.find((workspace) => workspace.id === location.workspaceId)?.projectId ===
			closure.projectId
	);
}

/**
 * A configured command says how it ended, naming it, as the TUI's status line does. The name is read
 * before the command runs, since the configuration may take the command away meanwhile.
 */
function reportingConfiguredCommands(
	commands: WorkbenchConfiguredCommands,
	intl: IntlShape,
	setNotice: React.Dispatch<React.SetStateAction<WorkbenchAuthorityNotice | null>>,
): WorkbenchConfiguredCommands {
	return {
		getSnapshot: () => commands.getSnapshot(),
		run: async (commandId) => {
			const name = formatWorkbenchConfiguredCommandName(intl, commands.getSnapshot(), commandId);
			const outcome = await commands.run(commandId);
			setNotice(workbenchConfiguredCommandNotice(name, outcome));
			return outcome;
		},
		subscribe: (listener) => commands.subscribe(listener),
	};
}

/** A Worktree command says what runs and how it ended, since Git's work shows nowhere else. */
function reportingWorktrees(
	worktrees: WorkbenchWorktrees,
	setNotice: React.Dispatch<React.SetStateAction<WorkbenchAuthorityNotice | null>>,
): WorkbenchWorktrees {
	return {
		discover: (workspaceId) => worktrees.discover(workspaceId),
		run: async (command) => {
			const progress = workbenchWorktreeProgressNotice(command);
			setNotice(progress);
			const outcome = await worktrees.run(command);
			const next = workbenchWorktreeOutcomeNotice(command, outcome);
			// Saying nothing removes only what this command said, not what later work says.
			setNotice((current) => (next === null ? (current === progress ? null : current) : next));
			return outcome;
		},
	};
}

/**
 * Reports what the authority decided without taking the person from what they are doing: only a
 * refusal or an unknown outcome needs words, because a committed change shows in the Workbench.
 * Closing ends terminals for every Client, so it also says how it is going and how it ended, and
 * dissolving a Project says that its Workspaces remain.
 */
export function useWorkbenchAuthorityNotice(
	authority: WorkbenchAuthority | undefined,
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchAuthorityNoticeView {
	const intl = useOrchbeeIntl();
	const [notice, setNotice] = React.useState<WorkbenchAuthorityNotice | null>(null);
	const reportingAuthority = React.useMemo<WorkbenchAuthority | undefined>(
		() =>
			authority === undefined
				? undefined
				: {
						minimumSplitPercent: authority.minimumSplitPercent,
						change: async (context, change) => {
							const dissolvedWorkspaces =
								change.type === 'project-dissolved' &&
								projectWorkspaceRuntime
									.getSnapshot()
									.workspaces.some((workspace) => workspace.projectId === change.projectId);
							setNotice(null);
							const result = await authority.change(context, change);
							if (result.kind === 'completed' && change.type === 'project-dissolved') {
								setNotice({
									descriptor: dissolvedWorkspaces
										? workbenchMessages.workspaceSetupProjectDissolved
										: workbenchMessages.workspaceSetupProjectDissolvedEmpty,
									lifetime: 'timed',
								});
							} else if (change.type === 'tab-renamed' || change.type === 'tabs-changed') {
								setNotice(workbenchTabsFailureNotice(result));
							} else if (change.type === 'panel-moved' || change.type === 'panels-swapped') {
								setNotice(workbenchPanelsFailureNotice(result));
							} else if (
								change.type === 'panel-size-adjusted' ||
								change.type === 'panels-equalized' ||
								change.type === 'split-ratio-set'
							) {
								setNotice(workbenchSplitRatioFailureNotice(result));
							} else {
								setNotice(workbenchFailureNotice(result));
							}
							return result;
						},
						close: async (context, closure) => {
							const subject = readWorkbenchCloseSubject(closure, projectWorkspaceRuntime);
							const origin = projectWorkspaceRuntime.getSnapshot().activeLocation;
							const landsAutomatically = closesActiveLocation(closure, projectWorkspaceRuntime);
							let progress: WorkbenchAuthorityNotice | null = null;
							let closingShown = false;
							const showProgress = (): void => {
								if (!closingShown) return;
								progress = workbenchCloseProgressNotice(subject, projectWorkspaceRuntime);
								setNotice(progress);
							};
							setNotice(null);
							const closingDelay = setTimeout(() => {
								closingShown = true;
								showProgress();
							}, CLOSING_NOTICE_DELAY_MILLISECONDS);
							const stopWatching = projectWorkspaceRuntime.subscribe(showProgress);
							try {
								const result = await authority.close(context, closure);
								const next = workbenchCloseResultNotice(
									subject,
									result,
									projectWorkspaceRuntime,
									authority,
									landsAutomatically ||
										workbenchActiveLocationsEqual(
											origin,
											projectWorkspaceRuntime.getSnapshot().activeLocation,
										),
								);
								const shown = progress;
								setNotice((current) => {
									// Saying nothing removes only what this close said, not what later work says.
									if (next === null) return current === shown ? null : current;
									return current?.lifetime === 'until-press' && next.lifetime === 'timed'
										? current
										: next;
								});
								return result;
							} finally {
								clearTimeout(closingDelay);
								stopWatching();
							}
						},
						create: async (context, creation) => {
							setNotice(null);
							const result = await authority.create(context, creation);
							setNotice(workbenchFailureNotice(result));
							return result;
						},
						createPanelTerminal: async (context, creation) => {
							setNotice(null);
							const result = await authority.createPanelTerminal(context, creation);
							setNotice(workbenchPanelsFailureNotice(result));
							return result;
						},
						createTab: async (context, creation) => {
							setNotice(null);
							const result = await authority.createTab(context, creation);
							setNotice(workbenchTabCreationFailureNotice(result));
							return result;
						},
						launchProfiles: authority.launchProfiles,
						stoppedTabClose: (terminalContextId, tabId) =>
							authority.stoppedTabClose(terminalContextId, tabId),
						watchClose: (closure, onChanged) =>
							authority.watchClose(closure, () => {
								setNotice(workbenchCloseWithdrawnNotice(closure));
								onChanged();
							}),
						worktrees:
							authority.worktrees === undefined
								? undefined
								: reportingWorktrees(authority.worktrees, setNotice),
						configuredCommands:
							authority.configuredCommands === undefined
								? undefined
								: reportingConfiguredCommands(authority.configuredCommands, intl, setNotice),
					},
		[authority, intl, projectWorkspaceRuntime],
	);
	const reportDuplicationStopped = useMemoizedFn(
		(
			stop: WorkbenchTabDuplicationStop,
			counts: { readonly created: number; readonly total: number },
		) => {
			setNotice(workbenchTabDuplicationStoppedNotice(stop, counts));
		},
	);
	React.useEffect(() => {
		if (notice === null || notice.lifetime === 'until-replaced') return undefined;
		if (notice.lifetime === 'timed') {
			const expiry = setTimeout(() => setNotice(null), TIMED_NOTICE_MILLISECONDS);
			return () => clearTimeout(expiry);
		}
		const dismiss = (): void => setNotice(null);
		window.addEventListener('keydown', dismiss, true);
		window.addEventListener('pointerdown', dismiss, true);
		return () => {
			window.removeEventListener('keydown', dismiss, true);
			window.removeEventListener('pointerdown', dismiss, true);
		};
	}, [notice]);
	return {
		authority: reportingAuthority,
		notice:
			notice === null
				? null
				: {
						message: intl.formatMessage(notice.descriptor, notice.values),
						persistent: notice.lifetime === 'until-press',
					},
		reportDuplicationStopped,
	};
}
