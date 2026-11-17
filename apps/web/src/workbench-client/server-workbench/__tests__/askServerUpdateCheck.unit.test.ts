import { parseWorkbenchResponse } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import { fixtureId, workbenchSnapshot } from '../../../__tests__/server-workbench-snapshot';
import { askServerUpdateCheck } from '../askServerUpdateCheck';
import type { ServerWorkbenchAnswer, ServerWorkbenchSession } from '../ServerWorkbenchSession';

type Request = Parameters<ServerWorkbenchSession['ask']>[0];

const SNAPSHOT = workbenchSnapshot(1, { workspaces: [] }, []);

/** A session whose Server answers each request through `reply`, given the request it was sent. */
function sessionAnswering(
	reply: (request: Request) => ServerWorkbenchAnswer,
): Pick<ServerWorkbenchSession, 'ask' | 'snapshot'> {
	return { ask: (request) => Promise.resolve(reply(request)), snapshot: SNAPSHOT };
}

/** The Server's answer, read through the protocol binding as the carrier reads it. */
function answered(record: Readonly<Record<string, unknown>>): ServerWorkbenchAnswer {
	return { kind: 'answered', response: parseWorkbenchResponse({ protocolVersion: 1, ...record }) };
}

function updateCheckResult(requestId: string, outcome: unknown): ServerWorkbenchAnswer {
	return answered({ kind: 'workbench.update-check.result', outcome, requestId });
}

describe('askServerUpdateCheck', () => {
	it("reads the Server's answer to its own request", async () => {
		const cases: readonly (readonly [unknown, string])[] = [
			[{ kind: 'checking' }, 'checking'],
			[{ kind: 'idle' }, 'idle'],
			[{ kind: 'unavailable' }, 'unavailable'],
			[{ kind: 'finished', result: 'up-to-date' }, 'up-to-date'],
			[{ kind: 'finished', result: 'installed' }, 'installed'],
			[{ kind: 'finished', result: 'failed' }, 'run-failed'],
			[{ kind: 'denied' }, 'not-answered'],
			[{ kind: 'rejected', reason: 'session-mismatch' }, 'not-answered'],
		];
		for (const [outcome, expected] of cases) {
			const session = sessionAnswering((request) => {
				expect(request).toMatchObject({ action: 'start', kind: 'workbench.update-check' });
				return updateCheckResult(request.requestId, outcome);
			});
			await expect(askServerUpdateCheck(session, 'start')).resolves.toBe(expected);
		}
	});

	it('ends as not answered when the answer is lost, refused, of another kind, or for another request', async () => {
		const answers: readonly ((request: Request) => ServerWorkbenchAnswer)[] = [
			() => ({ kind: 'outcome-unknown' }),
			() => ({ kind: 'not-submitted' }),
			() => updateCheckResult(fixtureId(0xbad), { kind: 'finished', result: 'up-to-date' }),
			(request) =>
				answered({
					kind: 'workbench.session-stop.result',
					outcome: { kind: 'busy' },
					requestId: request.requestId,
				}),
			() => {
				throw new Error('The connection failed.');
			},
		];
		for (const answer of answers) {
			await expect(askServerUpdateCheck(sessionAnswering(answer), 'read')).resolves.toBe(
				'not-answered',
			);
		}
	});
});
