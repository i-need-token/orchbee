import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	type FileError,
	type FileListPage,
	type FileResult,
	type FileWorkspaceSession,
	MemoryFileSessionPort,
} from '../../../file-session';
import { ExplorerTree, type ExplorerTreeState } from '../ExplorerTree';
import { ExplorerViewMemory } from '../ExplorerViewMemory';

const WORKSPACE = 'workspace';

function unexpectedReport(name: string): void {
	throw new Error(`The folder ${name} was reported.`);
}

/** A session whose listings and releases the test can count, and whose listed folders can fail. */
function observe(
	session: FileWorkspaceSession,
	failures: ReadonlyMap<string, FileError> = new Map(),
): {
	readonly listed: string[];
	readonly released: string[][];
	readonly session: FileWorkspaceSession;
} {
	const listed: string[] = [];
	const released: string[][] = [];
	return {
		listed,
		released,
		session: {
			...session,
			tree: {
				...session.tree,
				list: (request, signal): Promise<FileResult<FileListPage>> => {
					if (request.cursor === undefined) listed.push(request.path);
					const error = failures.get(request.path);
					if (error !== undefined) return Promise.resolve({ error, kind: 'error' });
					return session.tree.list(request, signal);
				},
				release: (paths) => {
					released.push([...paths]);
					session.tree.release(paths);
				},
			},
		},
	};
}

function machine(answering: 'immediate' | 'manual' = 'immediate'): MemoryFileSessionPort {
	const port = new MemoryFileSessionPort({ answering });
	port.writeFile('/w/a.txt', 'a');
	port.writeFile('/w/src/b.ts', 'b');
	port.writeFile('/w/src/lib/c.ts', 'c');
	port.writeFile('/w/docs/d.md', 'd');
	port.setRoot(WORKSPACE, '/w');
	return port;
}

async function settle(): Promise<void> {
	for (let turn = 0; turn < 20; turn += 1) await Promise.resolve();
}

/** The items the tree shows from top to bottom, each item of a compact row on its own. */
function rows(state: ExplorerTreeState): readonly string[] {
	if (state.kind !== 'tree') return [];
	const { model } = state;
	return Array.from({ length: model.rowCount }, (_, index) =>
		model.rowAt(index).segments.map((segment) => segment.key),
	).flat();
}

function modelOf(tree: ExplorerTree): Extract<ExplorerTreeState, { kind: 'tree' }>['model'] {
	const { state } = tree;
	if (state.kind !== 'tree') throw new Error(`The Explorer shows ${state.kind}.`);
	return state.model;
}

afterEach(() => {
	vi.useRealTimers();
});

