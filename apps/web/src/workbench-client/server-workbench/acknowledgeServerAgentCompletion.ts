import {
	WORKBENCH_PROTOCOL_VERSION,
	type TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';

import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import { findServerPresentation } from './findServerPresentation';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

/** The Server applies an acknowledgement under one lock, so a later answer was lost. */
const REPLY_MILLISECONDS = 10_000;

/**
 * Asks the Server to mark the finished turn of the Agent in a Presentation's terminal as seen, as
 * typed input there does. Only a Session whose handshake lists `agent-completion-acknowledge`
 * takes it. The Agent's next state reports what the Server did, so nothing waits on the answer.
 */
export function acknowledgeServerAgentCompletion(
	session: Pick<ServerWorkbenchSession, 'ask' | 'snapshot'>,
	presentationId: TerminalPresentationId,
): void {
	// The terminal the person saw finish; the Server refuses any other.
	const expectedBinding = findServerPresentation(session.snapshot, presentationId)?.binding ?? null;
	if (expectedBinding === null) return;
	void session.ask(
		{
			action: { expectedBinding, kind: 'completion-acknowledge', presentationId },
			kind: 'workbench.panel-agent',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId: createWorkbenchRequestId(),
			sessionId: session.snapshot.sessionId,
		},
		REPLY_MILLISECONDS,
	);
}
