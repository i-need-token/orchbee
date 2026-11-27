import { parseWorkbenchResponse } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import { fixtureId, workbenchSnapshot } from '../../../__tests__/server-workbench-snapshot';
import type { WorkbenchUpdateCheckAnswer } from '../../../workbench';
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
	it("reads the Server's answer to its own request, a failure by the phase it names", async () => {
		const cases: readonly (readonly [unknown, WorkbenchUpdateCheckAnswer])[] = [
			[{ kind: 'checking' }, { kind: 'checking' }],
			[{ kind: 'idle' }, { kind: 'idle' }],
			[{ kind: 'unavailable' }, { kind: 'unavailable' }],
			[
				{ kind: 'downloading', receivedBytes: 42, totalBytes: 100, version: '1.3.0' },
				{ kind: 'downloading', receivedBytes: 42, totalBytes: 100, version: '1.3.0' },
			],
			[{ kind: 'installing', version: '1.3.0' }, { kind: 'installing' }],
			[{ kind: 'finished', result: 'up-to-date' }, { kind: 'up-to-date' }],
			[{ kind: 'finished', result: 'installed' }, { kind: 'installed' }],
			// A Session without phases names no phase, so its failure reads as the check's.
			[
				{ kind: 'finished', result: 'failed' },
				{ kind: 'run-failed', phase: 'check' },
			],
			[
				{ kind: 'finished', result: 'check-failed' },
				{ kind: 'run-failed', phase: 'check' },
			],
			[
				{ kind: 'finished', result: 'download-failed' },
				{ kind: 'run-failed', phase: 'download' },
			],
			[
				{ kind: 'finished', result: 'install-failed' },
				{ kind: 'run-failed', phase: 'install' },
			],
			[{ kind: 'denied' }, { kind: 'not-answered' }],
			[{ kind: 'rejected', reason: 'session-mismatch' }, { kind: 'not-answered' }],
		];
		for (const [outcome, expected] of cases) {
			const session = sessionAnswering((request) => {
				expect(request).toMatchObject({ action: 'start', kind: 'workbench.update-check' });
				return updateCheckResult(request.requestId, outcome);
			});
			await expect(askServerUpdateCheck(session, 'start', true)).resolves.toEqual(expected);
		}
	});

	it('asks for phases only of a Session that lists them, leaving the member out otherwise', async () => {
		const sent: Request[] = [];
		const session = sessionAnswering((request) => {
			sent.push(request);
			return updateCheckResult(request.requestId, { kind: 'idle' });
		});
		await askServerUpdateCheck(session, 'read', false);
		await askServerUpdateCheck(session, 'read', true);
		expect(sent[0]).not.toHaveProperty('phases');
		expect(sent[1]).toMatchObject({ phases: true });
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
			await expect(askServerUpdateCheck(sessionAnswering(answer), 'read', true)).resolves.toEqual({
				kind: 'not-answered',
			});
		}
	});
});
