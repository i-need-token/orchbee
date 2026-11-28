import { afterEach, describe, expect, it, vi } from 'vitest';

import { type FileWorkspaceSession, MemoryFileSessionPort } from '../../../file-session';
import { ExplorerTree, type ExplorerTreeState } from '../ExplorerTree';

const WORKSPACE = 'workspace';

/** A session whose listings and releases the test can count. */
function observe(session: FileWorkspaceSession): {
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
				list: (request, signal) => {
					if (request.cursor === undefined) listed.push(request.path);
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

function rows(state: ExplorerTreeState): readonly string[] {
	if (state.kind !== 'tree') return [];
	const { model } = state;
	return Array.from({ length: model.rowCount }, (_, index) => model.rowAt(index).key);
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
		const tree = new ExplorerTree({ locale: 'en-US', session });
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
		const tree = new ExplorerTree({ locale: 'en-US', session });
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

	it('shows a new root once it has loaded, and only the latest one', async () => {
		const port = machine('manual');
		const { released, session } = observe(port.open(WORKSPACE));
		const tree = new ExplorerTree({ locale: 'en-US', session });
		await settle();
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

	it('keeps the tree of a root that went missing and lists it again when it returns', async () => {
		vi.useFakeTimers();
		const port = machine();
		const tree = new ExplorerTree({ locale: 'en-US', session: port.open(WORKSPACE) });
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
