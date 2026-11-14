import {
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchStopIdSchema,
} from '@orchbee/workbench-client-protocol';

import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

/**
 * What became of a Session stop: `recorded` when the Server holds it and runs it once, `unknown`
 * when it may have been recorded, and `not-sent` when nothing reached the Server. `busy`,
 * `unmanaged`, and `refused` recorded nothing.
 */
type ServerSessionStopAnswer =
	| 'busy'
	| 'not-sent'
	| 'recorded'
	| 'refused'
	| 'unknown'
	| 'unmanaged';

type SessionStopSession = Pick<ServerWorkbenchSession, 'ask' | 'snapshot'>;

/** Recording a stop takes one lock; the Server answers before the stop begins. */
const REPLY_MILLISECONDS = 10_000;

/**
 * Asks the Server to stop this Client's Session under `stopId`, as `server stop` does. It is sent
 * once: the recorded stop refuses every later request on this connection, so `server status`
 * reads its progress, and an answer that never arrives is never repeated.
 */
export async function stopServerSession(
	session: SessionStopSession,
	stopId: string,
): Promise<ServerSessionStopAnswer> {
	const id = WorkbenchStopIdSchema.safeParse(stopId);
	if (!id.success) return 'not-sent';
	const answer = await session.ask(
		{
			kind: 'workbench.session-stop',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId: createWorkbenchRequestId(),
			sessionId: session.snapshot.sessionId,
			stopId: id.data,
		},
		REPLY_MILLISECONDS,
	);
	if (answer.kind === 'not-submitted') return 'not-sent';
	if (
		answer.kind === 'outcome-unknown' ||
		answer.response.kind !== 'workbench.session-stop.result'
	) {
		return 'unknown';
	}
	switch (answer.response.outcome.kind) {
		case 'recorded':
		case 'busy':
		case 'unmanaged':
			return answer.response.outcome.kind;
		case 'denied':
		case 'rejected':
			return 'refused';
	}
}
