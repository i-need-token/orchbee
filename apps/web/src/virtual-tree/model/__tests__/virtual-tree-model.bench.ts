import { bench, type BenchOptions, vi } from 'vitest';

import { VirtualTreeModel } from '../VirtualTreeModel';
import type {
	VirtualTreeEvent,
	VirtualTreeFindQuery,
	VirtualTreeRow,
	VirtualTreeSource,
	VirtualTreeSplice,
} from '../virtual-tree-types';

// The reference scenarios of the File Bar proposal's performance targets: N = 100,000 rows labelled
// like `file-123.ts`, and 1,000 folders of 100 files, with compact rows over the same sizes. Each
// case builds its tree outside the measured region and times one operation, so its median compares
// with VS Code's model measurements.

const N = 100_000;
const FOLDERS = 1_000;
const FILES_PER_FOLDER = 100;

interface BenchItem {
	readonly key: string;
	readonly folder: boolean;
}

/** Time spent building inputs inside a case, removed from the clock the benchmark reads. */
let excluded = 0;

function untimed<R>(work: () => R): R {
	const start = performance.now();
	try {
		return work();
	} finally {
		excluded += performance.now() - start;
	}
}

const OPTIONS: BenchOptions = {
	iterations: 7,
	now: () => performance.now() - excluded,
	time: 0,
	warmupIterations: 2,
	warmupTime: 0,
};

const ROOT: BenchItem = { folder: true, key: '' };

class Fixture {
	readonly model: VirtualTreeModel<string, BenchItem>;
	readonly events: VirtualTreeEvent<string>[] = [];
	private readonly lists = new Map<string, readonly BenchItem[]>();

	/** Folders join compact rows when `compact` is set, as VS Code's Explorer compacts them. */
	constructor(rootItems: readonly BenchItem[], expandedByDefault = false, compact = false) {
		this.lists.set(ROOT.key, rootItems);
		const isFolder = (item: BenchItem) => item.folder;
		const source: VirtualTreeSource<string, BenchItem> = {
			children: (item) => this.lists.get(item.key) ?? [],
			hasChildren: isFolder,
			key: (item) => item.key,
			...(compact ? { compressible: isFolder } : {}),
		};
		this.model = new VirtualTreeModel({
			input: ROOT,
			source,
			...(expandedByDefault ? { expandedByDefault: (item: BenchItem) => item.folder } : {}),
		});
		this.model.subscribe((event) => this.events.push(event));
	}

	setChildren(key: string, items: readonly BenchItem[]): void {
		this.lists.set(key, items);
	}

	/** The splices the operations since the last call reported, as one list. */
	takeSplices(): VirtualTreeSplice[] {
		const splices = this.events.flatMap((event) => (event.kind === 'rows' ? event.splices : []));
		this.events.length = 0;
		return splices;
	}
}

function files(count: number, prefix = ''): BenchItem[] {
	return Array.from({ length: count }, (_, index) => ({
		folder: false,
		key: `${prefix}file-${index}.ts`,
	}));
}

function loadedRoot(items: readonly BenchItem[]): Fixture {
	const fixture = new Fixture(items);
	void fixture.model.refresh(null);
	fixture.takeSplices();
	return fixture;
}

/** Registers 1,000 folders of 100 files each with the fixture and returns the folders. */
function nestedFolders(fixture: Fixture): BenchItem[] {
	const folders: BenchItem[] = [];
	for (let index = 0; index < FOLDERS; index += 1) {
		const folder = { folder: true, key: `folder-${index}` };
		folders.push(folder);
		fixture.setChildren(folder.key, files(FILES_PER_FOLDER, `${folder.key}/`));
	}
	return folders;
}

