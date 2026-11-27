import { WORKBENCH_PROTOCOL_VERSION } from '@orchbee/workbench-client-protocol';

import type { WorkbenchUpdateCheckAnswer, WorkbenchUpdatePhase } from '../../workbench';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

type UpdateCheckSession = Pick<ServerWorkbenchSession, 'ask' | 'snapshot'>;

/**
 * The Server answers under one lock and never waits for a run, so a late answer means the request
 * was lost behind this connection's other requests or with the connection.
 */
const REPLY_MILLISECONDS = 10_000;

const NOT_ANSWERED: WorkbenchUpdateCheckAnswer = { kind: 'not-answered' };

/** The phase each failed result names; a Session without phases reports every failure as `failed`. */
const FAILED_PHASES = {
	'check-failed': 'check',
	'download-failed': 'download',
	failed: 'check',
	'install-failed': 'install',
} as const satisfies Readonly<Record<string, WorkbenchUpdatePhase>>;

/**
 * One `workbench.update-check` exchange: `start` queues a run of the Session Server's automatic-update
 * job unless one is queued or running, and `read` reads where it stands. With `phases`, which only a
 * Session that lists `update-phases` may receive, a running run answers its download or install and
 * a failure names its phase. The carrier strict-parses the answer against the protocol binding; an
 * answer of another kind or for another request, a refusal, and an answer that is lost or late all
 * come to `not-answered`, and none is repeated.
 */
export async function askServerUpdateCheck(
	session: UpdateCheckSession,
	action: 'read' | 'start',
	phases: boolean,
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
				// An older Session refuses any member it does not know, so the flag is left out
				// rather than sent false.
				...(phases ? { phases: true } : {}),
			},
			REPLY_MILLISECONDS,
		);
	} catch {
		return NOT_ANSWERED;
	}
	if (answer.kind !== 'answered') return NOT_ANSWERED;
	const response = answer.response;
	if (response.kind !== 'workbench.update-check.result' || response.requestId !== requestId) {
		return NOT_ANSWERED;
	}
	const outcome = response.outcome;
	switch (outcome.kind) {
		case 'checking':
		case 'idle':
		case 'installing':
		case 'unavailable':
			return { kind: outcome.kind };
		case 'downloading':
			return {
				kind: 'downloading',
				receivedBytes: outcome.receivedBytes,
				totalBytes: outcome.totalBytes,
				version: outcome.version,
			};
		case 'finished':
			return outcome.result === 'installed' || outcome.result === 'up-to-date'
				? { kind: outcome.result }
				: { kind: 'run-failed', phase: FAILED_PHASES[outcome.result] };
		case 'denied':
		case 'rejected':
			return NOT_ANSWERED;
	}
}
