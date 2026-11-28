import { describe, expect, it, vi } from 'vitest';

import {
	createFileOverlayState,
	transitionFileOverlay,
	type FileOverlayEvent,
	type FileOverlayState,
} from '../file-overlay-state';
import {
	forgetFileOverlayTabs,
	readFileOverlayTabs,
	writeFileOverlayTabs,
} from '../file-overlay-tabs-storage';

const KEY = 'orchbee:file-overlay:tabs:v1';

function workspaceId(index: number): string {
	return index.toString(16).padStart(32, '0');
}

function memoryStorage(initial: string | null = null) {
	let stored = initial;
	const setItem = vi.fn<(key: string, value: string) => void>((key, value) => {
		if (key === KEY) stored = value;
	});
	return {
		storage: { getItem: (key: string) => (key === KEY ? stored : null), setItem },
		value: () => stored,
	};
}

type Storage = ReturnType<typeof memoryStorage>['storage'];

function opened(path: string, preview = false): FileOverlayEvent {
	return {
		input: { kind: 'file', resource: { key: path.toLowerCase(), path } },
		preview,
		type: 'file-opened',
	};
}

/** Applies each event as a Workspace's overlay does, keeping every change the way the registry does. */
function changed(
	storage: Storage,
	id: string,
	events: readonly FileOverlayEvent[],
	from: FileOverlayState = createFileOverlayState(true),
): FileOverlayState {
	let state = from;
	for (const event of events) {
		const next = transitionFileOverlay(state, event).state;
		writeFileOverlayTabs(storage, id, next, state);
		state = next;
	}
	return state;
}

/** A record another run could have left: one Workspace's overlay with two tabs. */
const WELL_FORMED = {
	activeIndex: 0,
	mode: 'open',
	previewIndex: null,
	restoreTier: 'default',
	stickyCount: 0,
	tabs: [
		{ key: '/w/a.ts', path: '/w/A.ts' },
		{ key: '/w/b.ts', path: '/w/B.ts' },
	],
	tier: 'full',
	workspaceId: workspaceId(1),
};

function recordWith(change: object): string {
	return JSON.stringify({ workspaces: [{ ...WELL_FORMED, ...change }] });
}

function tabIds(state: FileOverlayState | null): readonly string[] | undefined {
	return state?.tabs.tabs.map((tab) => tab.id);
}