/** Registers 1,000 folders that each hold one folder of 100 files, and returns the outer folders. */
function compactFolders(fixture: Fixture): BenchItem[] {
	const folders: BenchItem[] = [];
	for (let index = 0; index < FOLDERS; index += 1) {
		const folder = { folder: true, key: `folder-${index}` };
		const inner = { folder: true, key: `${folder.key}/inner` };
		folders.push(folder);
		fixture.setChildren(folder.key, [inner]);
		fixture.setChildren(inner.key, files(FILES_PER_FOLDER, `${inner.key}/`));
	}
	return folders;
}

/**
 * A loaded tree whose one top-level row is compact: `a`, `a/b` and `a/b/c` over 100,000 files,
 * expanded or collapsed.
 */
function compactChain(expanded: boolean): Fixture {
	const fixture = new Fixture([{ folder: true, key: 'a' }], true, true);
	fixture.setChildren('a', [{ folder: true, key: 'a/b' }]);
	fixture.setChildren('a/b', [{ folder: true, key: 'a/b/c' }]);
	fixture.setChildren('a/b/c', files(N, 'a/b/c/'));
	void fixture.model.refresh(null);
	if (!expanded) fixture.model.collapse('a/b/c');
	fixture.takeSplices();
	return fixture;
}

/** Fails the case when an operation did not produce the rows and splices it promises. */
function expectRows(
	fixture: Fixture,
	rowCount: number,
	splices: readonly VirtualTreeSplice[] | null,
): void {
	untimed(() => {
		const actualRows = fixture.model.rowCount;
		const actualSplices = fixture.takeSplices();
		if (actualRows !== rowCount) throw new Error(`Expected ${rowCount} rows, found ${actualRows}`);
		if (splices !== null && JSON.stringify(actualSplices) !== JSON.stringify(splices)) {
			throw new Error(`Unexpected splices ${JSON.stringify(actualSplices)}`);
		}
	});
}

bench(
	'inserts 100,000 leaves at the root',
	() => {
		const fixture = untimed(() => new Fixture(files(N)));
		void fixture.model.refresh(null);
		expectRows(fixture, N, [{ deleteCount: 0, insertCount: N, start: 0 }]);
	},
	OPTIONS,
);

bench(
	'expands a loaded folder of 100,000 children',
	() => {
		const fixture = untimed(() => {
			const created = loadedRoot([{ folder: true, key: 'big' }]);
			created.setChildren('big', files(N));
			void created.model.expand('big');
			created.model.collapse('big');
			created.takeSplices();
			return created;
		});
		void fixture.model.expand('big');
		expectRows(fixture, N + 1, [{ deleteCount: 0, insertCount: N, start: 1 }]);
	},
	OPTIONS,
);

bench(
	'collapses a folder of 100,000 rows',
	() => {
		const fixture = untimed(() => {
			const created = loadedRoot([{ folder: true, key: 'big' }]);
			created.setChildren('big', files(N));
			void created.model.expand('big');
			created.takeSplices();
			return created;
		});
		fixture.model.collapse('big');
		expectRows(fixture, 1, [{ deleteCount: N, insertCount: 0, start: 1 }]);
	},
	OPTIONS,
);

bench(
	'inserts 1,000 folders of 100 files, expanded by default',
	() => {
		const fixture = untimed(() => {
			const created = new Fixture([], true);
			created.setChildren(ROOT.key, nestedFolders(created));
			return created;
		});
		void fixture.model.refresh(null);
		const rows = FOLDERS * (FILES_PER_FOLDER + 1);
		expectRows(fixture, rows, [{ deleteCount: 0, insertCount: rows, start: 0 }]);
	},
	OPTIONS,
);

bench(
	'collapses 1,000 expanded folders one at a time',
	() => {
		const fixture = untimed(() => {
			const created = new Fixture([], true);
			created.setChildren(ROOT.key, nestedFolders(created));
			void created.model.refresh(null);
			created.takeSplices();
			return created;
		});
		for (let index = 0; index < FOLDERS; index += 1) fixture.model.collapse(`folder-${index}`);
		expectRows(fixture, FOLDERS, null);
	},
	OPTIONS,
);

