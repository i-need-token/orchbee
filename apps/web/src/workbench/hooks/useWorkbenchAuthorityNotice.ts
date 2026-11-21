import type { ActionExecution, ActionInvocationSource } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages, type IntlShape } from '@orchbee/i18n';
import * as React from 'react';

import {
	isWorkbenchDirectoryRefusalNotice,
	readWorkbenchCloseSubject,
	workbenchActionOutcomeNotice,
	workbenchCloseProgressNotice,
	workbenchCloseResultNotice,
	workbenchCloseWithdrawnNotice,
	workbenchConfiguredCommandNotice,
	workbenchCreationFailureNotice,
	workbenchFailureNotice,
	workbenchNamedFailureNotice,
	workbenchPanelsFailureNotice,
	workbenchSplitRatioFailureNotice,
	workbenchTabCreationFailureNotice,
	workbenchTabDuplicationFailureNotice,
	workbenchTabDuplicationStoppedNotice,
	workbenchTabsFailureNotice,
	workbenchWorktreeOutcomeNotice,
	workbenchWorktreeProgressNotice,
	type WorkbenchAuthorityNotice,
} from '../model/workbench-authority-notice';
import { formatWorkbenchConfiguredCommandName } from '../model/workbench-configured-command-name';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchAgentNotResumed,
	WorkbenchAgentStartFailure,
	WorkbenchAuthority,
	WorkbenchHeldRestores,
	WorkbenchTabDuplicationStop,
} from '../types/workbench-authority';
import type { WorkbenchAgentCatalogSource } from '../types/workbench-agent-start';
import type { WorkbenchConfiguredCommands } from '../types/workbench-configured-commands';
import type { WorkbenchWorktrees } from '../types/workbench-worktrees';
import { formatWorkbenchAgentProvider } from '../utils/workbench-agent-names';

