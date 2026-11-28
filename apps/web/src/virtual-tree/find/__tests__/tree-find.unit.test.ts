import { afterEach, describe, expect, it, vi } from 'vitest';

import { TreeViewport } from '../../layout/TreeViewport';
import { VirtualTreeModel } from '../../model/VirtualTreeModel';
import type { VirtualTreeRow } from '../../model/virtual-tree-types';
import type { VirtualTreeFindProvider, VirtualTreeFindResult } from '../../types/virtual-tree-find';
import { TreeFind } from '../TreeFind';
import historyFixture from './fixtures/vscode-find-history.json';

// The tree find's rules, as VS Code's tree find controller and its asynchronous variant follow
// them, on a model of files whose labels are their keys. Rows are 22 px high in a view 220 px high,
// so ten rows show at a time.

interface Item {
	readonly key: string;
	readonly children: readonly Item[] | null;
}

type Row = VirtualTreeRow<string, Item>;

async function openTree(
	names: readonly string[],
	provider: VirtualTreeFindProvider<string, Item> | null = null,
) {
	const model = new VirtualTreeModel<string, Item>({
		input: { children: names.map((key) => ({ children: null, key })), key: '' },
		source: {
			children: (item) => item.children ?? [],
			hasChildren: (item) => item.children !== null,
			key: (item) => item.key,
		},
	});
	await model.refresh(null);
	const viewport = new TreeViewport(model);
	// The view follows the model only while something listens, as the tree's components do.
	viewport.subscribe(() => undefined);
	viewport.setSize(300, 220);
	let treeFocus = 0;
	const find = new TreeFind(viewport, {
		focusTree: () => {
			treeFocus += 1;
		},
		label: (row: Row) => row.key,
		provider: () => provider,
	});
	const disconnect = find.connect();
	return { disconnect, find, focusCount: () => treeFocus, model, viewport };
}

function rowKeys(model: VirtualTreeModel<string, Item>): string[] {
	return Array.from({ length: model.rowCount }, (_, index) => model.rowAt(index).key);
}

/** Waits until the model shows the find for `pattern`, or no find for null, which its slices may delay. */
async function applied(
	model: VirtualTreeModel<string, Item>,
	pattern: string | null,
): Promise<void> {
	await vi.waitFor(() => {
		expect(model.findStatus?.query.pattern ?? null).toBe(pattern);
	});
}

interface ProviderRequest {
	readonly pattern: string;
	readonly options: Parameters<VirtualTreeFindProvider<string, Item>['find']>[1];
	readonly signal: AbortSignal;
	readonly settle: (result: VirtualTreeFindResult<string, Item> | undefined) => void;
	readonly fail: () => void;
}

/** A provider whose requests wait until the test answers them. */
function waitingProvider(isVisible?: (row: Row) => boolean) {
	const requests: ProviderRequest[] = [];
	const startSession = vi.fn<() => void>();
	const endSession = vi.fn<() => Promise<void>>(async () => undefined);
	const provider: VirtualTreeFindProvider<string, Item> = {
		endSession,
		find: async (pattern, options, signal) =>
			await new Promise((resolve, reject) => {
				requests.push({
					fail: () => {
						reject(new Error('The search failed.'));
					},
					options,
					pattern,
					settle: resolve,
					signal,
				});
			}),
		isVisible,
		startSession,
	};
	const request = (index: number): ProviderRequest => {
		const found = requests[index];
		if (found === undefined) throw new Error(`No request ${index} among ${requests.length}`);
		return found;
	};
	return { endSession, provider, request, requests, startSession };
}

afterEach(() => {
	vi.useRealTimers();
});