bench(
	'refreshes 100,000 unchanged children',
	() => {
		const fixture = untimed(() => loadedRoot(files(N)));
		void fixture.model.refresh(null);
		untimed(() => {
			if (fixture.events.length > 0) throw new Error('An unchanged refresh reported a change');
		});
		expectRows(fixture, N, []);
	},
	OPTIONS,
);

bench(
	'refreshes 100,000 children fetched again as new objects',
	() => {
		const fixture = untimed(() => {
			const created = loadedRoot(files(N));
			created.setChildren(ROOT.key, files(N));
			return created;
		});
		void fixture.model.refresh(null);
		// Every row keeps its place; only its element and revision change.
		expectRows(fixture, N, []);
	},
	OPTIONS,
);

bench(
	'refreshes 100,000 children with one inserted in the middle',
	() => {
		const fixture = untimed(() => {
			const items = files(N);
			const created = loadedRoot(items);
			const next = [...items];
			next.splice(N / 2, 0, { folder: false, key: 'inserted.ts' });
			created.setChildren(ROOT.key, next);
			return created;
		});
		void fixture.model.refresh(null);
		expectRows(fixture, N + 1, [{ deleteCount: 0, insertCount: 1, start: N / 2 }]);
	},
	OPTIONS,
);

bench(
	'refreshes 100,000 children with every tenth renamed',
	() => {
		const fixture = untimed(() => {
			const items = files(N);
			const created = loadedRoot(items);
			created.setChildren(
				ROOT.key,
				items.map((item, index) =>
					index % 10 === 5 ? { folder: false, key: `renamed-${index}.ts` } : item,
				),
			);
			return created;
		});
		void fixture.model.refresh(null);
		expectRows(fixture, N, [{ deleteCount: N, insertCount: N, start: 0 }]);
	},
	OPTIONS,
);

bench(
	'expands a compact row of three folders over 100,000 children',
	() => {
		const fixture = untimed(() => compactChain(false));
		void fixture.model.expand('a/b/c');
		expectRows(fixture, N + 1, [{ deleteCount: 0, insertCount: N, start: 1 }]);
	},
	OPTIONS,
);

bench(
	'refreshes 100,000 unchanged children of a compact row',
	() => {
		const fixture = untimed(() => compactChain(true));
		void fixture.model.refresh('a/b/c');
		untimed(() => {
			if (fixture.events.length > 0) throw new Error('An unchanged refresh reported a change');
		});
		expectRows(fixture, N + 1, []);
	},
	OPTIONS,
);

// 1,000 compact rows of a folder and the one folder it holds, each over 100 files.
const COMPACT_ROWS = FOLDERS * (FILES_PER_FOLDER + 1);
const SPLIT_ROWS = FOLDERS * (FILES_PER_FOLDER + 2);

bench(
	'turns compact rows off across 1,000 folders that each hold one folder of 100 files',
	() => {
		const fixture = untimed(() => {
			const created = new Fixture([], true, true);
			created.setChildren(ROOT.key, compactFolders(created));
			void created.model.refresh(null);
			created.takeSplices();
			return created;
		});
		fixture.model.setCompressed(false);
		expectRows(fixture, SPLIT_ROWS, [
			{ deleteCount: COMPACT_ROWS, insertCount: SPLIT_ROWS, start: 0 },
		]);
	},
	OPTIONS,
);

bench(
	'turns compact rows back on across them',
	() => {
		const fixture = untimed(() => {
			const created = new Fixture([], true, true);
			created.setChildren(ROOT.key, compactFolders(created));
			void created.model.refresh(null);
			created.model.setCompressed(false);
			created.takeSplices();
			return created;
		});
		fixture.model.setCompressed(true);
		expectRows(fixture, COMPACT_ROWS, [
			{ deleteCount: SPLIT_ROWS, insertCount: COMPACT_ROWS, start: 0 },
		]);
	},
	OPTIONS,
);

