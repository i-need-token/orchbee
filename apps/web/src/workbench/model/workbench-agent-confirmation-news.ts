import { workbenchMessages } from '@orchbee/i18n';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type {
	WorkbenchAgentConfirmation,
	WorkbenchAgentConfirmationPress,
	WorkbenchAgentConfirmationWait,
} from '../types/workbench-agent-confirmations';
import type { WorkbenchAuthorityNotice } from './workbench-authority-notice';

/** Why a confirmation waits for the person, in the TUI's words. */
export const WORKBENCH_AGENT_CONFIRMATION_WAITS = {
	changing: workbenchMessages.agentConfirmationWaitChanging,
	'home-workspace': workbenchMessages.agentConfirmationWaitHomeWorkspace,
	'host-too-old': workbenchMessages.agentConfirmationWaitHostTooOld,
	'leaves-workspace': workbenchMessages.agentConfirmationWaitLeavesWorkspace,
	'marked-dangerous': workbenchMessages.agentConfirmationWaitMarkedDangerous,
	'not-answered': workbenchMessages.agentConfirmationWaitNotAnswered,
	'not-understood': workbenchMessages.agentConfirmationWaitNotUnderstood,
	'not-visible': workbenchMessages.agentConfirmationWaitNotVisible,
	person: workbenchMessages.agentConfirmationWaitPerson,
	'protected-file': workbenchMessages.agentConfirmationWaitProtectedFile,
	repeating: workbenchMessages.agentConfirmationWaitRepeating,
} as const satisfies Record<WorkbenchAgentConfirmationWait, WorkbenchAuthorityNotice['descriptor']>;

const ALLOWED = {
	command: workbenchMessages.agentConfirmationAllowedCommand,
	create: workbenchMessages.agentConfirmationAllowedCreate,
	edit: workbenchMessages.agentConfirmationAllowedEdit,
} as const satisfies Record<
	WorkbenchAgentConfirmationPress['kind'],
	WorkbenchAuthorityNotice['descriptor']
>;

/** Whether `notice` says an answer Orchbee gave, which a newer answer may replace. */
export function isWorkbenchAgentConfirmationAllowedNotice(
	notice: WorkbenchAuthorityNotice,
): boolean {
	return Object.values<WorkbenchAuthorityNotice['descriptor']>(ALLOWED).includes(notice.descriptor);
}

function allowedNotice(pressed: WorkbenchAgentConfirmationPress): WorkbenchAuthorityNotice {
	return {
		descriptor: ALLOWED[pressed.kind],
		lifetime: 'timed',
		values: pressed.kind === 'command' ? { command: pressed.text } : { path: pressed.text },
	};
}

/**
 * Reads each snapshot of the Server's rows and returns what is news in it, as the TUI decides: an
 * answer Orchbee gave is said once, only for a terminal the person can see, and never when it was
 * given before this Client first read that terminal's row; answering that stopped is said once,
 * wherever its terminal is. One snapshot yields one notice, its last.
 */
export function createWorkbenchAgentConfirmationNews(): (
	snapshot: ReadonlyMap<TerminalPresentationId, WorkbenchAgentConfirmation>,
	inView: (presentationId: TerminalPresentationId) => boolean,
) => WorkbenchAuthorityNotice | null {
	const said = new Map<TerminalPresentationId, { serial: number; stopped: boolean }>();
	return (snapshot, inView) => {
		// A row that left is another Agent's when it returns.
		for (const presentationId of said.keys()) {
			if (!snapshot.has(presentationId)) said.delete(presentationId);
		}
		let news: WorkbenchAuthorityNotice | null = null;
		for (const [presentationId, confirmation] of snapshot) {
			const serial = confirmation.pressed?.serial ?? 0;
			const prior = said.get(presentationId);
			said.set(presentationId, { serial, stopped: confirmation.stopped });
			if (prior === undefined) continue;
			if (confirmation.stopped && !prior.stopped) {
				news = { descriptor: workbenchMessages.agentConfirmationStopped, lifetime: 'timed' };
			} else if (
				// A lower count is another run's, whose earlier answers this Client never said.
				serial > prior.serial &&
				confirmation.pressed !== null &&
				inView(presentationId)
			) {
				news = allowedNotice(confirmation.pressed);
			}
		}
		return news;
	};
}