describe('tree find', () => {
	it('opens with the pattern it closed with and finds it again, and closing shows every row and returns to the tree', async () => {
		const { find, focusCount, model } = await openTree(['apple', 'banana', 'cherry', 'date']);
		find.setMode('filter');
		find.open();
		find.type('an');
		await applied(model, 'an');
		expect(rowKeys(model)).toEqual(['banana']);

		find.close();
		await applied(model, null);
		expect([find.getSnapshot().open, find.getSnapshot().value, focusCount()]).toEqual([
			false,
			'',
			1,
		]);
		expect(rowKeys(model)).toEqual(['apple', 'banana', 'cherry', 'date']);

		find.open();
		await applied(model, 'an');
		expect(find.getSnapshot().value).toBe('an');
		expect(rowKeys(model)).toEqual(['banana']);
	});

	it('moves focus to the first match from the focused row, wrapping, and brings it into sight', async () => {
		const names = Array.from({ length: 60 }, (_, index) => `file-${index}`);
		names[5] = 'early-target';
		names[35] = 'late-target';
		const { find, model, viewport } = await openTree(names);
		model.setFocus('file-2');
		find.open();

		// A match in sight takes focus without scrolling.
		find.type('target');
		await applied(model, 'target');
		expect([model.focus, viewport.scrollTop]).toEqual(['early-target', 0]);
		// A focused row that matches keeps focus.
		find.type('targ');
		await applied(model, 'targ');
		expect(model.focus).toBe('early-target');
		// VS Code reveals a row out of sight at the middle: 35 × 22 - (220 - 22) × 0.5.
		model.setFocus('file-30');
		find.type('target');
		await applied(model, 'target');
		expect([model.focus, viewport.scrollTop]).toEqual(['late-target', 671]);
		find.type('early');
		await applied(model, 'early');
		expect([model.focus, viewport.scrollTop]).toEqual(['early-target', 11]);
	});

	it('says nothing matched only among items, and announces each pattern it applied', async () => {
		const { find, model } = await openTree(['apple', 'banana']);
		find.open();
		find.type('an');
		await applied(model, 'an');
		expect(find.getSnapshot()).toMatchObject({
			announcement: { count: 1, serial: 1 },
			message: null,
		});
		find.type('anx');
		await applied(model, 'anx');
		expect(find.getSnapshot()).toMatchObject({
			announcement: { count: 0, serial: 2 },
			message: { kind: 'notFound' },
		});
		// A new match type renders again, as VS Code's does.
		find.setMatchType('contiguous');
		await vi.waitFor(() => {
			expect(model.findStatus?.query.matchType).toBe('contiguous');
		});
		expect(find.getSnapshot().announcement).toEqual({ count: 0, serial: 3 });
		find.type('');
		await applied(model, null);
		expect(find.getSnapshot()).toMatchObject({
			announcement: { count: 0, serial: 3 },
			message: null,
		});

		const empty = await openTree([]);
		empty.find.open();
		empty.find.type('a');
		await applied(empty.model, 'a');
		expect(empty.find.getSnapshot()).toMatchObject({
			announcement: { count: 0, serial: 1 },
			message: null,
		});
	});

	it('lets keys land only on matched rows while two or more match, and on any row otherwise', async () => {
		const { find, model } = await openTree(['a1', 'b1', 'a2']);
		const row = (key: string): Row => model.rowAt(model.indexOf(key));
		expect(find.allowsFocus(row('b1'))).toBe(true);
		find.open();
		find.type('a');
		await applied(model, 'a');
		expect([find.allowsFocus(row('a1')), find.allowsFocus(row('b1'))]).toEqual([true, false]);
		find.type('a2');
		await applied(model, 'a2');
		expect(find.allowsFocus(row('b1'))).toBe(true);
	});

	it('recalls and remembers patterns as VS Code’s tree find does', async () => {
		for (const { ops, states } of historyFixture.cases) {
			const { disconnect, find, focusCount } = await openTree([]);
			const actual = ops.split(' ').map((op) => {
				if (op === 'open') find.open();
				else if (op === 'close') find.close();
				else if (op === 'enter') find.commit();
				else if (op === 'up') find.historyPrevious();
				else if (op === 'down') find.historyNext();
				else find.type(op.slice('type:'.length));
				const { open, value } = find.getSnapshot();
				return `${open ? 1 : 0}:${value}:${focusCount()}`;
			});
			disconnect();
			// The operations go along, so a failure names the case.
			expect({ ops, states: actual.join(' ') }).toEqual({ ops, states });
		}
	});
});

