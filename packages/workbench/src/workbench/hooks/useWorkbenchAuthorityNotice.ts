import type { ActionResult } from '@orchbee/actions';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority } from '../types/workbench-authority';

type NoticeDescriptor = (typeof workbenchMessages)[
	| 'workspaceSetupNotApplied'
	| 'workspaceSetupOutcomeUnknown'
	| 'workspaceSetupProjectDissolved'
	| 'workspaceSetupProjectDissolvedEmpty'
	| 'workspaceSetupStale'];

interface WorkbenchAuthorityNotice {
	readonly descriptor: NoticeDescriptor;
	/** A failure stays until the person's next press, because they have to act on it. */
	readonly persistent: boolean;
}

export interface WorkbenchAuthorityNoticeView {
	readonly authority: WorkbenchAuthority | undefined;
	readonly notice: { readonly message: string; readonly persistent: boolean } | null;
}

/** The same lifetime a TUI status line gives a transient notice. */
const TRANSIENT_NOTICE_MILLISECONDS = 4_000;

function failureNotice(result: ActionResult<unknown>): WorkbenchAuthorityNotice | null {
	switch (result.kind) {
		case 'blocked':
			return {
				descriptor:
					result.code === 'revision-mismatch'
						? workbenchMessages.workspaceSetupStale
						: workbenchMessages.workspaceSetupNotApplied,
				persistent: true,
			};
		case 'failed':
			return {
				descriptor:
					result.failure.code === 'outcome-unknown'
						? workbenchMessages.workspaceSetupOutcomeUnknown
						: workbenchMessages.workspaceSetupNotApplied,
				persistent: true,
			};
		case 'cancelled':
		case 'completed':
		case 'unchanged':
		case 'unsupported':
			return null;
	}
}

/**
 * Reports what the authority decided without taking the person from what they are doing: only a
 * refusal or an unknown outcome needs words, because a committed change shows in the Workbench.
 * Dissolving a Project is the exception, since people need to hear that its Workspaces remain.
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
									persistent: false,
								});
							} else {
								setNotice(failureNotice(result));
							}
							return result;
						},
						create: async (context, creation) => {
							setNotice(null);
							const result = await authority.create(context, creation);
							setNotice(failureNotice(result));
							return result;
						},
					},
		[authority, projectWorkspaceRuntime],
	);
	React.useEffect(() => {
		if (notice === null) return undefined;
		if (!notice.persistent) {
			const expiry = setTimeout(() => setNotice(null), TRANSIENT_NOTICE_MILLISECONDS);
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
				: { message: intl.formatMessage(notice.descriptor), persistent: notice.persistent },
	};
}
