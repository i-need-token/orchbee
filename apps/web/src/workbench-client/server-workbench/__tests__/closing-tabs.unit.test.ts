import type { WorkbenchAuthorityClosure } from '../../../workbench';
import {
	parseWorkbenchResponse,
	type WorkbenchResponse,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	FIXTURE_INTL,
	fixtureContextId,
	fixtureId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import { createServerWorkbenchClose } from '../createServerWorkbenchClose';
import { projectServerSnapshot } from '../projectServerSnapshot';
import { ServerLeavingTabs } from '../ServerLeavingTabs';
import type { ServerWorkbenchAnswer, ServerWorkbenchSubmission } from '../ServerWorkbenchSession';

type CloseSession = Parameters<typeof createServerWorkbenchClose>[0];
type CloseStatus = 'blocked' | 'closed' | 'closing';

const CONTEXT = fixtureContextId(1);
const SESSION = fixtureId(0x1);
const REQUEST = fixtureId(0xf01);

/** The one close the Server keeps for Tab `sequence`. */
function closeRequestOf(sequence: number): string {
	return fixtureId(0xc00 + sequence);
}

/** What the window shows when the Server is read about a Tab's close. */
interface ReadView {
	readonly answer: CloseStatus;
	readonly selected: string | null;
	readonly shown: readonly string[];
	readonly tab: string;
}

function response(record: Readonly<Record<string, unknown>>): WorkbenchResponse {
	return parseWorkbenchResponse({ protocolVersion: 1, requestId: REQUEST, ...record });
}

function closeStatus(status: CloseStatus, revision: number): unknown {
	switch (status) {
		case 'blocked':
			return { kind: 'cleanup-blocked', reason: 'host-refused', resumable: true };
		case 'closed':
			return { kind: 'closed', revision: `${revision}` };
		case 'closing':
			return { kind: 'closing', phase: 'queued' };
	}
}

/**
 * A window showing Tabs 1 to `tabCount` with `selectedTab` selected, wired as the window wires its
 * close to the Tabs it leaves out, against a Server that answers each read of Tab `n`'s close with
 * the next status `reads.get(n)` names. Every Close of a Tab joins the one close the Server keeps
 * for it, and a closed read removes the Tab.
 */
function closingWindow(
	tabCount: number,
	selectedTab: number,
	reads: ReadonlyMap<number, readonly CloseStatus[]>,
) {
	let held = Array.from({ length: tabCount }, (_, index) => index + 1);
	let revision = 1;
	let snapshot: WorkbenchSnapshot = oneWorkspaceSnapshot(
		revision,
		held.map((sequence) => snapshotTab(sequence)),
	);
	const remaining = new Map([...reads].map(([sequence, statuses]) => [sequence, [...statuses]]));
	const views: ReadView[] = [];
	const runtime = createSnapshotRuntime(snapshot);
	activeTerminalContext(runtime).tabModel.selectTab(fixtureTabId(selectedTab));

	const leavingTabs = new ServerLeavingTabs(
		() => snapshot,
		(terminalContextId, tabId) =>
			runtime.getTerminalContext(terminalContextId)?.tabModel.getSnapshot().activeTabId === tabId,
		() => {
			project();
		},
	);
	function project(): void {
		leavingTabs.prune(snapshot);
		runtime.applyPublishedWorkbench(
			projectServerSnapshot(snapshot, FIXTURE_INTL, undefined, leavingTabs.leaving),
		);
	}
	/** Projects the Server's snapshot as it stands, as each publication does, and reads the strip. */
	function view(): Omit<ReadView, 'answer' | 'tab'> {
		project();
		const tabModel = activeTerminalContext(runtime).tabModel.getSnapshot();
		return { selected: tabModel.activeTabId, shown: tabModel.tabs.map((tab) => tab.id) };
	}
	const sequenceOf = (tabId: string) =>
		held.find((sequence) => fixtureTabId(sequence) === tabId) ?? 0;

	const read = (operationId: string): ServerWorkbenchAnswer => {
		const sequence = held.find((candidate) => closeRequestOf(candidate) === operationId);
		const answer = sequence === undefined ? undefined : remaining.get(sequence)?.shift();
		if (sequence === undefined || answer === undefined) throw new Error('Nothing more is read.');
		views.push({ answer, tab: fixtureTabId(sequence), ...view() });
		if (answer === 'closed') {
			held = held.filter((candidate) => candidate !== sequence);
			revision += 1;
			snapshot = oneWorkspaceSnapshot(
				revision,
				held.map((candidate) => snapshotTab(candidate)),
			);
			project();
		}
		return {
			kind: 'answered',
			response: response({
				kind: 'workbench.close-operation.result',
				operationId,
				outcome: {
					generation: '1',
					kind: 'available',
					status: closeStatus(answer, revision),
					target: { kind: 'tab', tabId: fixtureTabId(sequence), terminalContextId: CONTEXT },
				},
				sessionId: SESSION,
			}),
		};
	};

	const session: CloseSession = {
		ask: (request) => {
			switch (request.kind) {
				case 'workbench.close-operation.read':
					return Promise.resolve(read(request.operationId));
				case 'workbench.tab-close.resume':
					return Promise.resolve({
						kind: 'answered',
						response: response({
							closeRequestId: request.closeRequestId,
							kind: 'workbench.tab-close.resume.result',
							outcome: { attemptGeneration: '2', kind: 'resumed' },
							sessionId: SESSION,
							tabId: request.tabId,
							terminalContextId: request.terminalContextId,
						}),
					});
				default:
					throw new Error(`Nothing asks ${request.kind} here.`);
			}
		},
		published: () => Promise.resolve(),
		get snapshot() {
			return snapshot;
		},
		submit: (decide) => {
			const action = decide(snapshot);
			if (action?.kind !== 'tab.close') throw new Error('Only Tab closes are submitted here.');
			const outcome = response({
				kind: 'workbench.command.result',
				outcome: {
					acceptedRevision: `${revision}`,
					closeRequestId: closeRequestOf(sequenceOf(action.tabId)),
					kind: 'tab-close-accepted',
					tabId: action.tabId,
					terminalContextId: action.terminalContextId,
				},
			});
			if (outcome.kind !== 'workbench.command.result') throw new Error('Expected a result.');
			const submission: ServerWorkbenchSubmission = { kind: 'settled', outcome: outcome.outcome };
			return Promise.resolve(submission);
		},
		subscribe: () => () => undefined,
	};

	const closes = createServerWorkbenchClose(session, leavingTabs);
	const run = (closure: WorkbenchAuthorityClosure) =>
		closes.close({ markCommitted: () => undefined, signal: new AbortController().signal }, closure);
	return {
		close: (...sequences: number[]) =>
			run({ tabIds: sequences.map(fixtureTabId), terminalContextId: CONTEXT, type: 'tabs-close' }),
		resume: (sequence: number) =>
			run({ tabId: fixtureTabId(sequence), terminalContextId: CONTEXT, type: 'tab-close-resume' }),
		view,
		/** What the window showed at each read of Tab `sequence`'s close that answered `answer`. */
		viewsAt: (sequence: number, answer: CloseStatus) =>
			views
				.filter(
					(candidate) => candidate.tab === fixtureTabId(sequence) && candidate.answer === answer,
				)
				.map(({ selected, shown }) => ({ selected, shown })),
	};
}

const CLOSES_THEN_CLOSED: readonly CloseStatus[] = ['closing', 'closed'];

describe('closing several Tabs', () => {
	it('keeps the selected member selected and shown until the Server removes it, then selects the next Tab still shown, else the previous', async () => {
		const strip = closingWindow(
			4,
			3,
			new Map([
				[2, CLOSES_THEN_CLOSED],
				[3, CLOSES_THEN_CLOSED],
				[4, CLOSES_THEN_CLOSED],
			]),
		);

		await expect(strip.close(2, 3, 4)).resolves.toEqual({ kind: 'completed' });

		const selectedWhileClosing = (sequence: number) =>
			strip.viewsAt(sequence, 'closing').map(({ selected }) => selected);
		expect(selectedWhileClosing(2)).toEqual([fixtureTabId(3)]);
		expect(selectedWhileClosing(3)).toEqual([fixtureTabId(3)]);
		expect(strip.viewsAt(3, 'closing')[0]?.shown).toContain(fixtureTabId(3));
		expect(selectedWhileClosing(4)).toEqual([fixtureTabId(4)]);
		expect(strip.viewsAt(4, 'closing')[0]?.shown).toContain(fixtureTabId(4));
		expect(strip.view().selected).toBe(fixtureTabId(1));
	});

	it.each([
		['the selected member', [2, 3]],
		['a member that is not selected', [1, 2]],
	])('leaves the selection where it was when %s comes back', async (_name, members) => {
		const [first = 0] = members;
		const strip = closingWindow(3, 2, new Map([[first, ['blocked']]]));

		await expect(strip.close(...members)).resolves.toMatchObject({ kind: 'failed' });

		expect(strip.view()).toEqual({
			selected: fixtureTabId(2),
			shown: [fixtureTabId(1), fixtureTabId(2), fixtureTabId(3)],
		});
	});
});

describe('closing a Tab whose close stopped', () => {
	it('leaves the strip only once a read finds the close on its way, while a Resume leaves at once', async () => {
		const strip = closingWindow(
			2,
			1,
			new Map<number, readonly CloseStatus[]>([
				[1, ['blocked', 'blocked', 'closing', 'closed']],
				[2, ['blocked', 'closing', 'closed']],
			]),
		);
		const shownAt = (sequence: number, answer: CloseStatus) =>
			strip.viewsAt(sequence, answer).map(({ shown }) => shown.includes(fixtureTabId(sequence)));

		await strip.close(1);
		await expect(strip.close(1)).resolves.toMatchObject({ kind: 'failed' });
		// A new close leaves at once; the Close that joins the stopped one stays while it is still stopped.
		expect(shownAt(1, 'blocked')).toEqual([false, true]);

		await expect(strip.close(1)).resolves.toEqual({ kind: 'completed' });
		expect(shownAt(1, 'closing')).toEqual([true]);
		expect(shownAt(1, 'closed')).toEqual([false]);

		await strip.close(2);
		await expect(strip.resume(2)).resolves.toEqual({ kind: 'completed' });
		expect(shownAt(2, 'closing')).toEqual([false]);
	});
});
