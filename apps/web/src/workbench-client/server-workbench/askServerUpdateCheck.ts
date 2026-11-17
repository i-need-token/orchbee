import { WORKBENCH_PROTOCOL_VERSION } from '@orchbee/workbench-client-protocol';

import type { WorkbenchUpdateCheckAnswer } from '../../workbench';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

type UpdateCheckSession = Pick<ServerWorkbenchSession, 'ask' | 'snapshot'>;

/**
 * The Server answers under one lock and never waits for a run, so a late answer means the request
 * was lost behind this connection's other requests or with the connection.
 */
const REPLY_MILLISECONDS = 10_000;

/**
 * One `workbench.update-check` exchange: `start` queues a run of the Session Server's automatic-update
 * job unless one is queued or running, and `read` reads where it stands. The carrier strict-parses
 * the answer against the protocol binding; an answer of another kind or for another request, a
 * refusal, and an answer that is lost or late all come to `not-answered`, and none is repeated.
 */
export async function askServerUpdateCheck(
	session: UpdateCheckSession,
	action: 'read' | 'start',
): Promise<WorkbenchUpdateCheckAnswer> {
	const requestId = createWorkbenchRequestId();
	let answer: Awaited<ReturnType<UpdateCheckSession['ask']>>;
	try {
		answer = await session.ask(
			{
				kind: 'workbench.update-check',
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId,
				sessionId: session.snapshot.sessionId,
				action,
			},
			REPLY_MILLISECONDS,
		);
	} catch {
		return 'not-answered';
	}
	if (answer.kind !== 'answered') return 'not-answered';
	const response = answer.response;
	if (response.kind !== 'workbench.update-check.result' || response.requestId !== requestId) {
		return 'not-answered';
	}
	const outcome = response.outcome;
	switch (outcome.kind) {
		case 'checking':
		case 'idle':
		case 'unavailable':
			return outcome.kind;
		case 'finished':
			return outcome.result === 'failed' ? 'run-failed' : outcome.result;
		case 'denied':
		case 'rejected':
			return 'not-answered';
	}
}
