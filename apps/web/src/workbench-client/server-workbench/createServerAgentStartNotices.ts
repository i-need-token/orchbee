import type { WorkbenchAgentStartNotices } from '../../workbench';

import {
	subscribeServerPresentationFacts,
	type ServerPresentationFactSession,
} from './subscribeServerPresentationFacts';

/**
 * The Server's one-shot facts that the Agent start a new Tab, split or Worktree carried did not
 * happen, as the TUI shows them: each fact once, only while it is fresh, newest first. A fact from
 * before this Client began listening is old, so a window that attaches never repeats one.
 */
export function createServerAgentStartNotices(
	session: ServerPresentationFactSession,
	now: () => number = Date.now,
): WorkbenchAgentStartNotices {
	return {
		subscribe: (listener) =>
			subscribeServerPresentationFacts(
				session,
				(presentation) => presentation.agentStartFailure,
				(fact) => listener({ providerId: fact.providerId, reason: fact.reason }),
				{ now, since: now() },
			),
	};
}
