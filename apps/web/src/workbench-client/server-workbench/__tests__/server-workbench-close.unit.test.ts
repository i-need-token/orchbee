import type { WorkbenchAuthorityClosure, WorkbenchAuthorityContext } from '@orchbee/workbench';
import { parseWorkbenchResponse, type WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { closeServerWorkbench, watchServerWorkbenchClose } from '../server-workbench-close';
import type {
	ServerWorkbenchAction,
	ServerWorkbenchAnswer,
	ServerWorkbenchSubmission,
} from '../ServerWorkbenchSession';

type CloseSession = Parameters<typeof closeServerWorkbench>[0];

const SESSION = '11111111111111111111111111111111';
const WORKSPACE = '22222222222222222222222222222222';
const PROJECT = '33333333333333333333333333333333';
const CONTEXT = '44444444444444444444444444444444';
const FIRST_TAB = '55555555555555555555555555555555';
const SECOND_TAB = '66666666666666666666666666666666';
const PANEL = '77777777777777777777777777777777';
const PRESENTATION = '88888888888888888888888888888888';
const SECOND_PANEL = '99999999999999999999999999999999';
const SECOND_PRESENTATION = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const CLOSE_REQUEST = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
const REQUEST = 'cccccccccccccccccccccccccccccccc';
const OTHER_CLOSE_REQUEST = 'dddddddddddddddddddddddddddddddd';

const CLOSE_WORKSPACE: WorkbenchAuthorityClosure = {
	type: 'workspace-close-if-idle',
	workspaceId: WORKSPACE,
};
const CLOSE_TABS: WorkbenchAuthorityClosure = {
	tabIds: [FIRST_TAB, SECOND_TAB],
	terminalContextId: CONTEXT,
	type: 'tabs-close',
};
const CLOSE_PROJECT: WorkbenchAuthorityClosure = {
	projectId: PROJECT,
	type: 'project-workspaces-close',
};

function readSnapshot({
	revision = '5',
	secondTabLabel = 'Second',
	workspaceLabel = 'repo',
} = {}): WorkbenchSnapshot {
	const tab = (tabId: string, label: string, panelId: string, presentationId: string) => ({
		groupId: null,
		label,
		pinned: false,
		root: {
			childPresentations: [],
			kind: 'panel',
			panelId,
			primaryPresentation: {
				binding: null,
				customLabel: null,
				label: 'Terminal',
				presentationId,
			},
		},
		tabId,
	});
	const response = parseWorkbenchResponse({
		kind: 'workbench.snapshot.result',
		outcome: {
			kind: 'available',
			snapshot: {
				kind: 'workbench.snapshot',
				navigation: {
					directories: [],
					projects: [{ label: 'Project', projectId: PROJECT }],
					workspaces: [
						{
							label: workspaceLabel,
							primaryDirectoryId: null,
							projectId: PROJECT,
							terminalContextId: CONTEXT,
							workspaceId: WORKSPACE,
						},
					],
				},
				protocolVersion: 1,
				revision,
				sessionId: SESSION,
				terminalContexts: [
					{
						groups: [],
						tabs: [
							tab(FIRST_TAB, 'First', PANEL, PRESENTATION),
							tab(SECOND_TAB, secondTabLabel, SECOND_PANEL, SECOND_PRESENTATION),
						],
						terminalContextId: CONTEXT,
					},
				],
			},
		},
		protocolVersion: 1,
		requestId: REQUEST,
	});
	if (response.kind !== 'workbench.snapshot.result' || response.outcome.kind !== 'available') {
		throw new Error('The snapshot did not parse as available.');
	}
	return response.outcome.snapshot;
}

function settled(outcome: unknown): ServerWorkbenchSubmission {
	const response = parseWorkbenchResponse({
		kind: 'workbench.command.result',
		outcome,
		protocolVersion: 1,
		requestId: REQUEST,
	});
	if (response.kind !== 'workbench.command.result') {
		throw new Error('The outcome did not parse as a command result.');
	}
	return { kind: 'settled', outcome: response.outcome };
}

const STALE = settled({ kind: 'rejected', reason: 'revision-mismatch', revision: '6' });
const NEEDS_CONFIRMATION = settled({
	kind: 'rejected',
	reason: 'confirmation-required',
	revision: '6',
});
const WORKSPACE_ACCEPTED = settled({
	acceptedRevision: '5',
	closeRequestId: CLOSE_REQUEST,
	kind: 'workspace-close-accepted',
	terminalContextId: CONTEXT,
	workspaceId: WORKSPACE,
});
const FIRST_TAB_ACCEPTED = settled({
	acceptedRevision: '5',
	closeRequestId: CLOSE_REQUEST,
	kind: 'tab-close-accepted',
	tabId: FIRST_TAB,
	terminalContextId: CONTEXT,
});

function closeStatus(status: unknown): ServerWorkbenchAnswer {
	return {
		kind: 'answered',
		response: parseWorkbenchResponse({
			kind: 'workbench.close-operation.result',
			operationId: CLOSE_REQUEST,
			outcome: {
				generation: '1',
				kind: 'available',
				status,
				target: { kind: 'workspace', workspaceId: WORKSPACE },
			},
			protocolVersion: 1,
			requestId: REQUEST,
			sessionId: SESSION,
		}),
	};
}

const CLOSING = closeStatus({ kind: 'closing', phase: 'queued' });
const CLOSED = closeStatus({ kind: 'closed', revision: '7' });

function groupStatus(
	status: unknown,
	members: readonly { readonly revision: string | null; readonly requestId: string }[],
): ServerWorkbenchAnswer {
	return {
		kind: 'answered',
		response: parseWorkbenchResponse({
			kind: 'workbench.workspace-group-close.result',
			operationId: REQUEST,
			outcome: {
				attempt: 1,
				kind: 'available',
				members: members.map(({ requestId, revision }, index) => ({
					close:
						revision === null
							? null
							: {
									observation: {
										attemptGeneration: '1',
										status: { kind: 'closed', revision },
									},
									requestId,
								},
					label: `Member ${index}`,
					workspaceId: index === 0 ? WORKSPACE : SECOND_TAB,
				})),
				projectId: PROJECT,
				status,
			},
			protocolVersion: 1,
			requestId: REQUEST,
		}),
	};
}

interface Step {
	readonly answer: ServerWorkbenchSubmission;
	/** What the State stream had delivered by the time the answer settled. */
	readonly delivered?: WorkbenchSnapshot;
}

function fakeSession(
	steps: readonly (ServerWorkbenchSubmission | Step)[],
	answers: readonly ServerWorkbenchAnswer[] = [],
) {
	const actions: ServerWorkbenchAction[] = [];
	const asked: Parameters<CloseSession['ask']>[0][] = [];
	const published: string[] = [];
	const remainingAnswers = [...answers];
	const remainingSteps = [...steps];
	const session: { snapshot: WorkbenchSnapshot } & Omit<CloseSession, 'snapshot'> = {
		ask: (request) => {
			asked.push(request);
			const answer = remainingAnswers.shift();
			if (answer === undefined) throw new Error('Nothing more should be read.');
			return Promise.resolve(answer);
		},
		published: (revision) => {
			published.push(revision);
			return Promise.resolve();
		},
		snapshot: readSnapshot(),
		submit: (decide) => {
			const action = decide(session.snapshot);
			if (action === null) return Promise.resolve({ kind: 'not-submitted' });
			actions.push(action);
			const step = remainingSteps.shift();
			if (step === undefined) throw new Error('Nothing more should be submitted.');
			if ('answer' in step) {
				if (step.delivered !== undefined) session.snapshot = step.delivered;
				return Promise.resolve(step.answer);
			}
			return Promise.resolve(step);
		},
		subscribe: () => () => undefined,
	};
	return { actions, asked, published, session };
}

function executionContext(signal = new AbortController().signal) {
	const markCommitted = vi.fn<WorkbenchAuthorityContext['markCommitted']>();
	const context: WorkbenchAuthorityContext = { markCommitted, signal };
	return { context, markCommitted };
}

afterEach(() => {
	vi.useRealTimers();
});

describe('closeServerWorkbench', () => {
	it('waits for the removal of an accepted Workspace to reach this Client', async () => {
		const { actions, published, session } = fakeSession([WORKSPACE_ACCEPTED], [CLOSING, CLOSED]);
		const { context, markCommitted } = executionContext();

		await expect(closeServerWorkbench(session, context, CLOSE_WORKSPACE)).resolves.toEqual({
			kind: 'completed',
		});

		expect(actions).toEqual([{ kind: 'workspace.close-if-idle', workspaceId: WORKSPACE }]);
		expect(published).toEqual(['7']);
		expect(markCommitted).toHaveBeenCalled();
	});

	it('repeats a close the Server turned away for an unrelated change, but not forever', async () => {
		const { actions, session } = fakeSession([STALE, STALE, STALE, STALE, STALE]);

		await expect(
			closeServerWorkbench(session, executionContext().context, CLOSE_WORKSPACE),
		).resolves.toEqual({ code: 'scope-changed', kind: 'blocked' });

		expect(actions).toHaveLength(5);
	});

	it('does not repeat a close once what it closes has changed', async () => {
		const { actions, session } = fakeSession([
			{ answer: STALE, delivered:readSnapshot({ workspaceLabel: 'renamed' }) },
		]);

		await expect(
			closeServerWorkbench(session, executionContext().context, CLOSE_WORKSPACE),
		).resolves.toEqual({ code: 'scope-changed', kind: 'blocked' });

		expect(actions).toHaveLength(1);
	});

	it('asks for confirmation when the Workspace is still what was shown', async () => {
		const { actions, session } = fakeSession([
			{ answer: NEEDS_CONFIRMATION, delivered:readSnapshot({ revision: '6' }) },
		]);

		await expect(
			closeServerWorkbench(session, executionContext().context, CLOSE_WORKSPACE),
		).resolves.toEqual({ code: 'confirmation-required', kind: 'blocked' });

		expect(actions).toHaveLength(1);
	});

	it('takes one fresh look when the refusal described a Workspace that has moved on', async () => {
		const moved = readSnapshot({ workspaceLabel: 'renamed' });
		const { actions, session } = fakeSession([
			{ answer: NEEDS_CONFIRMATION, delivered:moved },
			{ answer: NEEDS_CONFIRMATION, delivered:moved },
		]);

		await expect(
			closeServerWorkbench(session, executionContext().context, CLOSE_WORKSPACE),
		).resolves.toEqual({ code: 'confirmation-required', kind: 'blocked' });
		expect(actions).toHaveLength(2);

		const again = fakeSession([
			{ answer: NEEDS_CONFIRMATION, delivered:moved },
			{ answer: NEEDS_CONFIRMATION, delivered:readSnapshot({ workspaceLabel: 'renamed again' }) },
		]);
		await expect(
			closeServerWorkbench(again.session, executionContext().context, CLOSE_WORKSPACE),
		).resolves.toEqual({ code: 'scope-changed', kind: 'blocked' });
		expect(again.actions).toHaveLength(2);
	});

	it('never repeats a close that may have run', async () => {
		const { actions, session } = fakeSession([{ kind: 'outcome-unknown' }]);

		await expect(
			closeServerWorkbench(session, executionContext().context, CLOSE_WORKSPACE),
		).resolves.toEqual({ failure: { code: 'outcome-unknown' }, kind: 'failed' });

		expect(actions).toHaveLength(1);
	});

	it.each<[string, unknown, string]>([
		[
			'blocked cleanup',
			{ kind: 'cleanup-blocked', reason: 'host-refused', resumable: true },
			'cleanup-blocked',
		],
		[
			'an unknown termination',
			{ kind: 'outcome-unknown', phase: 'terminate', reason: 'deadline' },
			'outcome-unknown',
		],
		[
			'a target that stays behind',
			{ kind: 'disposed-awaiting-publication', reason: 'authority-retired', resumable: false },
			'awaiting-removal',
		],
	])('stops at %s without claiming the close', async (_name, status, code) => {
		const { session } = fakeSession([WORKSPACE_ACCEPTED], [closeStatus(status)]);

		await expect(
			closeServerWorkbench(session, executionContext().context, CLOSE_WORKSPACE),
		).resolves.toEqual({ failure: { code }, kind: 'failed' });
	});

	it('stops watching at its budget, with the Server still at work', async () => {
		vi.useFakeTimers({ toFake: ['performance', 'setTimeout'] });
		const { session } = fakeSession([WORKSPACE_ACCEPTED]);
		session.ask = () => Promise.resolve(CLOSING);

		const closing = closeServerWorkbench(session, executionContext().context, CLOSE_WORKSPACE);
		await vi.advanceTimersByTimeAsync(50_001);

		await expect(closing).resolves.toEqual({
			failure: { code: 'status-unavailable' },
			kind: 'failed',
		});
	});

	it('closes a set of Tabs in order and leaves the rest once one does not close', async () => {
		const { actions, session } = fakeSession(
			[FIRST_TAB_ACCEPTED],
			[closeStatus({ kind: 'cleanup-blocked', reason: 'host-refused', resumable: true })],
		);

		await expect(
			closeServerWorkbench(session, executionContext().context, CLOSE_TABS),
		).resolves.toEqual({ failure: { code: 'cleanup-blocked' }, kind: 'failed' });

		expect(actions).toEqual([{ kind: 'tab.close', tabId: FIRST_TAB, terminalContextId: CONTEXT }]);
	});

	it('leaves a Tab that changed after the question was answered', async () => {
		const { actions, session } = fakeSession(
			[{ answer: FIRST_TAB_ACCEPTED, delivered:readSnapshot({ secondTabLabel: 'Renamed' }) }],
			[CLOSED],
		);

		await expect(
			closeServerWorkbench(session, executionContext().context, CLOSE_TABS),
		).resolves.toEqual({ code: 'scope-changed', kind: 'blocked' });

		expect(actions).toHaveLength(1);
	});

	it('takes an accepted Panel as closed, since its removal arrives as a publication', async () => {
		const { asked, session } = fakeSession([
			settled({
				acceptedRevision: '5',
				closeRequestId: CLOSE_REQUEST,
				kind: 'panel-close-accepted',
				panelId: PANEL,
				tabId: FIRST_TAB,
				terminalContextId: CONTEXT,
			}),
		]);

		await expect(
			closeServerWorkbench(session, executionContext().context, {
				panelId: PANEL,
				tabId: FIRST_TAB,
				terminalContextId: CONTEXT,
				type: 'panel-close',
			}),
		).resolves.toEqual({ kind: 'completed' });

		expect(asked).toEqual([]);
	});

	it('calls a cancelled close harmless only until a step has been handed over', async () => {
		const before = new AbortController();
		before.abort();
		const untouched = fakeSession([]);
		const { context, markCommitted } = executionContext(before.signal);

		await expect(closeServerWorkbench(untouched.session, context, CLOSE_TABS)).resolves.toEqual({
			kind: 'cancelled',
		});
		expect(markCommitted).not.toHaveBeenCalled();

		const during = new AbortController();
		const started = fakeSession([FIRST_TAB_ACCEPTED], []);
		started.session.ask = () => {
			during.abort();
			return Promise.resolve(CLOSED);
		};
		await expect(
			closeServerWorkbench(started.session, executionContext(during.signal).context, CLOSE_TABS),
		).resolves.toEqual({ code: 'not-submitted', kind: 'blocked' });
	});

	it('follows a Project close by its operation until the last member has been published', async () => {
		const { asked, published, session } = fakeSession(
			[],
			[
				groupStatus({ kind: 'pending' }, [
					{ requestId: CLOSE_REQUEST, revision: '8' },
					{ requestId: OTHER_CLOSE_REQUEST, revision: null },
				]),
				groupStatus({ kind: 'closed' }, [
					{ requestId: CLOSE_REQUEST, revision: '8' },
					{ requestId: OTHER_CLOSE_REQUEST, revision: '9' },
				]),
			],
		);

		await expect(
			closeServerWorkbench(session, executionContext().context, CLOSE_PROJECT),
		).resolves.toEqual({ kind: 'completed' });

		const [start, read] = asked;
		if (start?.kind !== 'workbench.workspace-group-close') throw new Error('Expected the start.');
		if (read?.kind !== 'workbench.workspace-group-close') throw new Error('Expected the read.');
		expect(start.action).toEqual({
			kind: 'start',
			operationId: start.requestId,
			projectId: PROJECT,
		});
		expect(read.action).toEqual({ kind: 'read', operationId: start.requestId });
		expect(read.requestId).not.toBe(start.requestId);
		expect(published).toEqual(['9']);
	});

	it('keeps the operation a Project close can be looked up by when its outcome is unknown', async () => {
		const { asked, session } = fakeSession([], [{ kind: 'outcome-unknown' }]);

		const result = await closeServerWorkbench(session, executionContext().context, CLOSE_PROJECT);

		const [start] = asked;
		expect(result).toEqual({
			failure: { code: 'outcome-unknown', details: start?.requestId },
			kind: 'failed',
		});
		expect(asked).toHaveLength(1);
	});
});

describe('watchServerWorkbenchClose', () => {
	it('withdraws a question only when what it describes changes', () => {
		const listeners: ((snapshot: WorkbenchSnapshot) => void)[] = [];
		const onChanged = vi.fn<() => void>();

		const stop = watchServerWorkbenchClose(
			{
				snapshot: readSnapshot(),
				subscribe: (listener) => {
					listeners.push(listener);
					return () => undefined;
				},
			},
			{ tabIds: [FIRST_TAB], terminalContextId: CONTEXT, type: 'tabs-close' },
			onChanged,
		);

		for (const listener of listeners) {
			listener(readSnapshot({ revision: '6', secondTabLabel: 'Not the one asked about' }));
		}
		expect(onChanged).not.toHaveBeenCalled();

		const renamedWorkspace = watchServerWorkbenchClose(
			{
				snapshot: readSnapshot(),
				subscribe: (listener) => {
					listener(readSnapshot({ revision: '6', workspaceLabel: 'renamed' }));
					return () => undefined;
				},
			},
			CLOSE_WORKSPACE,
			onChanged,
		);
		expect(onChanged).toHaveBeenCalledOnce();

		stop();
		renamedWorkspace();
	});
});
