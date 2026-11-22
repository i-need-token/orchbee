import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import type { WorkbenchClientUpdate } from '../types/workbench-client-update';

/** The question the window asks before a Terminal Host replacement restarts every terminal. */
export interface WorkbenchHostReplacementView {
	readonly cancel: () => void;
	readonly cancelLabel: string;
	readonly confirm: () => void;
	readonly confirmLabel: string;
	readonly description: string;
	readonly title: string;
}

/**
 * The Host replacement question while the host asks it: shown once the host's answer to an Update
 * says a Terminal Host must be replaced, answered once by Restart or Cancel.
 */
export function useWorkbenchHostReplacement(
	clientUpdate: WorkbenchClientUpdate | undefined,
): WorkbenchHostReplacementView | null {
	const intl = useOrchbeeIntl();
	const [asking, setAsking] = React.useState(false);
	React.useEffect(
		() => clientUpdate?.subscribeHostReplacement?.(() => setAsking(true)),
		[clientUpdate],
	);
	if (!asking || clientUpdate?.answerHostReplacement === undefined) return null;
	const answer = clientUpdate.answerHostReplacement;
	return {
		cancel: () => {
			setAsking(false);
			answer(false);
		},
		cancelLabel: intl.formatMessage(workbenchMessages.panelCancel),
		confirm: () => {
			setAsking(false);
			answer(true);
		},
		confirmLabel: intl.formatMessage(workbenchMessages.updateHostReplacementConfirm),
		description: intl.formatMessage(workbenchMessages.updateHostReplacementDescription),
		title: intl.formatMessage(workbenchMessages.updateHostReplacementTitle),
	};
}
