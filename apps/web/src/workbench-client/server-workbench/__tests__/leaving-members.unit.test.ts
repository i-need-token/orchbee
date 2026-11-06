import { parseWorkbenchResponse, type WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	createDefaultWorkbenchClientView,
	createWorkbenchClientView,
	type WorkbenchAuthorityContext,
} from '../../../workbench';
import {
	createSnapshotRuntime,
	FIXTURE_INTL,
	fixtureContextId,
	fixtureId,
	fixturePanelId,
	fixtureProjectId,
	fixtureTabId,
	fixtureWorkspaceId,
	oneWorkspaceSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotPanel,
	snapshotProject,
	snapshotSplit,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../__tests__/server-workbench-snapshot';
import { createServerWorkbenchClose } from '../createServerWorkbenchClose';
import { projectServerSnapshot } from '../projectServerSnapshot';
import { ServerLeaving } from '../ServerLeaving';
import type { ServerWorkbenchAnswer, ServerWorkbenchSubmission } from '../ServerWorkbenchSession';

type CloseSession = Parameters<typeof createServerWorkbenchClose>[0];

const SESSION = fixtureId(0x1);
const REQUEST = fixtureId(0xf01);
const CLOSE_REQUEST = fixtureId(0xc01);
const CONTEXT = fixtureContextId(1);
const TAB = fixtureTabId(1);
const PANEL = fixturePanelId(1);
const SIBLING = fixturePanelId(2);
const LAST_PANEL = fixturePanelId(3);
const WORKSPACE = fixtureWorkspaceId(1);
const PROJECT = fixtureProjectId(1);
const CLOSING = { kind: 'closing', phase: 'queued' };
const BLOCKED = { kind: 'cleanup-blocked', reason: 'host-refused', resumable: false };
const PANEL_TARGET = { kind: 'panel', tabId: TAB, terminalContextId: CONTEXT };
const WORKSPACE_TARGET = { kind: 'workspace', workspaceId: WORKSPACE };

function response(record: Readonly<Record<string, unknown>>) {
	return parseWorkbenchResponse({ protocolVersion: 1, requestId: REQUEST, ...record });
}

function settledWith(outcome: unknown): ServerWorkbenchSubmission {
	const parsed = response({ kind: 'workbench.command.result', outcome });
	if (parsed.kind !== 'workbench.command.result') {
		throw new Error('The outcome did not parse as a command result.');
	}
	return { kind: 'settled', outcome: parsed.outcome };
}

function readOf(status: unknown, target: unknown): ServerWorkbenchAnswer {
	return {
		kind: 'answered',
		response: response({
			kind: 'workbench.close-operation.result',
			operationId: CLOSE_REQUEST,
			outcome: { generation: '1', kind: 'available', status, target },
			sessionId: SESSION,
		}),
	};
}

function groupAnswer(status: unknown): ServerWorkbenchAnswer {
	return {
		kind: 'answered',
		response: response({
			kind: 'workbench.workspace-group-close.result',
			operationId: REQUEST,
			outcome: {
				attempt: 1,
				kind: 'available',
				members: [{ close: null, label: 'Workspace', workspaceId: WORKSPACE }],
				projectId: PROJECT,
				status,
			},
		}),
	};
}

/**
 * A Server that settles the one submission it is handed and answers the reads in turn. `onRead`
 * hears each read before it is answered, so a test can see what the window shows while it waits.
 */
function serverSession(
	snapshot: WorkbenchSnapshot,
	submission: ServerWorkbenchSubmission | null,
	reads: readonly ServerWorkbenchAnswer[],
	onRead: () => void = () => undefined,
): CloseSession {
	const remaining = [...reads];
	return {
		ask: () => {
			onRead();
			const answer = remaining.shift();
			if (answer === undefined) throw new Error('Nothing more is read.');
			return Promise.resolve(answer);
		},
		published: () => Promise.resolve(),
		snapshot,
		submit: (decide) => {
			decide(snapshot);
			if (submission === null) throw new Error('Nothing is submitted.');
			return Promise.resolve(submission);
		},
		subscribe: () => () => undefined,
	};
}

function executionContext(): WorkbenchAuthorityContext {
	return {
		markCommitted: vi.fn<WorkbenchAuthorityContext['markCommitted']>(),
		signal: new AbortController().signal,
	};
}

describe('leaving members', () => {
	it('removes a Panel of a split as the Server does, so its sibling takes the split place', () => {
		const snapshot = oneWorkspaceSnapshot(5, [
			snapshotTab(1, snapshotSplit(1, 'horizontal', snapshotPanel(1), snapshotPanel(2))),
		]);
		const leaving = new ServerLeaving(
			() => snapshot,
			() => undefined,
		);
		leaving.leave([{ kind: 'panel', panelId: PANEL, tabId: TAB, terminalContextId: CONTEXT }]);

		const publication = projectServerSnapshot(snapshot, FIXTURE_INTL, undefined, leaving);
		expect(publication.terminalContexts[0]?.tabs[0]?.root).toMatchObject({
			id: SIBLING,
			kind: 'panel',
		});
	});

	it('withholds a Tab once every Panel of it has left', () => {
		const snapshot = oneWorkspaceSnapshot(5, [
			snapshotTab(1, snapshotSplit(1, 'horizontal', snapshotPanel(1), snapshotPanel(2))),
		]);
		const leaving = new ServerLeaving(
			() => snapshot,
			() => undefined,
		);
		leaving.leave([
			{ kind: 'panel', panelId: PANEL, tabId: TAB, terminalContextId: CONTEXT },
			{ kind: 'panel', panelId: SIBLING, tabId: TAB, terminalContextId: CONTEXT },
		]);

		const publication = projectServerSnapshot(snapshot, FIXTURE_INTL, undefined, leaving);
		expect(publication.terminalContexts[0]).toMatchObject({ tabs: [], withheldTabIds: [TAB] });
	});

	it('leaves a Panel of several out from its close acceptance and shows it again once the close is refused', async () => {
		const snapshot = oneWorkspaceSnapshot(5, [
			snapshotTab(1, snapshotSplit(1, 'horizontal', snapshotPanel(1), snapshotPanel(2))),
		]);
		const leaving = new ServerLeaving(
			() => snapshot,
			() => undefined,
		);
		const leftWhileReads: boolean[] = [];
		const session = serverSession(
			snapshot,
			settledWith({
				acceptedRevision: '5',
				closeRequestId: CLOSE_REQUEST,
				kind: 'panel-close-accepted',
				panelId: PANEL,
				tabId: TAB,
				terminalContextId: CONTEXT,
			}),
			[readOf(CLOSING, PANEL_TARGET), readOf(BLOCKED, PANEL_TARGET)],
			() => leftWhileReads.push(leaving.panel(CONTEXT, TAB, PANEL)),
		);
		const closes = createServerWorkbenchClose(session, leaving);

		await expect(
			closes.close(executionContext(), {
				panelId: PANEL,
				tabId: TAB,
				terminalContextId: CONTEXT,
				type: 'panel-close',
			}),
		).resolves.toEqual({ failure: { code: 'cleanup-blocked' }, kind: 'failed' });
		expect(leftWhileReads).toEqual([true, true]);
		expect(leaving.panel(CONTEXT, TAB, PANEL)).toBe(false);
	});

	it('leaves a Workspace out from its close acceptance and shows it again once the close stops', async () => {
		const snapshot = oneWorkspaceSnapshot(5, [snapshotTab(1)]);
		const leaving = new ServerLeaving(
			() => snapshot,
			() => undefined,
		);
		const leftWhileReads: boolean[] = [];
		const session = serverSession(
			snapshot,
			settledWith({
				acceptedRevision: '5',
				closeRequestId: CLOSE_REQUEST,
				kind: 'workspace-close-accepted',
				terminalContextId: CONTEXT,
				workspaceId: WORKSPACE,
			}),
			[readOf(CLOSING, WORKSPACE_TARGET), readOf(BLOCKED, WORKSPACE_TARGET)],
			() => leftWhileReads.push(leaving.workspace(WORKSPACE)),
		);
		const closes = createServerWorkbenchClose(session, leaving);

		await expect(
			closes.close(executionContext(), { type: 'workspace-close', workspaceId: WORKSPACE }),
		).resolves.toEqual({ failure: { code: 'cleanup-blocked' }, kind: 'failed' });
		expect(leftWhileReads).toEqual([true, true]);
		expect(leaving.workspace(WORKSPACE)).toBe(false);
	});

	it('leaves each Workspace of a Project out once the Server accepts the Project close', async () => {
		const snapshot = workbenchSnapshot(
			5,
			{
				directories: [snapshotDirectory(1)],
				projects: [snapshotProject(1)],
				workspaces: [snapshotWorkspace(1, { projectId: PROJECT })],
			},
			[snapshotContext(1, [snapshotTab(1)])],
		);
		const leaving = new ServerLeaving(
			() => snapshot,
			() => undefined,
		);
		// The first read is the Server's answer to the start, which the window reads before it leaves.
		const leftWhileReads: boolean[] = [];
		const session = serverSession(
			snapshot,
			null,
			[
				groupAnswer({ kind: 'pending' }),
				groupAnswer({ kind: 'failed', reason: 'outcome-unknown', resumable: false }),
			],
			() => leftWhileReads.push(leaving.workspace(WORKSPACE)),
		);
		const closes = createServerWorkbenchClose(session, leaving);

		await expect(
			closes.close(executionContext(), { projectId: PROJECT, type: 'project-workspaces-close' }),
		).resolves.toMatchObject({ failure: { code: 'outcome-unknown' }, kind: 'failed' });
		expect(leftWhileReads).toEqual([false, true]);
		expect(leaving.workspace(WORKSPACE)).toBe(false);
	});

	it('hands focus from a focused Panel that leaves to the Panel at its place, clamped to those that remain', () => {
		const snapshot = oneWorkspaceSnapshot(5, [
			snapshotTab(
				1,
				snapshotSplit(
					1,
					'horizontal',
					snapshotPanel(1),
					snapshotSplit(2, 'vertical', snapshotPanel(2), snapshotPanel(3)),
				),
			),
		]);
		const leaving = new ServerLeaving(
			() => snapshot,
			() => undefined,
		);
		const before = createDefaultWorkbenchClientView(
			projectServerSnapshot(snapshot, FIXTURE_INTL, undefined, leaving),
		);
		const [context] = before.terminalContexts;
		const [tab] = context?.panelWorkspace.tabs ?? [];
		if (context === undefined || tab === undefined) throw new Error('Expected one Tab of Panels.');
		const focusedOnLast = createWorkbenchClientView({
			...before,
			terminalContexts: [
				{ ...context, panelWorkspace: { tabs: [{ ...tab, focusedPanelId: LAST_PANEL }] } },
			],
		});
		const runtime = createSnapshotRuntime(snapshot, { clientView: focusedOnLast });
		leaving.leave([{ kind: 'panel', panelId: LAST_PANEL, tabId: TAB, terminalContextId: CONTEXT }]);

		runtime.applyPublishedWorkbench(
			projectServerSnapshot(snapshot, FIXTURE_INTL, undefined, leaving),
		);
		expect(
			runtime.getClientView().terminalContexts[0]?.panelWorkspace.tabs[0]?.focusedPanelId,
		).toBe(SIBLING);
	});
});