describe('ExplorerTree', () => {
	it('lists each loaded folder a burst of changes affects once, after the delay', async () => {
		vi.useFakeTimers();
		const port = machine();
		const { listed, session } = observe(port.open(WORKSPACE));
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: unexpectedReport,
			session,
		});
		await settle();
		await modelOf(tree).expand('/w/src');
		listed.length = 0;

		port.writeFile('/w/src/new.ts', 'n');
		port.writeFile('/w/src/b.ts', 'changed');
		port.writeFile('/w/src/other.ts', 'o');
		port.remove('/w/a.txt');
		await vi.advanceTimersByTimeAsync(499);
		expect(listed).toEqual([]);
		await vi.advanceTimersByTimeAsync(1);
		await settle();

		expect([...listed].sort()).toEqual(['/w', '/w/src']);
		expect(rows(tree.state)).toEqual([
			'/w/docs',
			'/w/src',
			'/w/src/lib',
			'/w/src/b.ts',
			'/w/src/new.ts',
			'/w/src/other.ts',
		]);
		tree.dispose();
	});

	it('lists every loaded folder again after records were lost', async () => {
		const port = machine();
		const { listed, session } = observe(port.open(WORKSPACE));
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: unexpectedReport,
			session,
		});
		await settle();
		const model = modelOf(tree);
		await model.expand('/w/src');
		await model.expand('/w/docs');
		model.collapse('/w/docs');

		port.disconnect();
		port.writeFile('/w/docs/e.md', 'e');
		port.writeFile('/w/src/f.ts', 'f');
		listed.length = 0;
		port.reconnect();
		await settle();

		// The collapsed folder forgets its children rather than listing them now.
		expect([...listed].sort()).toEqual(['/w', '/w/src']);
		expect(rows(tree.state)).toContain('/w/src/f.ts');
		await model.expand('/w/docs');
		expect(rows(tree.state)).toContain('/w/docs/e.md');
		tree.dispose();
	});

	it('matches the folders again after a rescan amid changes names only a folder above them', async () => {
		vi.useFakeTimers();
		const port = machine();
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: unexpectedReport,
			session: port.open(WORKSPACE),
		});
		await settle();
		const model = modelOf(tree);
		await model.expand('/w/src');
		await model.expand('/w/src/lib');

		port.writeFile('/w/first.txt', 'f');
		// The Server reads late, so these changes fold into one rescan, which names `/w` alone once
		// its folders are past the bound.
		port.overflow(WORKSPACE);
		port.writeFile('/w/src/lib/deep.ts', 'd');
		port.remove('/w/src/b.ts');
		port.writeFile('/w/top.txt', 't');
		port.drain(WORKSPACE, 1);
		port.writeFile('/w/later.txt', 'l');
		await vi.advanceTimersByTimeAsync(500);
		await settle();

		expect(rows(tree.state)).toEqual([
			'/w/docs',
			'/w/src',
			'/w/src/lib',
			'/w/src/lib/c.ts',
			'/w/src/lib/deep.ts',
			'/w/a.txt',
			'/w/first.txt',
			'/w/later.txt',
			'/w/top.txt',
		]);
		tree.dispose();
	});

	it('lists a folder again when a rescan of it overtook its first listing', async () => {
		const port = machine('manual');
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: unexpectedReport,
			session: port.open(WORKSPACE),
		});
		await settle();
		port.flush();
		await settle();
		const expanding = modelOf(tree).expand('/w/src');
		await settle();

		// The Server lists the folder and then drops a change in it; the rescan for that change
		// arrives before the listing does.
		port.flush();
		port.overflow(WORKSPACE);
		port.writeFile('/w/src/e.ts', 'e');
		port.drain(WORKSPACE);
		await expanding;
		await settle();
		port.flush();
		await settle();

		expect(rows(tree.state)).toEqual([
			'/w/docs',
			'/w/src',
			'/w/src/lib',
			'/w/src/b.ts',
			'/w/src/e.ts',
			'/w/a.txt',
		]);
		tree.dispose();
	});

	it('shows a new root once it has loaded, and only the latest one', async () => {
		const port = machine('manual');
		const { released, session } = observe(port.open(WORKSPACE));
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: unexpectedReport,
			session,
		});
		await settle();
		// The first root's title shows while its tree loads, with nothing else to show.
		expect(tree.state).toEqual({ kind: 'opening', name: 'w', root: '/w' });
		port.flush();
		await settle();
		const first = modelOf(tree);

		port.setRoot(WORKSPACE, '/w/src');
		await settle();
		port.setRoot(WORKSPACE, '/w/src/lib');
		await settle();
		expect(tree.state).toMatchObject({ kind: 'tree', model: first, root: '/w' });

		port.flush();
		await settle();
		expect(tree.state).toMatchObject({ kind: 'tree', root: '/w/src/lib' });
		expect(rows(tree.state)).toEqual(['/w/src/lib/c.ts']);
		expect(released).toEqual([['/w']]);
		tree.dispose();
	});

	it('moves the root down and back up drawing what it listed, and lists again what went unwatched', async () => {
		const port = machine('manual');
		const { listed, released, session } = observe(port.open(WORKSPACE));
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: unexpectedReport,
			session,
		});
		await settle();
		port.flush();
		await settle();
		for (const folder of ['/w/src', '/w/src/lib']) {
			const expanding = modelOf(tree).expand(folder);
			await settle();
			port.flush();
			await expanding;
		}
		listed.length = 0;

		port.setRoot(WORKSPACE, '/w/src');
		await settle();
		expect(listed).toEqual([]);
		expect(rows(tree.state)).toEqual(['/w/src/lib', '/w/src/lib/c.ts', '/w/src/b.ts']);
		expect(released).toEqual([['/w']]);

		// The Server no longer reports changes in the released root.
		port.writeFile('/w/e.txt', 'e');
		port.setRoot(WORKSPACE, '/w');
		await settle();
		const model = modelOf(tree);
		expect(rows(tree.state)).toEqual([
			'/w/docs',
			'/w/src',
			'/w/src/lib',
			'/w/src/lib/c.ts',
			'/w/src/b.ts',
			'/w/a.txt',
		]);
		expect(model.focus).toBe('/w/src');
		expect(model.scrollTop).toBe(22);
		expect(listed).toEqual(['/w']);
		port.flush();
		await settle();
		expect(rows(tree.state)).toContain('/w/e.txt');
		tree.dispose();
	});

	it("restores and saves the Workspace's expanded folders through its record", async () => {
		const saved: string[][] = [];
		const record = {
			read: () => ['/w/docs'],
			write: (expanded: readonly string[]) => saved.push([...expanded]),
		};
		const port = machine();
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(record),
			reportUnreadable: unexpectedReport,
			session: port.open(WORKSPACE),
		});
		await settle();
		await settle();
		expect(rows(tree.state)).toEqual(['/w/docs', '/w/docs/d.md', '/w/src', '/w/a.txt']);

		await modelOf(tree).expand('/w/src');
		await modelOf(tree).expand('/w/src/lib');
		expect([...(saved.at(-1) ?? [])].sort()).toEqual(['/w/docs', '/w/src', '/w/src/lib']);
		modelOf(tree).collapseAll();
		await settle();
		expect(saved.at(-1)).toEqual([]);

		// A reveal expands every folder above the row in one operation, which saves once.
		const writes = saved.length;
		modelOf(tree).reveal('/w/src/lib/c.ts');
		await settle();
		expect(saved.length).toBe(writes + 1);
		expect([...(saved.at(-1) ?? [])].sort()).toEqual(['/w/src', '/w/src/lib']);
		tree.dispose();
	});

	it("joins a folder that is its parent's only child into the parent's row, but never a file", async () => {
		const port = new MemoryFileSessionPort();
		port.writeFile('/w/a/b/c/d.ts', 'd');
		port.writeFile('/w/e/f.ts', 'f');
		port.setRoot(WORKSPACE, '/w');
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: unexpectedReport,
			session: port.open(WORKSPACE),
		});
		await settle();
		await modelOf(tree).expand('/w/a');
		await modelOf(tree).expand('/w/e');
		await settle();

		const model = modelOf(tree);
		const names = Array.from({ length: model.rowCount }, (_, index) =>
			model
				.rowAt(index)
				.segments.map((segment) => segment.element.name)
				.join('/'),
		);
		expect(names).toEqual(['a/b/c', 'd.ts', 'e', 'f.ts']);
		tree.dispose();
	});

	it("lists a compact row's folders in one request and holds them while the tree shows", async () => {
		const port = machine();
		port.writeFile('/w/deep/a/b/c.txt', 'c');
		port.writeFile('/w/repo/.git/HEAD', 'h');
		port.writeFile('/w/repo/.git/config', 'c');
		const { listed, released, session } = observe(port.open(WORKSPACE));
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: unexpectedReport,
			session,
		});
		await settle();
		await modelOf(tree).expand('/w/deep');
		await modelOf(tree).expand('/w/repo');
		await settle();

		expect(listed).toEqual(['/w', '/w/deep', '/w/repo']);
		expect(rows(tree.state).slice(0, 4)).toEqual([
			'/w/deep',
			'/w/deep/a',
			'/w/deep/a/b',
			'/w/deep/a/b/c.txt',
		]);
		// The Explorer hides the lone `.git` that listed with its folder, so the Server keeps nothing.
		expect(released).toEqual([['/w/repo/.git']]);

		port.setRoot(WORKSPACE, '/w/docs');
		await settle();
		expect(released.at(-1)).toEqual(['/w', '/w/deep', '/w/deep/a', '/w/deep/a/b', '/w/repo']);
		tree.dispose();
	});

	it('lists a folder of a compact row again when a change in it overtook the row’s listing', async () => {
		const port = machine('manual');
		port.writeFile('/w/deep/a/c.txt', 'c');
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: unexpectedReport,
			session: port.open(WORKSPACE),
		});
		await settle();
		port.flush();
		await settle();
		const expanding = modelOf(tree).expand('/w/deep');
		await settle();

		// The Server lists the folder with the folder below it and then reports a change there,
		// which arrives before the listing does.
		port.flush();
		port.writeFile('/w/deep/a/e.txt', 'e');
		await settle();
		port.flush();
		await expanding;
		await settle();

		expect(rows(tree.state).slice(0, 4)).toEqual([
			'/w/deep',
			'/w/deep/a',
			'/w/deep/a/c.txt',
			'/w/deep/a/e.txt',
		]);
		tree.dispose();
	});

	it('shows a folder it cannot list as empty and reports why', async () => {
		const port = machine();
		const reports: string[] = [];
		const { session } = observe(
			port.open(WORKSPACE),
			new Map([['/w/src', { code: 'permission-denied' }]]),
		);
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: (name, error) => reports.push(`${name}: ${error.code}`),
			session,
		});
		await settle();
		await modelOf(tree).expand('/w/src');

		expect(rows(tree.state)).toEqual(['/w/docs', '/w/src', '/w/a.txt']);
		expect(modelOf(tree).rowAt(1).collapsed).toBe(false);
		expect(reports).toEqual(['src: permission-denied']);
		tree.dispose();
	});

	it('lists the parent of a folder found gone, and reports it once the parent still lists it', async () => {
		vi.useFakeTimers();
		const port = machine();
		const reports: string[] = [];
		const { listed, session } = observe(
			port.open(WORKSPACE),
			new Map([['/w/src', { code: 'not-found' }]]),
		);
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: (name, error) => reports.push(`${name}: ${error.code}`),
			session,
		});
		await settle();
		await modelOf(tree).expand('/w/src');
		expect(reports).toEqual([]);
		listed.length = 0;

		await vi.advanceTimersByTimeAsync(500);
		await settle();
		expect(listed).toEqual(['/w', '/w/src']);
		expect(reports).toEqual(['src: not-found']);

		await vi.advanceTimersByTimeAsync(5000);
		expect(listed).toEqual(['/w', '/w/src']);
		tree.dispose();
	});

	it('shows progress only once a listing has run 1500 ms for the first root and 800 ms later', async () => {
		vi.useFakeTimers();
		const port = machine('manual');
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: unexpectedReport,
			session: port.open(WORKSPACE),
		});
		await vi.advanceTimersByTimeAsync(1499);
		expect(tree.progress).toBe(false);
		await vi.advanceTimersByTimeAsync(1);
		expect(tree.progress).toBe(true);
		port.flush();
		await settle();
		expect(tree.progress).toBe(false);

		const expanding = modelOf(tree).expand('/w/src');
		await vi.advanceTimersByTimeAsync(799);
		expect(tree.progress).toBe(false);
		await vi.advanceTimersByTimeAsync(1);
		expect(tree.progress).toBe(true);
		port.flush();
		await expanding;
		expect(tree.progress).toBe(false);
		tree.dispose();
	});

	it('keeps the tree of a root that went missing and lists it again when it returns', async () => {
		vi.useFakeTimers();
		const port = machine();
		const tree = new ExplorerTree({
			locale: 'en-US',
			memory: new ExplorerViewMemory(null),
			reportUnreadable: unexpectedReport,
			session: port.open(WORKSPACE),
		});
		await settle();
		await modelOf(tree).expand('/w/src');

		port.remove('/w');
		await vi.advanceTimersByTimeAsync(1000);
		expect(tree.state).toEqual({ error: null, kind: 'unresolved', name: 'w', root: '/w' });

		port.writeFile('/w/src/b.ts', 'b');
		await vi.advanceTimersByTimeAsync(0);
		await settle();
		expect(rows(tree.state)).toEqual(['/w/src', '/w/src/b.ts']);
		tree.dispose();
	});
});
