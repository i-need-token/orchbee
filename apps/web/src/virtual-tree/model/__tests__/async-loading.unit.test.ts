import { afterEach, describe, expect, it, vi } from 'vitest';

import { VirtualTreeModel } from '../VirtualTreeModel';
import type { VirtualTreeEvent, VirtualTreeSource } from '../virtual-tree-types';

// The race scenarios of VS Code's asynchronous data tree tests at the pinned commit
// (src/vs/base/test/browser/ui/tree/asyncDataTree.test.ts), with this engine's own rules beside
// them: one request per row at a time, the loading indicator only after 800 ms, and collapse
// aborting the request. A request waits until the test answers it with the children as they are
// then. An item with children in `folders` is a folder.

interface Item {
	readonly key: string;
	readonly label: string;
}

interface Request {
	readonly key: string;
	readonly signal: AbortSignal;
	readonly resolve: (children: readonly Item[]) => void;
	readonly reject: (error: unknown) => void;
}

class ControlledSource {
	readonly folders: Map<string, readonly string[]>;
	/** Labels that differ from the key. */
	readonly labels = new Map<string, string>();
	/** Every key asked for, in order. */
	readonly asked: string[] = [];
	readonly source: VirtualTreeSource<string, Item>;
	private readonly waiting: Request[] = [];

	constructor(folders: Readonly<Record<string, readonly string[]>>) {
		this.folders = new Map(Object.entries(folders));
		this.source = {
			children: (item, signal) =>
				new Promise((resolve, reject) => {
					this.asked.push(item.key);
					this.waiting.push({ key: item.key, reject, resolve, signal });
				}),
			hasChildren: (item) => (this.folders.get(item.key)?.length ?? 0) > 0,
			key: (item) => item.key,
		};
	}

	item(key: string): Item {
		return { key, label: this.labels.get(key) ?? key };
	}

	/** The keys of the requests not answered yet, aborted ones included. */
	get pending(): string[] {
		return this.waiting.map((request) => request.key);
	}

	/** The oldest unanswered request for a key. */
	request(key: string): Request {
		const request = this.waiting.find((waiting) => waiting.key === key);
		if (request === undefined) throw new Error(`Nothing asked for ${key}`);
		return request;
	}

	/** Answers the oldest request for a key and lets the model apply the answer. */
	async answer(key: string): Promise<void> {
		const request = this.take(key);
		request.resolve((this.folders.get(key) ?? []).map((child) => this.item(child)));
		await settled();
	}

	async fail(key: string, error: unknown): Promise<void> {
		this.take(key).reject(error);
		await settled();
	}

	private take(key: string): Request {
		const request = this.request(key);
		this.waiting.splice(this.waiting.indexOf(request), 1);
		return request;
	}
}

/** Lets every pending promise reaction run. */
async function settled(): Promise<void> {
	if (vi.isFakeTimers()) {
		await vi.advanceTimersByTimeAsync(0);
		return;
	}
	await new Promise((resolve) => {
		setTimeout(resolve, 0);
	});
}

function rows(model: VirtualTreeModel<string, Item>): string[] {
	return Array.from({ length: model.rowCount }, (_, index) => model.rowAt(index).element.label);
}

/** A model whose top level has loaded, with nothing asked since. */
async function loadedModel(folders: Readonly<Record<string, readonly string[]>>) {
	const world = new ControlledSource(folders);
	const model = new VirtualTreeModel({ input: world.item('root'), source: world.source });
	const loading = model.refresh(null);
	await world.answer('root');
	await loading;
	world.asked.length = 0;
	return { model, world };
}

afterEach(() => {
	vi.useRealTimers();
});

