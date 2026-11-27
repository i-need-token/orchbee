import { bench, type BenchOptions } from 'vitest';

import { VirtualTreeModel } from '../VirtualTreeModel';
import type { VirtualTreeEvent, VirtualTreeSource, VirtualTreeSplice } from '../virtual-tree-types';

// The reference scenarios of the File Bar proposal's performance targets: N = 100,000 rows labelled
// like `file-123.ts`, and 1,000 folders of 100 files. Each case builds its tree outside the measured
// region and times one operation, so its median compares with VS Code's model measurements.

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

	constructor(rootItems: readonly BenchItem[], expandedByDefault = false) {
		this.lists.set(ROOT.key, rootItems);
		const source: VirtualTreeSource<string, BenchItem> = {
			children: (item) => this.lists.get(item.key) ?? [],
			hasChildren: (item) => item.folder,
			key: (item) => item.key,
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
