import { parseWorkbenchResponse } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import { fixtureId, oneWorkspaceSnapshot } from '../../../__tests__/server-workbench-snapshot';
import { createServerWorkspaceFiles } from '../createServerWorkspaceFiles';
import type { ServerWorkbenchAnswer, ServerWorkbenchSession } from '../ServerWorkbenchSession';

type Request = Parameters<ServerWorkbenchSession['ask']>[0];

const SNAPSHOT = oneWorkspaceSnapshot(1, []);
const WORKSPACE = SNAPSHOT.navigation.workspaces[0]?.workspaceId ?? '';

function sessionAnswering(
	reply: (request: Request) => ServerWorkbenchAnswer,
): Pick<ServerWorkbenchSession, 'ask' | 'snapshot'> {
	return { ask: (request) => Promise.resolve(reply(request)), snapshot: SNAPSHOT };
}

function filesResult(requestId: string, outcome: unknown): ServerWorkbenchAnswer {
	return {
		kind: 'answered',
		response: parseWorkbenchResponse({
			kind: 'workbench.workspace-files.result',
			outcome,
			protocolVersion: 1,
			requestId,
			sessionId: SNAPSHOT.sessionId,
			workspaceId: WORKSPACE,
		}),
	};
}

describe('createServerWorkspaceFiles', () => {
	it("asks for one read in the Session's Workspace and returns the Server's outcome", async () => {
		const outcome = {
			entries: [{ entryKind: 'file', ignored: false, name: 'a.txt', symbolicLink: false }],
			kind: 'directory',
			truncated: false,
		};
		const requests: Request[] = [];
		const source = createServerWorkspaceFiles(
			sessionAnswering((request) => {
				requests.push(request);
				return filesResult(request.requestId, outcome);
			}),
		);
		await expect(source.read(WORKSPACE, { kind: 'list-directory', path: '' })).resolves.toEqual(
			outcome,
		);
		expect(requests).toEqual([
			expect.objectContaining({
				kind: 'workbench.workspace-files',
				operation: { kind: 'list-directory', path: '' },
				sessionId: SNAPSHOT.sessionId,
				workspaceId: WORKSPACE,
			}),
		]);
	});

	it('reads as no answer when the answer is lost, of another kind, or for another request', async () => {
		const answers: readonly ((request: Request) => ServerWorkbenchAnswer)[] = [
			() => ({ kind: 'outcome-unknown' }),
			() => ({ kind: 'not-submitted' }),
			() => filesResult(fixtureId(0xbad), { kind: 'failed', reason: 'timeout' }),
			(request) => ({
				kind: 'answered',
				response: parseWorkbenchResponse({
					kind: 'workbench.session-stop.result',
					outcome: { kind: 'busy' },
					protocolVersion: 1,
					requestId: request.requestId,
				}),
			}),
		];
		for (const answer of answers) {
			const source = createServerWorkspaceFiles(sessionAnswering(answer));
			await expect(source.read(WORKSPACE, { kind: 'git-status' })).resolves.toBeNull();
		}
	});

	it('asks nothing about a Workspace the Session does not show', async () => {
		const requests: Request[] = [];
		const source = createServerWorkspaceFiles(
			sessionAnswering((request) => {
				requests.push(request);
				return { kind: 'not-submitted' };
			}),
		);
		await expect(source.read(fixtureId(0x99), { kind: 'git-status' })).resolves.toEqual({
			kind: 'failed',
			reason: 'not-found',
		});
		expect(requests).toEqual([]);
	});
});
