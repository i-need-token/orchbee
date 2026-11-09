import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages, type IntlShape } from '@orchbee/i18n';
import * as React from 'react';

import {
	isWorkbenchDirectoryRefusalNotice,
	readWorkbenchCloseSubject,
	workbenchCloseProgressNotice,
	workbenchCloseResultNotice,
	workbenchCloseWithdrawnNotice,
	workbenchConfiguredCommandNotice,
	workbenchCreationFailureNotice,
	workbenchEarlierLayoutNotice,
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
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityClosure,
	WorkbenchEarlierLayouts,
	WorkbenchHeldRestores,
	WorkbenchTabDuplicationStop,
} from '../types/workbench-authority';
import type { WorkbenchConfiguredCommands } from '../types/workbench-configured-commands';
import type { WorkbenchWorktrees } from '../types/workbench-worktrees';

export interface WorkbenchAuthorityNoticeView {
	readonly authority: WorkbenchAuthority;
	readonly notice: { readonly message: string; readonly persistent: boolean } | null;
	/** An Agent destination failed revalidation, so nothing moved; the TUI says the same. */
	readonly reportAgentNavigationUnavailable: () => void;
	readonly reportDuplicationStopped: (
		stop: WorkbenchTabDuplicationStop,
		counts: { readonly created: number; readonly total: number },
	) => void;
	/** A folder field now shows why its folder was refused, so the status line no longer says it. */
	readonly withdrawDirectoryRefusal: () => void;
}

/** The same lifetime a TUI status line gives a transient notice. */
const TIMED_NOTICE_MILLISECONDS = 4_000;
/** A close that settles faster than this reads as immediate, so it never says it is closing. */
const CLOSING_NOTICE_DELAY_MILLISECONDS = 200;
/**
 * As in the TUI, one Tab's close stays off the status line for this long after it began, since its
 * Tab leaving the strip already shows it, so only a slow close says Closing.
 */
const SLOW_TAB_CLOSE_MILLISECONDS = 1_000;

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
		discover: (workspaceId, panel) => worktrees.discover(workspaceId, panel),
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
 * Starting a held restore says why it did not start, as the TUI's status line does, and that the
 * Agent conversation stays behind when it starts in the home folder instead.
 */
function reportingHeldRestores(
	restores: WorkbenchHeldRestores,
	setNotice: React.Dispatch<React.SetStateAction<WorkbenchAuthorityNotice | null>>,
): WorkbenchHeldRestores {
	return {
		restore: async (presentationId, directory) => {
			setNotice(null);
			const outcome = await restores.restore(presentationId, directory);
			let descriptor: WorkbenchAuthorityNotice['descriptor'] | null = null;
			if (outcome.kind === 'directory-unavailable') {
				descriptor = workbenchMessages.heldRestoreDirectoryUnavailable;
			} else if (outcome.kind === 'not-started') {
				descriptor = workbenchMessages.heldRestoreNotStarted;
			} else if (outcome.agentNotResumed) {
				descriptor = workbenchMessages.heldRestoreAgentNotResumed;
			}
			if (descriptor !== null) setNotice({ descriptor, lifetime: 'timed' });
			return outcome;
		},
	};
}

/** Reopening an earlier layout says what it reopened or why nothing came back, as the TUI does. */
function reportingEarlierLayouts(
	layouts: WorkbenchEarlierLayouts,
	setNotice: React.Dispatch<React.SetStateAction<WorkbenchAuthorityNotice | null>>,
): WorkbenchEarlierLayouts {
	return {
		list: () => layouts.list(),
		reopen: async (savedAt) => {
			setNotice(null);
			const outcome = await layouts.reopen(savedAt);
			setNotice(workbenchEarlierLayoutNotice(outcome));
			return outcome;
		},
	};
}

/**
 * Reports what the authority decided without taking the person from what they are doing: only a
 * refusal or an unknown outcome needs words, because a committed change shows in the Workbench.
 * Closing ends terminals for every Client, so it also says how it is going and how it ended: a
 * Workspace or Project close says Closed once its Workspaces leave at the Server's acceptance, and
 * one Tab's close says Closing only while it runs slow after its Tab left. Dissolving a Project
 * says that its Workspaces remain.
 */
