import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import type { WorkbenchTabDuplicationStop } from '../actions/workbench-shared-tab-creation';
import {
	readWorkbenchCloseSubject,
	workbenchCloseProgressNotice,
	workbenchCloseResultNotice,
	workbenchCloseWithdrawnNotice,
	workbenchFailureNotice,
	workbenchTabCreationFailureNotice,
	workbenchTabDuplicationStoppedNotice,
	workbenchTabsFailureNotice,
	type WorkbenchAuthorityNotice,
} from '../model/workbench-authority-notice';
import { workbenchActiveLocationsEqual } from '../model/workbench-terminal-location-activation';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority, WorkbenchAuthorityClosure } from '../types/workbench-authority';

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
						createTab: async (context, creation) => {
							setNotice(null);
							const result = await authority.createTab(context, creation);
							setNotice(workbenchTabCreationFailureNotice(result));
							return result;
						},
						watchClose: (closure, onChanged) =>
							authority.watchClose(closure, () => {
								setNotice(workbenchCloseWithdrawnNotice(closure));
								onChanged();
							}),
					},
		[authority, projectWorkspaceRuntime],
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