describe('tree find provider', () => {
	it('asks once typing rests for 250 ms, one request at a time, for the latest pattern, and reports its answer', async () => {
		vi.useFakeTimers();
		const { endSession, provider, request, requests, startSession } = waitingProvider();
		const { disconnect, find, model } = await openTree(['alpha', 'beta'], provider);
		find.open();
		find.type('a');
		await vi.advanceTimersByTimeAsync(200);
		find.type('al');
		await vi.advanceTimersByTimeAsync(249);
		expect(requests).toHaveLength(0);
		// The loaded rows are found at once, and the message and announcement wait for the provider.
		expect(model.findStatus?.query.pattern).toBe('al');
		expect(find.getSnapshot().announcement).toBeNull();
		await vi.advanceTimersByTimeAsync(1);
		expect(requests.map(({ options, pattern }) => [pattern, options])).toEqual([
			['al', { matchType: 'fuzzy', mode: 'highlight' }],
		]);
		expect(startSession).toHaveBeenCalledTimes(1);

		// A newer pattern aborts the request at once, and is asked once the request ends.
		find.type('alp');
		expect(request(0).signal.aborted).toBe(true);
		await vi.advanceTimersByTimeAsync(250);
		expect(requests).toHaveLength(1);
		request(0).settle({ isMatch: () => true, matchCount: 9 });
		await vi.advanceTimersByTimeAsync(0);
		expect(requests.map(({ pattern }) => pattern)).toEqual(['al', 'alp']);
		expect(find.getSnapshot().announcement).toBeNull();

		const warning = 'Showing the first 512 results.';
		request(1).settle({ isMatch: (row) => row.key === 'beta', matchCount: 7, warning });
		await vi.advanceTimersByTimeAsync(0);
		expect(find.getSnapshot()).toMatchObject({
			announcement: { count: 7 },
			message: { kind: 'warning', text: warning },
		});
		// Keys land on the rows either the provider or the loaded rows matched.
		const row = (key: string): Row => model.rowAt(model.indexOf(key));
		expect([find.allowsFocus(row('alpha')), find.allowsFocus(row('beta'))]).toEqual([true, true]);
		expect(model.focus).toBe('alpha');

		// Emptying the input ends the session once typing rests; the next character starts another.
		find.type('');
		await vi.advanceTimersByTimeAsync(250);
		expect(endSession).toHaveBeenCalledTimes(1);
		find.type('b');
		await vi.advanceTimersByTimeAsync(250);
		expect(startSession).toHaveBeenCalledTimes(2);
		// The tree going aborts the request and ends the session.
		disconnect();
		expect(request(2).signal.aborted).toBe(true);
		expect(endSession).toHaveBeenCalledTimes(2);
	});

	it('hides the rows its isVisible rejects while filtering, from its answer until the mode changes', async () => {
		vi.useFakeTimers();
		const { provider, request } = waitingProvider((row) => row.key !== 'alpine');
		const { find, model } = await openTree(['alpha', 'alpine', 'beta'], provider);
		find.setMode('filter');
		find.open();
		find.type('al');
		await vi.advanceTimersByTimeAsync(0);
		expect(rowKeys(model)).toEqual(['alpha', 'alpine']);
		await vi.advanceTimersByTimeAsync(250);
		request(0).settle({ isMatch: () => false, matchCount: 1 });
		await vi.advanceTimersByTimeAsync(0);
		expect(rowKeys(model)).toEqual(['alpha']);

		find.setMode('highlight');
		find.setMode('filter');
		await vi.advanceTimersByTimeAsync(0);
		expect(rowKeys(model)).toEqual(['alpha', 'alpine']);
		await vi.advanceTimersByTimeAsync(250);
		expect(request(1).options.mode).toBe('filter');
	});

	it('leaves the rows and messages as they were when the provider fails, and asks again for the next pattern', async () => {
		vi.useFakeTimers();
		const { provider, request, requests } = waitingProvider();
		const { find } = await openTree(['alpha', 'beta'], {
			...provider,
			startSession: () => {
				if (requests.length === 0) throw new Error('The session could not start.');
			},
		});
		find.open();
		find.type('z');
		await vi.advanceTimersByTimeAsync(250);
		expect(requests).toHaveLength(0);
		find.type('zz');
		await vi.advanceTimersByTimeAsync(250);
		request(0).fail();
		await vi.advanceTimersByTimeAsync(0);
		expect(find.getSnapshot()).toMatchObject({ announcement: null, message: null });

		find.type('zzz');
		await vi.advanceTimersByTimeAsync(250);
		request(1).settle({ isMatch: () => false, matchCount: 0 });
		await vi.advanceTimersByTimeAsync(0);
		expect(find.getSnapshot()).toMatchObject({
			announcement: { count: 0 },
			message: { kind: 'notFound' },
		});
	});
});