describe('VirtualTreeModel asynchronous loading', () => {
	it('asks for a row first expanded and settles once the answer arrives (VS Code #80098)', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a'] });
		let expanded = false;
		void model.expand('a').then(() => {
			expanded = true;
		});
		expect(world.asked).toEqual(['a']);
		await settled();
		expect(expanded).toBe(false);
		await world.answer('a');
		expect(expanded).toBe(true);
		expect(rows(model)).toEqual(['a', 'aa']);
	});

	it('expands a row a refresh just left stale with one request (VS Code #80098)', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a'] });
		const refreshing = model.refresh('a');
		const expanding = model.expand('a');
		expect(world.pending).toEqual(['a']);
		await world.answer('a');
		await Promise.all([refreshing, expanding]);
		expect(world.asked).toEqual(['a']);
		expect(rows(model)).toEqual(['a', 'aa']);
	});

	it('loads a row expanded before an ancestor refresh reaches it once (VS Code #347997)', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], b: ['ba'], root: ['a', 'b'] });
		const refreshing = model.refresh(null);
		void model.expand('a');
		await world.answer('root');
		await world.answer('a');
		await refreshing;
		expect(world.asked).toEqual(['root', 'a']);
		expect(world.pending).toEqual([]);
		expect(rows(model)).toEqual(['a', 'aa', 'b']);
	});

	it('keeps a row expanded after an ancestor refresh passed it (VS Code #347997)', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], b: ['ba'], root: ['a', 'b'] });
		await Promise.all([model.expand('b'), world.answer('b')]);
		world.asked.length = 0;
		const refreshing = model.refresh(null);
		await world.answer('root');
		void model.expand('a');
		await world.answer('b');
		await refreshing;
		await world.answer('a');
		expect(world.asked).toEqual(['root', 'b', 'a']);
		expect(rows(model)).toEqual(['a', 'aa', 'b', 'ba']);
	});

	it('refreshes a row and its child together with one request each (VS Code #84569, #82629)', async () => {
		const { model, world } = await loadedModel({ a: ['b'], b: ['ba'], root: ['a'] });
		await Promise.all([model.expand('a'), world.answer('a')]);
		await Promise.all([model.expand('b'), world.answer('b')]);
		world.asked.length = 0;
		world.folders.set('b', ['bb']);
		world.labels.set('b', 'b2');
		const refreshing = Promise.all([model.refresh('a'), model.refresh('b')]);
		await world.answer('a');
		await world.answer('b');
		await refreshing;
		expect(world.asked).toEqual(['a', 'b']);
		expect(rows(model)).toEqual(['a', 'b2', 'bb']);
	});

	it('asks again for a child whose request started before its parent was refreshed', async () => {
		const { model, world } = await loadedModel({ a: ['b'], b: ['ba'], root: ['a'] });
		await Promise.all([model.expand('a'), world.answer('a')]);
		await Promise.all([model.expand('b'), world.answer('b')]);
		world.asked.length = 0;
		const refreshingChild = model.refresh('b');
		// The change may come after the child's request has read.
		world.folders.set('b', ['bb']);
		const refreshingParent = model.refresh('a');
		await world.answer('a');
		await world.answer('b');
		await world.answer('b');
		await Promise.all([refreshingChild, refreshingParent]);
		expect(world.asked).toEqual(['b', 'a', 'b']);
		expect(rows(model)).toEqual(['a', 'b', 'bb']);
	});

	it('asks for a collapsed row only when it expands, whatever refreshes come first (VS Code #67722, #121567)', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a'] });
		await Promise.all([model.expand('a'), world.answer('a')]);
		model.collapse('a');
		world.asked.length = 0;
		await model.refresh('a');
		world.folders.set('a', []);
		await model.refresh('a');
		expect(model.rowAt(0).collapsible).toBe(false);
		world.folders.set('a', ['aa']);
		await model.refresh('a');
		expect(model.rowAt(0).collapsible).toBe(true);
		expect(world.asked).toEqual([]);
		void model.expand('a');
		expect(world.asked).toEqual(['a']);
	});

	it('shows a refreshed collapsed row none of its old children when it expands (VS Code #192422)', async () => {
		const { model, world } = await loadedModel({ a: ['b'], root: ['a'] });
		await Promise.all([model.expand('a'), world.answer('a')]);
		model.collapse('a');
		// Without a refresh the children stay, and expanding shows them at once.
		await model.expand('a');
		expect(rows(model)).toEqual(['a', 'b']);
		model.collapse('a');
		world.folders.set('a', ['c']);
		await model.refresh('a');
		void model.expand('a');
		expect(rows(model)).toEqual(['a']);
		await world.answer('a');
		expect(rows(model)).toEqual(['a', 'c']);
	});

	it('shares a row request between callers that need no newer answer', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a'] });
		const first = model.expand('a');
		const second = model.expand('a');
		expect(world.pending).toEqual(['a']);
		await world.answer('a');
		await Promise.all([first, second]);
		expect(world.asked).toEqual(['a']);
	});

	it('answers every refresh asked during a request with one more request once it ends', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a'] });
		await Promise.all([model.expand('a'), world.answer('a')]);
		world.asked.length = 0;
		const refreshes = [model.refresh('a')];
		world.folders.set('a', ['aa', 'ab']);
		refreshes.push(model.refresh('a'), model.refresh('a'));
		expect(world.pending).toEqual(['a']);
		await world.answer('a');
		expect(world.pending).toEqual(['a']);
		world.folders.set('a', ['aa', 'ab', 'ac']);
		await world.answer('a');
		await Promise.all(refreshes);
		expect(world.asked).toEqual(['a', 'a']);
		expect(rows(model)).toEqual(['a', 'aa', 'ab', 'ac']);
	});

	it('keeps a recursive refresh recursive once the older request it waited for ends', async () => {
		const { model, world } = await loadedModel({ a: ['b'], b: ['c'], root: ['a'] });
		await Promise.all([model.expand('a'), world.answer('a')]);
		await Promise.all([model.expand('b'), world.answer('b')]);
		model.collapse('b');
		world.folders.set('b', ['d']);
		const refreshing = Promise.all([model.refresh(null), model.refresh(null, { recursive: true })]);
		await world.answer('root');
		await world.answer('a');
		await world.answer('root');
		await world.answer('a');
		await refreshing;
		// The collapsed row forgot the children it held, so expanding it asks again.
		void model.expand('b');
		await world.answer('b');
		expect(rows(model)).toEqual(['a', 'b', 'd']);
	});

	it('keeps a recursive refresh recursive when it shares a newer request', async () => {
		const { model, world } = await loadedModel({ a: ['b'], b: ['c'], root: ['a'] });
		await Promise.all([model.expand('a'), world.answer('a')]);
		await Promise.all([model.expand('b'), world.answer('b')]);
		model.collapse('b');
		world.folders.set('b', ['d']);
		world.asked.length = 0;
		const refreshing = Promise.all([model.refresh(null, { recursive: true }), model.refresh('a')]);
		await world.answer('root');
		await world.answer('a');
		await refreshing;
		expect(world.asked).toEqual(['root', 'a']);
		void model.expand('b');
		await world.answer('b');
		expect(rows(model)).toEqual(['a', 'b', 'd']);
	});

	it('aborts the request of a row that collapses and drops its late answer', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a'] });
		const expanding = model.expand('a');
		const { signal } = world.request('a');
		model.collapse('a');
		expect(signal.aborted).toBe(true);
		await expanding;
		await world.answer('a');
		expect(rows(model)).toEqual(['a']);
		void model.expand('a');
		expect(world.asked).toEqual(['a', 'a']);
		await world.answer('a');
		expect(rows(model)).toEqual(['a', 'aa']);
	});

	it('loads a row whose refresh a collapse aborted again when it expands', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a'] });
		await Promise.all([model.expand('a'), world.answer('a')]);
		world.asked.length = 0;
		const refreshing = model.refresh('a');
		model.collapse('a');
		await refreshing;
		void model.expand('a');
		expect(rows(model)).toEqual(['a']);
		world.folders.set('a', ['ab']);
		await world.answer('a');
		expect(rows(model)).toEqual(['a']);
		await world.answer('a');
		expect(world.asked).toEqual(['a', 'a']);
		expect(rows(model)).toEqual(['a', 'ab']);
	});

	it('aborts the request of a row that leaves the tree', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a', 'b'] });
		void model.expand('a');
		const { signal } = world.request('a');
		world.folders.set('root', ['b']);
		const refreshing = model.refresh(null);
		await world.answer('root');
		await refreshing;
		expect(signal.aborted).toBe(true);
		expect(rows(model)).toEqual(['b']);
	});

	it('shows a row as loading only once its request has waited 800 ms', async () => {
		vi.useFakeTimers();
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a'] });
		const events: VirtualTreeEvent<string>[] = [];
		model.subscribe((event) => events.push(event));
		void model.expand('a');
		void model.expand('a');
		await vi.advanceTimersByTimeAsync(799);
		expect(model.rowAt(0).loading).toBe(false);
		events.length = 0;
		await vi.advanceTimersByTimeAsync(1);
		expect(model.rowAt(0).loading).toBe(true);
		expect(events).toEqual([{ kind: 'rows', splices: [] }]);
		await world.answer('a');
		expect(model.rowAt(0).loading).toBe(false);
		expect(rows(model)).toEqual(['a', 'aa']);
		expect(world.asked).toEqual(['a']);
	});

	it('collapses a row whose request fails, rejects the expansion, and asks again on expand', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a'] });
		const failure = new Error('unreadable');
		const expansion = model.expand('a').then(
			() => null,
			(error: unknown) => error,
		);
		await world.fail('a', failure);
		expect(await expansion).toBe(failure);
		expect(model.rowAt(0).collapsed).toBe(true);
		void model.expand('a');
		expect(world.asked).toEqual(['a', 'a']);
	});

	it('collapses a row whose source cancels its request without failing the expansion (VS Code)', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a'] });
		const expansion = model.expand('a');
		await world.fail('a', new DOMException('The read was cancelled.', 'AbortError'));
		await expansion;
		expect(model.rowAt(0).collapsed).toBe(true);
		void model.expand('a');
		expect(world.asked).toEqual(['a', 'a']);
	});

	it('aborts every request once disposed and applies no late answer (VS Code #199264)', async () => {
		const { model, world } = await loadedModel({ a: ['aa'], root: ['a'] });
		void model.expand('a');
		const refreshing = model.refresh(null);
		model.dispose();
		expect(world.request('a').signal.aborted).toBe(true);
		expect(world.request('root').signal.aborted).toBe(true);
		await world.answer('a');
		await refreshing;
		expect(rows(model)).toEqual(['a']);
	});
});