describe('file overlay tabs storage', () => {
	it("brings back each Workspace's row, active, preview and sticky tabs and tier, shrunk", () => {
		const { storage } = memoryStorage();
		const id = workspaceId(1);
		changed(storage, id, [
			opened('/w/A.ts'),
			opened('/w/B.ts'),
			{ id: 'file:/w/b.ts', type: 'tab-stuck' },
			opened('/w/C.ts', true),
			{ id: 'file:/w/a.ts', type: 'tab-activated' },
			{ id: 'file:/w/a.ts', type: 'unsaved-changed', unsaved: true },
			{ tier: 'left-expand', type: 'tier-chosen' },
			{ type: 'full-toggled' },
		]);
		changed(storage, workspaceId(2), [opened('/x/D.ts')]);

		const restored = readFileOverlayTabs(storage, id, true);

		// The preview opened next to B, the active tab then, as VS Code opens editors.
		expect(tabIds(restored)).toEqual(['file:/w/b.ts', 'file:/w/c.ts', 'file:/w/a.ts']);
		expect(restored?.tabs.tabs.map((tab) => tab.input.resource.path)).toEqual([
			'/w/B.ts',
			'/w/C.ts',
			'/w/A.ts',
		]);
		expect(restored?.tabs).toMatchObject({
			activeId: 'file:/w/a.ts',
			closed: [],
			pendingClose: null,
			previewId: 'file:/w/c.ts',
			stickyCount: 1,
		});
		// Content is not kept, so no tab comes back with unsaved edits.
		expect(restored?.tabs.tabs.some((tab) => tab.unsaved)).toBe(false);
		// The terminal takes the keys as a Workspace first shows, so an open overlay comes back shrunk.
		expect(restored?.mode).toEqual({
			kind: 'pip',
			leftExpandOffered: true,
			restoreTier: 'left-expand',
			tier: 'full',
		});
		expect(tabIds(readFileOverlayTabs(storage, workspaceId(2), true))).toEqual(['file:/x/d.ts']);
		expect(readFileOverlayTabs(storage, workspaceId(3), true)).toBeNull();
	});

	it('restores a left-expand tier at the default tier in a window that does not offer it', () => {
		const { storage } = memoryStorage();
		const id = workspaceId(1);
		changed(storage, id, [opened('/w/A.ts'), { tier: 'left-expand', type: 'tier-chosen' }]);

		expect(readFileOverlayTabs(storage, id, false)?.mode).toMatchObject({
			kind: 'pip',
			tier: 'default',
		});
		expect(readFileOverlayTabs(storage, id, true)?.mode).toMatchObject({ tier: 'left-expand' });
	});

	it('brings shrunk overlays back shrunk and lets closed ones leave the record', () => {
		const { storage, value } = memoryStorage();
		const id = workspaceId(1);
		const shown = changed(storage, id, [opened('/w/A.ts'), { type: 'terminal-replaced' }]);
		expect(readFileOverlayTabs(storage, id, true)?.mode.kind).toBe('pip');

		changed(storage, id, [{ target: { kind: 'overlay' }, type: 'close-requested' }], shown);

		expect(readFileOverlayTabs(storage, id, true)).toBeNull();
		expect(value()).toBe('{"workspaces":[]}');
	});

	it('writes nothing for a change it does not keep', () => {
		const { storage } = memoryStorage();
		const id = workspaceId(1);
		const shown = changed(storage, id, [opened('/w/A.ts')]);
		storage.setItem.mockClear();

		changed(
			storage,
			id,
			[
				{ id: 'file:/w/a.ts', type: 'unsaved-changed', unsaved: true },
				{ id: 'file:/w/a.ts', type: 'unsaved-changed', unsaved: false },
				{ id: 'file:/w/a.ts', type: 'tab-activated' },
			],
			shown,
		);

		expect(storage.setItem).not.toHaveBeenCalled();
	});

	it('keeps the most recently active tabs and the most recently changed Workspaces within bounds', () => {
		const { storage, value } = memoryStorage();
		const paths = Array.from({ length: 201 }, (_, index) => `/w/${index}.ts`);
		changed(
			storage,
			workspaceId(0),
			paths.map((path) => opened(path)),
		);

		const restored = readFileOverlayTabs(storage, workspaceId(0), true);
		expect(restored?.tabs.tabs).toHaveLength(200);
		expect(restored?.tabs.tabs[0]?.input.resource.path).toBe('/w/1.ts');
		expect(restored?.tabs.activeId).toBe('file:/w/200.ts');

		for (let index = 1; index <= 50; index += 1) {
			changed(storage, workspaceId(index), [opened('/w/A.ts')]);
		}
		expect(readFileOverlayTabs(storage, workspaceId(0), true)).toBeNull();
		expect(readFileOverlayTabs(storage, workspaceId(1), true)).not.toBeNull();
		expect(readFileOverlayTabs(storage, workspaceId(50), true)).not.toBeNull();

		// One Workspace too long for the record on its own leaves the others where they were.
		const long = `/w/${'x'.repeat(3000)}`;
		changed(
			storage,
			workspaceId(51),
			paths.slice(0, 100).map((path) => opened(`${long}${path}`)),
		);
		expect(readFileOverlayTabs(storage, workspaceId(51), true)).toBeNull();
		expect(readFileOverlayTabs(storage, workspaceId(50), true)).not.toBeNull();
		expect(value()?.length).toBeLessThanOrEqual(500_000);
	});

	it('forgets the Workspaces it is told no longer exist', () => {
		const { storage } = memoryStorage();
		changed(storage, workspaceId(1), [opened('/w/A.ts')]);
		changed(storage, workspaceId(2), [opened('/x/B.ts')]);

		forgetFileOverlayTabs(storage, [workspaceId(1)]);

		expect(readFileOverlayTabs(storage, workspaceId(1), true)).toBeNull();
		expect(tabIds(readFileOverlayTabs(storage, workspaceId(2), true))).toEqual(['file:/x/b.ts']);
	});

	it.each([
		['text that is not JSON', 'not-json'],
		['null', 'null'],
		['an extra field', JSON.stringify({ extra: true, workspaces: [WELL_FORMED] })],
		['an extra field in a Workspace', recordWith({ extra: true })],
		['a Workspace id outside its shape', recordWith({ workspaceId: 'W1' })],
		['no tabs', recordWith({ tabs: [] })],
		['an active tab past the row', recordWith({ activeIndex: 2 })],
		['a preview among the sticky tabs', recordWith({ previewIndex: 0, stickyCount: 1 })],
		['more sticky tabs than tabs', recordWith({ stickyCount: 3 })],
		['a closed mode', recordWith({ mode: 'closed' })],
		['an empty path', recordWith({ tabs: [{ key: '', path: '' }] })],
		[
			'one file twice',
			recordWith({
				tabs: [
					{ key: '/w/a.ts', path: '/w/A.ts' },
					{ key: '/w/a.ts', path: '/w/a.ts' },
				],
			}),
		],
		[
			'a path past the longest the backend sends',
			recordWith({ tabs: [{ key: 'a', path: 'a'.repeat(4097) }] }),
		],
	])('starts the overlay closed from a record with %s', (_name, record) => {
		expect(readFileOverlayTabs(memoryStorage(record).storage, workspaceId(1), true)).toBeNull();
	});

	it('restores a well-formed record and survives storage that fails', () => {
		const id = workspaceId(1);
		const record = recordWith({ activeIndex: 1, mode: 'pip', previewIndex: 1, stickyCount: 1 });
		expect(readFileOverlayTabs(memoryStorage(record).storage, id, true)).toMatchObject({
			mode: { kind: 'pip', restoreTier: 'default', tier: 'full' },
			tabs: { activeId: 'file:/w/b.ts', previewId: 'file:/w/b.ts', stickyCount: 1 },
		});

		const failing = {
			getItem: (): string | null => {
				throw new Error('Storage is unavailable');
			},
			setItem: (): void => {
				throw new Error('Storage is full');
			},
		};
		expect(readFileOverlayTabs(failing, id, true)).toBeNull();
		const before = createFileOverlayState(true);
		const after = transitionFileOverlay(before, opened('/w/A.ts')).state;
		expect(() => writeFileOverlayTabs(failing, id, after, before)).not.toThrow();
		expect(() => forgetFileOverlayTabs(failing, [id])).not.toThrow();
	});
});
