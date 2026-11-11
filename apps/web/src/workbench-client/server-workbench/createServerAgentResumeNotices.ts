import type { WorkbenchAgentNotResumed, WorkbenchAgentResumeNotices } from '../../workbench';

import {
	subscribeServerPresentationFacts,
	type ServerPresentationFactSession,
} from './subscribeServerPresentationFacts';

/**
 * The Server's one-shot facts that a resume after a restart typed nothing, as the TUI shows them:
 * each fact once, only while it is fresh, newest first. A Client that connects later, or reads a
 * snapshot the fact has long sat in, hears nothing of it.
 */
export function createServerAgentResumeNotices(
	session: ServerPresentationFactSession,
	now: () => number = Date.now,
): WorkbenchAgentResumeNotices {
	return {
		subscribe: (listener) =>
			subscribeServerPresentationFacts(
				session,
				(presentation) => presentation.agentNotResumed,
				({ providerId, reason }) => {
					const notice: WorkbenchAgentNotResumed = { providerId, reason };
					listener(notice);
				},
				{ now, since: 0 },
			),
	};
}
