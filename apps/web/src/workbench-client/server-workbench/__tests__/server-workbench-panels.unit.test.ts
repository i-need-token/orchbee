import type {
	WorkbenchAuthorityContext,
	WorkbenchAuthoritySplitRatioChange,
} from '@orchbee/workbench';
import { parseWorkbenchResponse, type WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { changeServerSplitRatios } from '../server-workbench-panels';
import type { ServerWorkbenchAction, ServerWorkbenchSubmission } from '../ServerWorkbenchSession';

const SESSION = '11111111111111111111111111111111';
const WORKSPACE = '22222222222222222222222222222222';
const DIRECTORY = '33333333333333333333333333333333';
const WORKER = '44444444444444444444444444444444';
const CONTEXT = '55555555555555555555555555555555';
const TAB = '66666666666666666666666666666666';
const OUTER_SPLIT = '77777777777777777777777777777777';
const INNER_SPLIT = '88888888888888888888888888888888';
const REQUEST = '99999999999999999999999999999999';
const EVEN_RATIO = 0.5;

interface Ratios {
	inner: number;
	outer: number;
}

function panel(serial: number) {
	const id = (offset: number): string => `${offset + serial}`.padStart(32, '0');
	return {
		childPresentations: [],
		kind: 'panel',
		panelId: id(100),
		primaryPresentation: {
			binding: null,
			customLabel: null,
			label: 'Terminal',
			presentationId: id(200),
		},
	};
}

function split(splitId: string, ratio: number, first: unknown, second: unknown, slots: number) {
	const slotId = (offset: number): string => `${300 + slots + offset}`.padStart(32, '0');
	return {
		children: [
			{ node: first, slotId: slotId(0) },
			{ node: second, slotId: slotId(1) },
		],
		kind: 'split',
		orientation: 'horizontal',
		...(ratio === EVEN_RATIO ? {} : { ratio }),
		splitId,
	};
}

/** Three Panels in a row, the second and third inside a split of their own. */
function readSnapshot(ratios: Ratios): WorkbenchSnapshot {
	const response = parseWorkbenchResponse({
		kind: 'workbench.snapshot.result',
		outcome: {
			kind: 'available',
			snapshot: {
				kind: 'workbench.snapshot',
				navigation: {
					directories: [{ directoryId: DIRECTORY, path: 'C:\\work', workerId: WORKER }],
					projects: [],
					workspaces: [
						{
							label: 'work',
							primaryDirectoryId: DIRECTORY,
							projectId: null,
							terminalContextId: CONTEXT,
							workspaceId: WORKSPACE,
						},
					],
				},
				protocolVersion: 1,
				revision: '5',
				sessionId: SESSION,
				terminalContexts: [
					{
						groups: [],
						tabs: [
							{
								groupId: null,
								label: 'Tab',
								pinned: false,
								root: split(
									OUTER_SPLIT,
									ratios.outer,
									panel(1),
									split(INNER_SPLIT, ratios.inner, panel(2), panel(3), 2),
									0,
								),
								tabId: TAB,
							},
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

/** Commits what it is asked, unless `refuses` says a commit elsewhere got there first. */
function harness(initial: Ratios, refuses: (attempt: number, ratios: Ratios) => boolean) {
	const ratios = { ...initial };
	const sent: {
		readonly previousRatio: number;
		readonly ratio: number;
		readonly splitId: string;
	}[] = [];
	const session = {
		get snapshot(): WorkbenchSnapshot {
			return readSnapshot(ratios);
		},
		submit: (
			decide: (snapshot: WorkbenchSnapshot) => ServerWorkbenchAction | null,
		): Promise<ServerWorkbenchSubmission> => {
			const action = decide(readSnapshot(ratios));
			if (action === null) return Promise.resolve({ kind: 'not-submitted' });
			if (action.kind !== 'layout.set-split-ratio') throw new Error(`Unexpected ${action.kind}.`);
			const { previousRatio, ratio } = action;
			sent.push({ previousRatio, ratio, splitId: action.split.splitId });
			if (refuses(sent.length, ratios)) {
				return Promise.resolve(
					settled({ kind: 'rejected', reason: 'revision-mismatch', revision: '6' }),
				);
			}
			ratios[action.split.splitId === OUTER_SPLIT ? 'outer' : 'inner'] = ratio;
			return Promise.resolve(settled({ kind: 'split-ratio-configured', revision: '6' }));
		},
	};
	return { sent, session };
}

const CONTEXT_FOR_ACTION: WorkbenchAuthorityContext = {
	markCommitted: () => undefined,
	signal: new AbortController().signal,
};
const RATIO_SET: WorkbenchAuthoritySplitRatioChange = {
	leadingWeight: 60,
	splitId: OUTER_SPLIT,
	tabId: TAB,
	terminalContextId: CONTEXT,
	type: 'split-ratio-set',
};

describe('changeServerSplitRatios', () => {
	afterEach(() => {
		vi.restoreAllMocks();
	});

	it('evens a Tab one split at a time, giving every Panel on the axis the same share', async () => {
		const { sent, session } = harness({ inner: 0.3, outer: 0.7 }, () => false);

		await expect(
			changeServerSplitRatios(session, CONTEXT_FOR_ACTION, {
				tabId: TAB,
				terminalContextId: CONTEXT,
				type: 'panels-equalized',
			}),
		).resolves.toEqual({ kind: 'completed' });

		expect(sent).toEqual([
			{ previousRatio: 0.7, ratio: 0.33333, splitId: OUTER_SPLIT },
			{ previousRatio: 0.3, ratio: EVEN_RATIO, splitId: INNER_SPLIT },
		]);
	});

	it('asks nothing of a Tab that is already even', async () => {
		const { sent, session } = harness({ inner: EVEN_RATIO, outer: 0.33333 }, () => false);

		await expect(
			changeServerSplitRatios(session, CONTEXT_FOR_ACTION, {
				tabId: TAB,
				terminalContextId: CONTEXT,
				type: 'panels-equalized',
			}),
		).resolves.toEqual({ kind: 'unchanged' });

		expect(sent).toEqual([]);
	});

	it('goes again when only the revision moved, and gives up after five seconds of that', async () => {
		let clock = 0;
		vi.spyOn(performance, 'now').mockImplementation(() => clock);
		const { sent, session } = harness({ inner: EVEN_RATIO, outer: EVEN_RATIO }, () => {
			clock += 2_000;
			return true;
		});

		await expect(changeServerSplitRatios(session, CONTEXT_FOR_ACTION, RATIO_SET)).resolves.toEqual({
			code: 'ratio-contended',
			kind: 'blocked',
		});

		// The deadline starts at the first refusal, at two seconds, and the fourth passes it.
		expect(sent).toHaveLength(4);
		expect(new Set(sent.map(({ ratio }) => ratio))).toEqual(new Set([0.6]));
	});

	it('stops rather than overwrite a split that someone else moved', async () => {
		const { sent, session } = harness(
			{ inner: EVEN_RATIO, outer: EVEN_RATIO },
			(attempt, ratios) => {
				ratios.outer = 0.25;
				return attempt === 1;
			},
		);

		await expect(changeServerSplitRatios(session, CONTEXT_FOR_ACTION, RATIO_SET)).resolves.toEqual({
			code: 'split-changed',
			kind: 'blocked',
		});

		expect(sent).toHaveLength(1);
	});
});