export function useWorkbenchAuthorityNotice(
	authority: WorkbenchAuthority,
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchAuthorityNoticeView {
	const intl = useOrchbeeIntl();
	const [notice, setNotice] = React.useState<WorkbenchAuthorityNotice | null>(null);
	const reportingAuthority = React.useMemo<WorkbenchAuthority>(
		() => ({
			minimumSplitPercent: authority.minimumSplitPercent,
			// The board reports its own outcomes, so nothing it does reaches the status line.
			agentBoard: authority.agentBoard,
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
				} else if (
					change.type === 'panel-moved' ||
					change.type === 'panels-swapped' ||
					change.type === 'panel-renamed'
				) {
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
				const sawClosed = (): boolean =>
					landsAutomatically ||
					workbenchActiveLocationsEqual(
						origin,
						projectWorkspaceRuntime.getSnapshot().activeLocation,
					);
				// One Tab's close says Closing only once its Tab has left the strip and it still runs a
				// second after it began, as the TUI's does, and its end says nothing.
				const tab = subject.kind === 'tab';
				let progress: WorkbenchAuthorityNotice | null = null;
				let closingShown = false;
				let left = false;
				let closedSaid = false;
				const showProgress = (): void => {
					if (!closingShown || closedSaid || (tab && !left)) return;
					progress = workbenchCloseProgressNotice(subject, projectWorkspaceRuntime);
					setNotice(progress);
				};
				setNotice(null);
				const closingDelay = setTimeout(
					() => {
						closingShown = true;
						showProgress();
					},
					tab ? SLOW_TAB_CLOSE_MILLISECONDS : CLOSING_NOTICE_DELAY_MILLISECONDS,
				);
				const stopWatching = projectWorkspaceRuntime.subscribe(showProgress);
				// As in the TUI, a close whose Workspaces have left reads as closed while it settles, so
				// Closed shows with the landing and only a later failure or lost outcome replaces it.
				const sayClosed = (): void => {
					closedSaid = true;
					clearTimeout(closingDelay);
					const closed = workbenchCloseResultNotice(
						subject,
						{ kind: 'completed' },
						projectWorkspaceRuntime,
						authority,
						sawClosed(),
					);
					const shown = progress;
					setNotice((current) => {
						if (closed === null) return current === shown ? null : current;
						return current?.lifetime === 'until-press' ? current : closed;
					});
				};
				const markLeft = (): void => {
					if (!left) {
						left = true;
						if (tab) showProgress();
						else sayClosed();
					}
					context.markLeft?.();
				};
				try {
					const result = await authority.close(
						{ markCommitted: () => context.markCommitted(), markLeft, signal: context.signal },
						closure,
					);
					// Closed has already been said, and its time runs from then.
					if (closedSaid && result.kind === 'completed') return result;
					const next = workbenchCloseResultNotice(
						subject,
						result,
						projectWorkspaceRuntime,
						authority,
						sawClosed(),
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
				setNotice(workbenchCreationFailureNotice(creation, result));
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
			duplicateTabs: async (context, duplication) => {
				setNotice(null);
				const result = await authority.duplicateTabs(context, duplication);
				setNotice(workbenchTabCreationFailureNotice(result));
				return result;
			},
			earlierLayouts:
				authority.earlierLayouts === undefined
					? undefined
					: reportingEarlierLayouts(authority.earlierLayouts, setNotice),
			focusNewPanel: authority.focusNewPanel,
			heldRestores:
				authority.heldRestores === undefined
					? undefined
					: reportingHeldRestores(authority.heldRestores, setNotice),
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
		}),
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
	const reportAgentNavigationUnavailable = useMemoizedFn(() => {
		setNotice({
			descriptor: workbenchMessages.workspaceAgentNavigationUnavailable,
			lifetime: 'timed',
		});
	});
	const withdrawDirectoryRefusal = useMemoizedFn(() => {
		setNotice((current) =>
			current !== null && isWorkbenchDirectoryRefusalNotice(current) ? null : current,
		);
	});
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
		reportAgentNavigationUnavailable,
		reportDuplicationStopped,
		withdrawDirectoryRefusal,
	};
}
