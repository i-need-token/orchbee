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
import { ServerLeaving } from '../ServerLeaving';
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
 * for it, and a closed read removes the Tab. A set closes its members in order, each by that same
 * close, and each read of the set reads its first member not yet closed; a blocked member stops it.
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
	let set: { readonly members: readonly number[]; readonly operationId: string } | null = null;
	const observed = new Map<number, { readonly answer: CloseStatus; readonly revision: number }>();

	const leaving = new ServerLeaving(
		() => snapshot,
		() => {
			project();
		},
	);
	function project(): void {
		leaving.prune(snapshot);
		runtime.applyPublishedWorkbench(
			projectServerSnapshot(snapshot, FIXTURE_INTL, undefined, leaving),
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

	/** Takes the next status of Tab `sequence`'s close, as the window shows it then. */
	const step = (sequence: number): CloseStatus => {
		const answer = remaining.get(sequence)?.shift();
		if (answer === undefined) throw new Error('Nothing more is read.');
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
		return answer;
	};
	const read = (operationId: string): ServerWorkbenchAnswer => {
		const sequence = held.find((candidate) => closeRequestOf(candidate) === operationId);
		if (sequence === undefined) throw new Error('Nothing more is read.');
		const answer = step(sequence);
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

	const readSet = ({ members, operationId }: NonNullable<typeof set>): ServerWorkbenchAnswer => {
		const seen = members.map((sequence) => observed.get(sequence)?.answer);
		let status: Readonly<Record<string, string>> = { kind: 'pending' };
		if (seen.includes('blocked')) status = { kind: 'failed', reason: 'child-blocked' };
		else if (seen.every((answer) => answer === 'closed')) status = { kind: 'closed' };
		return {
			kind: 'answered',
			response: response({
				kind: 'workbench.tab-set-close.result',
				operationId,
				outcome: {
					condition: 'confirmed',
					kind: 'available',
					members: members.map((sequence) => {
						const close = observed.get(sequence);
						return {
							absent: false,
							close:
								close === undefined
									? null
									: {
											observation: {
												attemptGeneration: '1',
												status: closeStatus(close.answer, close.revision),
											},
											requestId: closeRequestOf(sequence),
										},
							label: `Tab ${sequence}`,
							tabId: fixtureTabId(sequence),
						};
					}),
					status,
					terminalContextId: CONTEXT,
				},
			}),
		};
	};

	const session: CloseSession = {
		ask: (request) => {
			switch (request.kind) {
				case 'workbench.close-operation.read':
					return Promise.resolve(read(request.operationId));
				case 'workbench.tab-set-close': {
					const { action } = request;
					if (action.kind === 'start') {
						set = { members: action.tabIds.map(sequenceOf), operationId: action.operationId };
					} else if (set?.operationId === action.operationId) {
						const next = set.members.find(
							(sequence) => observed.get(sequence)?.answer !== 'closed',
						);
						if (next !== undefined) observed.set(next, { answer: step(next), revision });
					}
					if (set === null) throw new Error('No set has started.');
					return Promise.resolve(readSet(set));
				}
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

	const closes = createServerWorkbenchClose(session, leaving);
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

/**
 * As in the TUI, every member of a set leaves the strip once the Server accepts the set, and a
 * selected member gives way in that same change to the next Tab outside the set, else the previous
 * one, else none. The members the Server leaves open come back, and the selection stays where it
 * moved.
 */
describe('closing several Tabs', () => {
	it('leaves with every member at once, moving the selection to the next Tab outside the set, else the previous, else none', async () => {
		const next = closingWindow(
			5,
			3,
			new Map([
				[2, CLOSES_THEN_CLOSED],
				[3, CLOSES_THEN_CLOSED],
				[4, CLOSES_THEN_CLOSED],
			]),
		);
		await expect(next.close(2, 3, 4)).resolves.toEqual({ kind: 'completed' });
		const outside = { selected: fixtureTabId(5), shown: [fixtureTabId(1), fixtureTabId(5)] };
		expect(next.viewsAt(2, 'closing')).toEqual([outside]);
		expect(next.view()).toEqual(outside);

		const previous = closingWindow(
			4,
			4,
			new Map([
				[3, CLOSES_THEN_CLOSED],
				[4, CLOSES_THEN_CLOSED],
			]),
		);
		await expect(previous.close(3, 4)).resolves.toEqual({ kind: 'completed' });
		expect(previous.viewsAt(3, 'closing')).toEqual([
			{ selected: fixtureTabId(2), shown: [fixtureTabId(1), fixtureTabId(2)] },
		]);

		const none = closingWindow(
			2,
			1,
			new Map([
				[1, CLOSES_THEN_CLOSED],
				[2, CLOSES_THEN_CLOSED],
			]),
		);
		await expect(none.close(1, 2)).resolves.toEqual({ kind: 'completed' });
		expect(none.viewsAt(1, 'closing')).toEqual([{ selected: null, shown: [] }]);
	});

	it('brings back the members the Server leaves open, and keeps the selection where it moved', async () => {
		const strip = closingWindow(
			4,
			2,
			new Map<number, readonly CloseStatus[]>([
				[1, CLOSES_THEN_CLOSED],
				[2, ['blocked']],
			]),
		);

		await expect(strip.close(1, 2, 3)).resolves.toMatchObject({ kind: 'failed' });

		expect(strip.view()).toEqual({
			selected: fixtureTabId(4),
			shown: [fixtureTabId(2), fixtureTabId(3), fixtureTabId(4)],
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