bench(
	'selects all of 100,000 siblings',
	() => {
		const fixture = untimed(() => loadedRoot(files(N)));
		fixture.model.selectAll();
		untimed(() => {
			if (fixture.model.selection.size !== N) throw new Error('Select all missed rows');
		});
	},
	OPTIONS,
);

bench(
	'resolves the last 100 of 100,000 rows after a splice at the top',
	() => {
		const fixture = untimed(() => {
			const created = loadedRoot(files(N));
			created.setChildren(ROOT.key, [{ folder: false, key: 'first.ts' }, ...files(N)]);
			void created.model.refresh(null);
			return created;
		});
		let total = 0;
		for (let index = N - 100; index < N; index += 1)
			total += fixture.model.indexOf(`file-${index}.ts`);
		untimed(() => {
			const expected = ((N - 99 + N) * 100) / 2;
			if (total !== expected) throw new Error(`Resolved ${total}, expected ${expected}`);
		});
	},
	OPTIONS,
);

bench(
	'expands an unloaded folder of 100,000 children for 1,000 callers with one request',
	async () => {
		const fixture = untimed(() => {
			const children = files(N, 'big/');
			const source = { answer: (): void => undefined, requests: 0 };
			const model = new VirtualTreeModel<string, BenchItem>({
				input: ROOT,
				source: {
					children: (item) => {
						if (item.key === ROOT.key) return [{ folder: true, key: 'big' }];
						source.requests += 1;
						return new Promise((resolve) => {
							source.answer = () => {
								resolve(children);
							};
						});
					},
					hasChildren: (item) => item.folder,
					key: (item) => item.key,
				},
			});
			void model.refresh(null);
			const splices: VirtualTreeSplice[] = [];
			model.subscribe((event) => {
				if (event.kind === 'rows') splices.push(...event.splices);
			});
			return { model, source, splices };
		});
		const expansions = Array.from({ length: 1_000 }, () => fixture.model.expand('big'));
		fixture.source.answer();
		await Promise.all(expansions);
		untimed(() => {
			const { model, source, splices } = fixture;
			const inserted = splices.filter((splice) => splice.insertCount > 0);
			if (source.requests !== 1) throw new Error(`${source.requests} requests for one folder`);
			if (model.rowCount !== N + 1)
				throw new Error(`Expected ${N + 1} rows, found ${model.rowCount}`);
			if (
				JSON.stringify(inserted) !== JSON.stringify([{ deleteCount: 0, insertCount: N, start: 1 }])
			) {
				throw new Error(`Unexpected splices ${JSON.stringify(inserted)}`);
			}
		});
	},
	OPTIONS,
);

/** A find's label: the last part of the item's key, as the Explorer labels a file by its name. */
function findLabel(row: VirtualTreeRow<string, BenchItem>): string {
	return row.key.slice(row.key.lastIndexOf('/') + 1);
}

function findQuery(pattern: string, filter: boolean): VirtualTreeFindQuery<string, BenchItem> {
	return { filter, label: findLabel, matchType: 'fuzzy', pattern };
}

/**
 * Calls `find` and runs every task it leaves back to back, as an idle page would; returns how long
 * each task took, `find` itself first.
 */
function findTasks(
	model: VirtualTreeModel<string, BenchItem>,
	query: VirtualTreeFindQuery<string, BenchItem> | null,
): number[] {
	untimed(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] }));
	try {
		let start = performance.now();
		model.find(query);
		const tasks = [performance.now() - start];
		while (vi.getTimerCount() > 0) {
			start = performance.now();
			vi.advanceTimersToNextTimer();
			tasks.push(performance.now() - start);
		}
		return tasks;
	} finally {
		untimed(() => vi.useRealTimers());
	}
}

/** Leaves only the longest task on the clock, for the cases that time what blocks typing. */
function timeLongestTask(tasks: readonly number[]): void {
	const total = tasks.reduce((sum, task) => sum + task, 0);
	excluded += total - Math.max(...tasks);
}

