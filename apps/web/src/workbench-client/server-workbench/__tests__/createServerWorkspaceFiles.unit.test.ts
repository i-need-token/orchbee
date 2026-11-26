import { parseWorkbenchResponse } from '@orchbee/workbench-client-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { fixtureId, oneWorkspaceSnapshot } from '../../../__tests__/server-workbench-snapshot';
import { createServerWorkspaceFiles } from '../createServerWorkspaceFiles';
import type { ServerConnection, ServerSubscription } from '../../server-connection';
import type { ServerWorkbenchAnswer, ServerWorkbenchSession } from '../ServerWorkbenchSession';

type Request = Parameters<ServerWorkbenchSession['ask']>[0];

const SNAPSHOT = oneWorkspaceSnapshot(1, []);
const WORKSPACE = SNAPSHOT.navigation.workspaces[0]?.workspaceId ?? '';

function sessionAnswering(
	reply: (request: Request) => ServerWorkbenchAnswer,
): Pick<ServerWorkbenchSession, 'ask' | 'snapshot'> {
	return { ask: (request) => Promise.resolve(reply(request)), snapshot: SNAPSHOT };
}

type Handlers = Parameters<ServerConnection['subscribe']>[2];

interface Opened {
	readonly request: { readonly requestId: string; readonly operation: unknown };
	readonly handlers: Handlers;
	closed: boolean;
}

/** A connection whose State streams the test answers by hand. */
function streams(): {
	readonly connection: Pick<ServerConnection, 'subscribe'>;
	readonly opened: Opened[];
} {
	const opened: Opened[] = [];
	return {
		connection: {
			subscribe: (_mode, request, handlers): ServerSubscription => {
				const stream: Opened = {
					closed: false,
					handlers,
					request: JSON.parse(new TextDecoder().decode(request.subarray(12))) as Opened['request'],
				};
				opened.push(stream);
				return {
					close: () => {
						stream.closed = true;
					},
					resnapshot: () => undefined,
				};
			},
		},
		opened,
	};
}

/** A `workbench.workspace-files.result` frame (kind 84) answering a stream. */
function watching(
	stream: Opened | undefined,
	files: string,
	repository: string,
	state = 'watching',
): void {
	const payload = new TextEncoder().encode(
		JSON.stringify({
			kind: 'workbench.workspace-files.result',
			outcome: { files, kind: 'watching', repository, state },
			protocolVersion: 1,
			requestId: stream?.request.requestId,
			sessionId: SNAPSHOT.sessionId,
			workspaceId: WORKSPACE,
		}),
	);
	const record = new Uint8Array(12 + payload.byteLength);
	const view = new DataView(record.buffer);
	record.set([0x4f, 0x42, 0x57, 0x50]);
	view.setUint16(4, 1);
	view.setUint16(6, 84);
	view.setUint32(8, payload.byteLength);
	record.set(payload, 12);
	stream?.handlers.onFrame('update', record);
}

const NO_STREAMS = streams().connection;

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
			NO_STREAMS,
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
			const source = createServerWorkspaceFiles(sessionAnswering(answer), NO_STREAMS);
			await expect(source.read(WORKSPACE, { kind: 'git-status' })).resolves.toBeNull();
		}
	});

	it('reads as no answer when a file chunk is not the one asked for', async () => {
		const chunk = (offset: number) => ({
			byteLength: 10,
			content: { kind: 'text', nextOffset: offset === 0 ? 5 : null, offset, text: 'abcde' },
			kind: 'file',
			version: null,
		});
		const read = {
			encoding: 'text',
			kind: 'read-file',
			offset: 5,
			path: 'a.txt',
			source: 'worktree',
		} as const;
		const elsewhere = createServerWorkspaceFiles(
			sessionAnswering((request) => filesResult(request.requestId, chunk(0))),
			NO_STREAMS,
		);
		await expect(elsewhere.read(WORKSPACE, read)).resolves.toBeNull();
		const asked = createServerWorkspaceFiles(
			sessionAnswering((request) => filesResult(request.requestId, chunk(5))),
			NO_STREAMS,
		);
		await expect(asked.read(WORKSPACE, read)).resolves.toEqual(chunk(5));
		await expect(asked.read(WORKSPACE, { ...read, encoding: 'bytes' })).resolves.toBeNull();
	});

	it('asks nothing about a Workspace the Session does not show', async () => {
		const requests: Request[] = [];
		const source = createServerWorkspaceFiles(
			sessionAnswering((request) => {
				requests.push(request);
				return { kind: 'not-submitted' };
			}),
			NO_STREAMS,
		);
		await expect(source.read(fixtureId(0x99), { kind: 'git-status' })).resolves.toEqual({
			kind: 'failed',
			reason: 'not-found',
		});
		expect(requests).toEqual([]);
	});
});

describe('createServerWorkspaceFiles watch', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	const SCOPE = { directories: [''], files: ['a.txt'] };

	it('reports the state first, then only the revisions that moved', () => {
		const { connection, opened } = streams();
		const reports: unknown[] = [];
		createServerWorkspaceFiles(
			sessionAnswering(() => ({ kind: 'not-submitted' })),
			connection,
		).watch(WORKSPACE, SCOPE, (report) => reports.push(report));
		expect(opened[0]?.request.operation).toEqual({
			directories: [''],
			files: ['a.txt'],
			kind: 'watch',
		});
		watching(opened[0], '1', '1', 'starting');
		watching(opened[0], '1', '1');
		watching(opened[0], '2', '1');
		watching(opened[0], '2', '3');
		expect(reports).toEqual([
			{ files: false, repository: false, state: 'starting' },
			{ files: false, repository: false, state: 'watching' },
			{ files: true, repository: false, state: 'watching' },
			{ files: false, repository: true, state: 'watching' },
		]);
	});

	it('opens a new scope before closing the old one and drops what the old one says', () => {
		const { connection, opened } = streams();
		const reports: unknown[] = [];
		const watch = createServerWorkspaceFiles(
			sessionAnswering(() => ({ kind: 'not-submitted' })),
			connection,
		).watch(WORKSPACE, SCOPE, (report) => reports.push(report));
		watching(opened[0], '1', '1');
		watch.update(SCOPE);
		expect(opened).toHaveLength(1);
		watch.update({ directories: ['', 'src'], files: ['a.txt'] });
		expect(opened).toHaveLength(2);
		expect(opened[0]?.closed).toBe(true);
		watching(opened[0], '9', '9');
		watching(opened[1], '1', '1');
		expect(reports).toEqual([{ files: false, repository: false, state: 'watching' }]);
		watch.close();
		expect(opened[1]?.closed).toBe(true);
	});

	it('reports a closed stream as unavailable and compares the reopened one with the last frame', () => {
		vi.useFakeTimers();
		const { connection, opened } = streams();
		const reports: unknown[] = [];
		createServerWorkspaceFiles(
			sessionAnswering(() => ({ kind: 'not-submitted' })),
			connection,
		).watch(WORKSPACE, SCOPE, (report) => reports.push(report));
		watching(opened[0], '4', '4');
		opened[0]?.handlers.onClosed('cancelled');
		expect(reports.at(-1)).toEqual({ files: false, repository: false, state: 'unavailable' });
		expect(opened).toHaveLength(1);
		vi.advanceTimersByTime(2000);
		expect(opened).toHaveLength(2);
		watching(opened[1], '5', '4');
		expect(reports.at(-1)).toEqual({ files: true, repository: false, state: 'watching' });
	});
});