export interface WorkbenchAuthorityNoticeView {
	readonly authority: WorkbenchAuthority;
	readonly notice: { readonly message: string; readonly persistent: boolean } | null;
	/**
	 * Runs an Action invoked from a control, menu, or shortcut, and says its refusal or failure when no
	 * surface did while it ran.
	 */
	readonly reportActionOutcome: <Execution extends ActionExecution<unknown>>(
		source: ActionInvocationSource,
		start: () => Execution,
		named?: WorkbenchAuthorityNotice['descriptor'],
	) => Execution;
	/** An Agent destination failed revalidation, so nothing moved; the TUI says the same. */
	readonly reportAgentNavigationUnavailable: () => void;
	readonly reportDuplicationStopped: (stop: WorkbenchTabDuplicationStop) => void;
	/** Update could not switch or launch; the TUI says the same. */
	readonly reportUpdateUnavailable: () => void;
	/** Update could not switch yet; the TUI says the same. */
	readonly reportUpdateWaiting: () => void;
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

/** A resume the Server gave up after a restart, worded as the TUI's status line words it. */
function agentNotResumedNotice(
	intl: IntlShape,
	{ providerId, reason }: WorkbenchAgentNotResumed,
): WorkbenchAuthorityNotice {
	return {
		descriptor:
			reason === 'terminal-too-old'
				? workbenchMessages.agentResumeTerminalTooOld
				: workbenchMessages.agentResumePromptUnconfirmed,
		lifetime: 'timed',
		values: { agent: formatWorkbenchAgentProvider(intl, providerId) },
	};
}

/**
 * A start the Server found did not happen, worded as the TUI's start notice. Not found names the
 * key that opens Start Agent to choose another Agent; the shell's prompt never confirmed, or a
 * terminal too old to confirm one, reads like a typed start's own refusal.
 */
function agentStartFailureNotice(
	intl: IntlShape,
	{ providerId, reason }: WorkbenchAgentStartFailure,
	catalog: WorkbenchAgentCatalogSource | undefined,
	startAgentShortcutLabel: string | undefined,
): WorkbenchAuthorityNotice {
	const product =
		catalog?.getSnapshot()?.agents.find((agent) => agent.providerId === providerId)?.productName ??
		formatWorkbenchAgentProvider(intl, providerId);
	const values = { product };
	switch (reason) {
		case 'not-found':
			return startAgentShortcutLabel === undefined
				? { descriptor: workbenchMessages.agentStartNotFoundUnbound, lifetime: 'timed', values }
				: {
						descriptor: workbenchMessages.agentStartNotFound,
						lifetime: 'timed',
						values: { key: startAgentShortcutLabel, product },
					};
		case 'exited':
			return { descriptor: workbenchMessages.agentStartExited, lifetime: 'timed', values };
		case 'unsupported-shell':
			return {
				descriptor: workbenchMessages.agentStartUnsupportedShell,
				lifetime: 'timed',
				values,
			};
		case 'prompt-unconfirmed':
			return { descriptor: workbenchMessages.agentStartNotAtPrompt, lifetime: 'timed' };
		case 'terminal-too-old':
			return { descriptor: workbenchMessages.agentStartTerminalTooOld, lifetime: 'timed' };
	}
}

/** Whether `current` already says what `next` says, so `next` would only restart its time. */
function sameWords(
	current: WorkbenchAuthorityNotice | null,
	next: WorkbenchAuthorityNotice,
): boolean {
	if (current?.descriptor !== next.descriptor) return false;
	const values = Object.entries(next.values ?? {});
	return (
		values.length === Object.keys(current.values ?? {}).length &&
		values.every(([key, value]) => current.values?.[key] === value)
	);
}

/**
 * Reports what the authority decided without taking the person from what they are doing: only a
 * refusal or an unknown outcome needs words, because a committed change shows in the Workbench.
 * Closing ends terminals for every Client, so it also says how it is going and how it ended: a
 * Workspace or Project close says Closed once its Workspaces leave at the Server's acceptance, and
 * one Tab's close says Closing only while it runs slow after its Tab left. Dissolving a Project
 * says that its Workspaces remain. A resume the Server gave up after a restart is said once, and
 * the same words already showing keep their time; so is an Agent start a new terminal carried that
 * did not happen.
 */
export function useWorkbenchAuthorityNotice(
	authority: WorkbenchAuthority,
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
	/** Start Agent's effective shortcut, which a not-found start names; absent while it has none. */
	startAgentShortcutLabel?: string,
): WorkbenchAuthorityNoticeView {
	const intl = useOrchbeeIntl();
	const [notice, setNoticeState] = React.useState<WorkbenchAuthorityNotice | null>(null);
	const noticeRef = React.useRef<WorkbenchAuthorityNotice | null>(null);
	/** Counts each notice written, so an Action's outcome can tell whether a surface already said what happened. */
	const noticeWrites = React.useRef(0);
	const setNotice = useMemoizedFn(
		(update: React.SetStateAction<WorkbenchAuthorityNotice | null>) => {
			// Resolved here, against the latest notice, so the write counter sees each notice as it is written.
			const next = typeof update === 'function' ? update(noticeRef.current) : update;
			noticeRef.current = next;
			if (next !== null) noticeWrites.current += 1;
			setNoticeState(next);
		},
	);
	/**
	 * Runs an Action a person invoked from a control, menu, or shortcut. A refusal or failure that no
	 * surface said while the Action ran is said here, so it never reads as nothing happened.
	 */
	const reportActionOutcome = useMemoizedFn(
		<Execution extends ActionExecution<unknown>>(
			source: ActionInvocationSource,
			start: () => Execution,
			named?: WorkbenchAuthorityNotice['descriptor'],
		): Execution => {
			// Automation and programmatic calls are not a person's control, so their caller owns any outcome.
			if (source === 'automation' || source === 'programmatic') return start();
			const writesBefore = noticeWrites.current;
			const execution = start();
			const say = (outcome: WorkbenchAuthorityNotice | null): void => {
				if (outcome !== null && noticeWrites.current === writesBefore) setNotice(outcome);
			};
			void execution.completion.then(
				(result) => say(workbenchActionOutcomeNotice(result, named)),
				() => say({ descriptor: workbenchMessages.actionUnavailable, lifetime: 'until-press' }),
			);
			return execution;
		},
	);
	const reportingAuthority = React.useMemo<WorkbenchAuthority>(
		() => ({
			minimumSplitPercent: authority.minimumSplitPercent,
			agentCatalog: authority.agentCatalog,
			change: async (context, change) => {
				setNotice(null);
				const result = await authority.change(context, change);
				if (change.type === 'tab-renamed') {
					setNotice(workbenchNamedFailureNotice(result, workbenchMessages.tabsRenameNotApplied));
				} else if (change.type === 'tabs-changed' && change.change.kind === 'tab-moved') {
					setNotice(workbenchNamedFailureNotice(result, workbenchMessages.tabsMoveNotApplied));
				} else if (change.type === 'tabs-changed') {
					setNotice(workbenchTabsFailureNotice(result));
				} else if (change.type === 'panel-moved') {
					setNotice(workbenchNamedFailureNotice(result, workbenchMessages.panelsMoveNotApplied));
				} else if (change.type === 'panels-swapped' || change.type === 'panel-renamed') {
					setNotice(workbenchPanelsFailureNotice(result));
				} else if (
					change.type === 'panel-size-adjusted' ||
					change.type === 'panels-equalized' ||
					change.type === 'split-ratio-set'
				) {
					setNotice(workbenchSplitRatioFailureNotice(result));
				} else if (change.type === 'project-renamed') {
					setNotice(workbenchNamedFailureNotice(result, workbenchMessages.projectRenameNotApplied));
				} else if (change.type === 'workspace-updated') {
					setNotice(
						workbenchNamedFailureNotice(result, workbenchMessages.workspaceRenameNotApplied),
					);
				} else {
					setNotice(workbenchFailureNotice(result));
				}
				return result;
			},
			close: async (context, closure) => {
				const subject = readWorkbenchCloseSubject(closure, projectWorkspaceRuntime);
				// One Tab's close says Closing only once its Tab has left the strip and it still runs a
				// second after it began, as the TUI's does, and its end says nothing.
				const tab = subject.kind === 'tab';
				let progress: WorkbenchAuthorityNotice | null = null;
				let closingShown = false;
				let left = false;
				let settled = false;
				const showProgress = (): void => {
					if (!closingShown || settled || (tab && !left)) return;
					progress = workbenchCloseProgressNotice(subject);
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
				// As in the TUI, a close whose Workspaces have left reads as done while it settles: what it
				// said leaves with the landing, and only a later failure or lost outcome says more.
				const settle = (): void => {
					settled = true;
					clearTimeout(closingDelay);
					const shown = progress;
					setNotice((current) => (current === shown ? null : current));
				};
				const markLeft = (): void => {
					if (!left) {
						left = true;
						if (tab) showProgress();
						else settle();
					}
					context.markLeft?.();
				};
				try {
					const result = await authority.close(
						{ markCommitted: () => context.markCommitted(), markLeft, signal: context.signal },
						closure,
					);
					if (settled && result.kind === 'completed') return result;
					const next = workbenchCloseResultNotice(subject, result);
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
				setNotice(workbenchNamedFailureNotice(result, workbenchMessages.panelsSplitNotApplied));
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
				setNotice(workbenchTabDuplicationFailureNotice(result));
				return result;
			},
			heldRestores:
				authority.heldRestores === undefined
					? undefined
					: reportingHeldRestores(authority.heldRestores, setNotice),
			launchProfiles: authority.launchProfiles,
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
		[authority, intl, projectWorkspaceRuntime, setNotice],
	);
	const reportDuplicationStopped = useMemoizedFn((stop: WorkbenchTabDuplicationStop) => {
		setNotice(workbenchTabDuplicationStoppedNotice(stop));
	});
	const reportAgentNavigationUnavailable = useMemoizedFn(() => {
		setNotice({
			descriptor: workbenchMessages.workspaceAgentNavigationUnavailable,
			lifetime: 'timed',
		});
	});
	const reportUpdateUnavailable = useMemoizedFn(() => {
		setNotice({ descriptor: workbenchMessages.updateUnavailable, lifetime: 'until-press' });
	});
	const reportUpdateWaiting = useMemoizedFn(() => {
		setNotice({ descriptor: workbenchMessages.updateWaiting, lifetime: 'timed' });
	});
	const agentResumeNotices = authority.agentResumeNotices;
	React.useEffect(
		() =>
			agentResumeNotices?.subscribe((resume) => {
				const next = agentNotResumedNotice(intl, resume);
				setNotice((current) =>
					current?.lifetime === 'until-press' || sameWords(current, next) ? current : next,
				);
			}),
		[agentResumeNotices, intl, setNotice],
	);
	const agentStartNotices = authority.agentStartNotices;
	const startNoticeFor = useMemoizedFn((failure: WorkbenchAgentStartFailure) =>
		agentStartFailureNotice(intl, failure, authority.agentCatalog, startAgentShortcutLabel),
	);
	React.useEffect(
		() =>
			agentStartNotices?.subscribe((failure) => {
				const next = startNoticeFor(failure);
				setNotice((current) =>
					current?.lifetime === 'until-press' || sameWords(current, next) ? current : next,
				);
			}),
		[agentStartNotices, startNoticeFor, setNotice],
	);
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
	}, [notice, setNotice]);
	return {
		authority: reportingAuthority,
		notice:
			notice === null
				? null
				: {
						message: intl.formatMessage(notice.descriptor, notice.values),
						persistent: notice.lifetime === 'until-press',
					},
		reportActionOutcome,
		reportAgentNavigationUnavailable,
		reportDuplicationStopped,
		reportUpdateUnavailable,
		reportUpdateWaiting,
		withdrawDirectoryRefusal,
	};
}