/** Fails the case when a find did not apply with the counts expected. */
function expectFound(
	fixture: Fixture,
	pattern: string,
	matchCount: number,
	rowCount: number,
): void {
	untimed(() => {
		const status = fixture.model.findStatus;
		if (status?.query.pattern !== pattern) throw new Error(`The find for ${pattern} did not apply`);
		if (status.matchCount !== matchCount) {
			throw new Error(`Expected ${matchCount} matches, found ${status.matchCount}`);
		}
		if (fixture.model.rowCount !== rowCount) {
			throw new Error(`Expected ${rowCount} rows, found ${fixture.model.rowCount}`);
		}
		fixture.takeSplices();
	});
}

/** How many of `file-0.ts` to `file-99999.ts` hold the digit 9 at least `count` times. */
function filesWithNines(count: number): number {
	let total = 0;
	for (let index = 0; index < N; index += 1) {
		if (String(index).split('9').length - 1 >= count) total += 1;
	}
	return total;
}

const ONE_NINE = filesWithNines(1);
const TWO_NINES = filesWithNines(2);

/** 100,000 loaded siblings, filtered by `9` when `filtered` is set. */
function findFixture(filtered: boolean): Fixture {
	const fixture = loadedRoot(files(N));
	if (filtered) {
		findTasks(fixture.model, findQuery('9', true));
		fixture.takeSplices();
	}
	return fixture;
}

bench(
	'finds a first key in all 100,000 siblings, highlighting every row, until it applies',
	() => {
		const fixture = untimed(() => findFixture(false));
		findTasks(fixture.model, findQuery('f', false));
		expectFound(fixture, 'f', N, N);
	},
	OPTIONS,
);

bench(
	'finds a first key in 100,000 siblings, longest task',
	() => {
		const fixture = untimed(() => findFixture(false));
		timeLongestTask(findTasks(fixture.model, findQuery('f', false)));
		expectFound(fixture, 'f', N, N);
	},
	OPTIONS,
);

bench(
	'filters 100,000 siblings by a first key, until it applies',
	() => {
		const fixture = untimed(() => findFixture(false));
		findTasks(fixture.model, findQuery('9', true));
		expectFound(fixture, '9', ONE_NINE, ONE_NINE);
	},
	OPTIONS,
);

bench(
	'filters 100,000 siblings by a first key, longest task',
	() => {
		const fixture = untimed(() => findFixture(false));
		timeLongestTask(findTasks(fixture.model, findQuery('9', true)));
		expectFound(fixture, '9', ONE_NINE, ONE_NINE);
	},
	OPTIONS,
);

bench(
	'narrows a filter of 100,000 siblings by a second key, until it applies',
	() => {
		const fixture = untimed(() => findFixture(true));
		findTasks(fixture.model, findQuery('99', true));
		expectFound(fixture, '99', TWO_NINES, TWO_NINES);
	},
	OPTIONS,
);

bench(
	'clears a filter of 100,000 siblings',
	() => {
		const fixture = untimed(() => findFixture(true));
		findTasks(fixture.model, null);
		untimed(() => {
			if (fixture.model.findStatus !== null || fixture.model.rowCount !== N) {
				throw new Error('The filter did not clear');
			}
		});
	},
	OPTIONS,
);

bench(
	'filters 1,000 expanded folders of 100 files by a first key, until it applies',
	() => {
		const fixture = untimed(() => {
			const created = new Fixture([], true);
			created.setChildren(ROOT.key, nestedFolders(created));
			void created.model.refresh(null);
			created.takeSplices();
			return created;
		});
		findTasks(fixture.model, findQuery('9', true));
		untimed(() => {
			// Every folder holds `file-9.ts`, so every folder shows with its files that hold a 9.
			const status = fixture.model.findStatus;
			if (status?.matchCount !== FOLDERS * 19 + 271) {
				throw new Error(`Unexpected ${status?.matchCount} matches`);
			}
		});
	},
	OPTIONS,
);
