import '@orchbee/ui/styles.css';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import { PointerDragKind } from '../pointer-drag';
import {
	VirtualTree,
	VirtualTreeModel,
	VirtualTreeSegments,
	type VirtualTreeContextMenuEvent,
	type VirtualTreeDragAndDrop,
	type VirtualTreeOpenEvent,
	type VirtualTreeProps,
	type VirtualTreeRow,
	type VirtualTreeSource,
} from '../virtual-tree';

// The render benchmark's page: a tree over the reference scenario's folder of 100,000 files, and the
// measurements `system-tests/virtual-tree.spec.ts` runs in it. A measurement runs in the page, so
// timers read the main thread directly: script time ends once React's synchronous render commits,
// and a frame's rendering ends when a task posted from its animation frame callback runs. The page
// also records the rows the tree asks to open, the context menus it asks for and the drops it takes,
// which the interaction checks read. Its rows drag as the Explorer's do: a folder takes a drop and
// expands under a resting drag, a file hands the drop to its folder, and no row takes itself. With
// `?tree=compact`, the page shows a small tree whose folders join into compact rows, as VS Code's
// Explorer shows compact folders, and starts with every folder expanded.

interface FixtureItem {
	readonly key: string;
	readonly label: string;
	readonly children: readonly FixtureItem[] | null;
}

const compact = new URLSearchParams(location.search).get('tree') === 'compact';

function fixtureItem(key: string, children: readonly FixtureItem[] | null = null): FixtureItem {
	return { children, key, label: key.slice(key.lastIndexOf('/') + 1) };
}

const LARGE_FOLDER = 'folder';
const LARGE_FOLDER_SIZE = 100_000;
const TREE_HEIGHT = 600;
const TREE_WIDTH = 320;

const files: FixtureItem[] = Array.from({ length: LARGE_FOLDER_SIZE }, (_, index) => ({
	children: null,
	key: `${LARGE_FOLDER}/file-${index}.ts`,
	label: `file-${index}.ts`,
}));
const input = compact
	? fixtureItem('', [
			fixtureItem('src', [
				fixtureItem('src/app', [
					fixtureItem('src/app/lib', [
						fixtureItem('src/app/lib/index.ts'),
						fixtureItem('src/app/lib/util.ts'),
					]),
				]),
			]),
			fixtureItem('package.json'),
		])
	: fixtureItem('', [
			fixtureItem(LARGE_FOLDER, files),
			fixtureItem('package.json'),
			fixtureItem('README.md'),
		]);
const isFolder = (item: FixtureItem) => item.children !== null;
const source: VirtualTreeSource<string, FixtureItem> = {
	children: (item) => item.children ?? [],
	hasChildren: isFolder,
	key: (item) => item.key,
	// Folders join compact rows, as VS Code's Explorer keeps files out of them.
	...(compact ? { compressible: isFolder } : {}),
};
const model = new VirtualTreeModel({
	input,
	source,
	...(compact ? { expandedByDefault: isFolder } : {}),
});
void model.refresh(null);

const host = document.createElement('div');
host.id = 'virtual-tree-host';
host.style.height = `${TREE_HEIGHT}px`;
host.style.width = `${TREE_WIDTH}px`;
document.body.replaceChildren(host);
document.body.style.margin = '0';

interface TreeRequest {
	readonly kind: 'contextMenu' | 'open';
	/** The row's key; null for a context menu on the empty space. */
	readonly key: string | null;
	/** The key of the item a context menu is for, which a compact row picks among its items. */
	readonly segment?: string | null;
	readonly pinned?: boolean;
	readonly preserveFocus?: boolean;
	readonly sideBySide?: boolean;
	/**
	 * Where a context menu would open, relative to the bottom-left corner of the row, or of the name
	 * of the item it is for in a compact row.
	 */
	readonly offset?: { readonly x: number; readonly y: number };
}

const requests: TreeRequest[] = [];

function recordOpen({
	pinned,
	preserveFocus,
	row,
	sideBySide,
}: VirtualTreeOpenEvent<string, FixtureItem>): void {
	requests.push({ key: row.key, kind: 'open', pinned, preserveFocus, sideBySide });
}

function recordContextMenu({
	browserEvent,
	row,
	segment,
}: VirtualTreeContextMenuEvent<string, FixtureItem>): void {
	const index = row === null ? -1 : model.indexOf(row.key);
	const rowElement = index < 0 ? null : document.querySelector(`[data-row-index="${index}"]`);
	const position = row === null || segment === null ? -1 : row.segments.indexOf(segment);
	const anchor =
		row !== null && row.segments.length > 1
			? rowElement?.querySelector(`[data-tree-segment="${position}"]`)
			: rowElement;
	const box = anchor?.getBoundingClientRect();
	requests.push({
		key: row?.key ?? null,
		kind: 'contextMenu',
		segment: segment?.key ?? null,
		...(box === undefined
			? {}
			: {
					// Chromium truncates a constructed event's coordinates, so the fraction of a pixel by
					// which an item's name starts rounds away; adding 0 drops the sign it leaves.
					offset: {
						x: Math.round(browserEvent.clientX - box.left) + 0,
						y: Math.round(browserEvent.clientY - box.bottom) + 0,
					},
				}),
	});
}

/** The keys of the rows a drag from the tree carries. */
const draggedRows = new PointerDragKind<readonly string[]>();

interface TreeDrop {
	readonly keys: readonly string[];
	/** The row the drag was released on; null for the tree itself. */
	readonly target: string | null;
	readonly ctrlKey: boolean;
}

const drops: TreeDrop[] = [];
/** How many rows the latest drag from the tree carried. */
let draggedCount = 0;

const dragAndDrop: VirtualTreeDragAndDrop<string, FixtureItem> = {
	drag: (rows) => {
		draggedCount = rows.length;
		const [first] = rows;
		const label = rows.length === 1 && first !== undefined ? first.element.label : `${rows.length}`;
		return draggedRows.carry(
			rows.map((row) => row.key),
			label,
		);
	},
	drop: (drag, row) => {
		drops.push({
			ctrlKey: drag.modifiers.ctrlKey,
			keys: draggedRows.read(drag) ?? [],
			target: row?.key ?? null,
		});
	},
	over: (drag, row) => {
		const keys = draggedRows.read(drag);
		if (keys === undefined || (row !== null && keys.includes(row.key))) return null;
		if (row === null) return {};
		return row.collapsible ? { autoExpand: true } : { bubble: 'up' };
	},
};

/** A row's items' names, outermost first: the row's own alone outside a compact row. */
const namesOf = (row: VirtualTreeRow<string, FixtureItem>) =>
	row.segments.map((segment) => segment.element.label);

const props: VirtualTreeProps<string, FixtureItem> = {
	dragAndDrop,
	// VS Code's Explorer joins a compact row's names for type navigation.
	keyboardNavigationLabel: compact ? (row) => namesOf(row).join('/') : (row) => row.element.label,
	label: 'Benchmark tree',
	model,
	onContextMenu: recordContextMenu,
	onOpen: recordOpen,
	platform: /^(?:Mac|iPhone|iPad|iPod)/u.test(navigator.platform) ? 'apple' : 'standard',
	renderRow: compact
		? (row, state) => createElement(VirtualTreeSegments, { names: namesOf(row), state })
		: (row) => row.element.label,
};
// The application's English messages, as the product names a compact row's items with them.
createRoot(host).render(
	createElement(
		OrchbeeI18nProvider,
		{ locale: 'en-US' },
		createElement(VirtualTree<string, FixtureItem>, props),
	),
);

interface FrameTiming {
	/** Main-thread time from the start of the frame's animation frame callbacks until it painted. */
	readonly renderMs: number;
	readonly paintedAt: number;
}

/** The time once every microtask queued so far, React's synchronous render among them, has run. */
function afterMicrotasks(): Promise<number> {
	return new Promise((resolve) => {
		queueMicrotask(() => {
			resolve(performance.now());
		});
	});
}

/** Runs `then` once the frame being rendered has painted. */
function afterPaint(then: () => void): void {
	const channel = new MessageChannel();
	channel.port1.addEventListener('message', () => {
		channel.port1.close();
		then();
	});
	channel.port1.start();
	channel.port2.postMessage(null);
}

/** Waits for the next frame to paint. */
function nextPaint(): Promise<FrameTiming> {
	return new Promise((resolve) => {
		requestAnimationFrame(() => {
			const frameStart = performance.now();
			afterPaint(() => {
				const paintedAt = performance.now();
				resolve({ paintedAt, renderMs: paintedAt - frameStart });
			});
		});
	});
}

/** Lets ten frames pass, so a measurement starts from a still page. */
function settle(frames = 10): Promise<void> {
	return frames === 0 ? Promise.resolve() : nextPaint().then(() => settle(frames - 1));
}

function percentile(values: readonly number[], fraction: number): number {
	const sorted = [...values].sort((left, right) => left - right);
	return sorted[Math.min(sorted.length - 1, Math.floor(fraction * sorted.length))] ?? 0;
}

function summary(values: readonly number[]) {
	return {
		max: Math.max(...values),
		median: percentile(values, 0.5),
		p95: percentile(values, 0.95),
	};
}

/** Expands the folder of 100,000 files from collapsed and times it to its first painted frame. */
async function measureExpand() {
	model.collapse(LARGE_FOLDER);
	model.setScrollTop(0);
	await settle();
	const start = performance.now();
	void model.expand(LARGE_FOLDER);
	const committed = await afterMicrotasks();
	const frame = await nextPaint();
	return {
		renderMs: frame.renderMs,
		rowCount: model.rowCount,
		scriptMs: committed - start,
		// Wall-clock time to the paint, including the wait for the next frame to begin.
		toPaintMs: frame.paintedAt - start,
		workMs: committed - start + frame.renderMs,
	};
}

interface ScrollFrame {
	/** From the frame's callback to React's commit of the new offset. */
	readonly scriptMs: number;
	/** From the frame's callback until the frame painted. */
	readonly frameMs: number;
	/** Since the previous frame began; NaN for the first. */
	readonly intervalMs: number;
}

/** Scrolls by `pixelsPerFrame` in each of `frames` consecutive frames. */
function scrollFrames(pixelsPerFrame: number, frames: number): Promise<readonly ScrollFrame[]> {
	return new Promise((resolve) => {
		const samples: ScrollFrame[] = [];
		let previous = Number.NaN;
		const step = (timestamp: number) => {
			const intervalMs = timestamp - previous;
			previous = timestamp;
			const start = performance.now();
			model.setScrollTop(model.scrollTop + pixelsPerFrame);
			queueMicrotask(() => {
				const scriptMs = performance.now() - start;
				afterPaint(() => {
					samples.push({ frameMs: performance.now() - start, intervalMs, scriptMs });
					if (samples.length === frames) resolve(samples);
					else requestAnimationFrame(step);
				});
			});
		};
		requestAnimationFrame(step);
	});
}

/** Scrolls the expanded folder at a fixed number of pixels per frame and times every frame. */
async function measureScroll(pixelsPerFrame: number, frames: number) {
	await settle();
	const longTasks: number[] = [];
	const observer = new PerformanceObserver((list) => {
		for (const entry of list.getEntries()) longTasks.push(entry.duration);
	});
	observer.observe({ type: 'longtask' });
	const samples = await scrollFrames(pixelsPerFrame, frames);
	observer.disconnect();
	return {
		frameIntervalMs: summary(samples.slice(1).map((sample) => sample.intervalMs)),
		frameMs: summary(samples.map((sample) => sample.frameMs)),
		frames,
		longTasks: longTasks.length,
		pixelsPerFrame,
		scriptMs: summary(samples.map((sample) => sample.scriptMs)),
	};
}

/** Selects all 100,000 files of the expanded folder and times it to the painted selection. */
async function measureSelectAll() {
	model.setSelection([]);
	await settle();
	const start = performance.now();
	model.selectAll();
	const committed = await afterMicrotasks();
	const frame = await nextPaint();
	return {
		renderMs: frame.renderMs,
		scriptMs: committed - start,
		selected: model.selection.size,
		toPaintMs: frame.paintedAt - start,
		workMs: committed - start + frame.renderMs,
	};
}

/** Presses a key on the tree element, as the keyboard does while the tree has focus. */
function pressKey(code: string, key: string, primaryModifier = false): void {
	host.querySelector('[role="tree"]')?.dispatchEvent(
		new KeyboardEvent('keydown', {
			bubbles: true,
			cancelable: true,
			code,
			ctrlKey: primaryModifier && props.platform === 'standard',
			key,
			metaKey: primaryModifier && props.platform === 'apple',
		}),
	);
}

/**
 * Selects the expanded folder's 100,000 files with the select-all key pressed on one of them, as
 * the tree's keyboard handling runs it, and times it to the painted selection.
 */
async function measureSelectSiblings() {
	await model.expand(LARGE_FOLDER);
	model.setFocus(`${LARGE_FOLDER}/file-50000.ts`);
	model.setSelection([]);
	await settle();
	const start = performance.now();
	pressKey('KeyA', 'a', true);
	const committed = await afterMicrotasks();
	const frame = await nextPaint();
	return {
		renderMs: frame.renderMs,
		scriptMs: committed - start,
		selected: model.selection.size,
		toPaintMs: frame.paintedAt - start,
		workMs: committed - start + frame.renderMs,
	};
}

/**
 * Starts a drag from one of 100,003 selected rows, which carries them all, and times the pointer
 * move that starts it, then cancels it.
 */
async function measureDragStart() {
	await model.expand(LARGE_FOLDER);
	model.setScrollTop(0);
	model.selectAll();
	await settle();
	const row = host.querySelector('[data-row-index="1"]');
	if (row === null) throw new Error('The first file has no row.');
	const box = row.getBoundingClientRect();
	const pointer = (type: string, clientX: number, buttons: number) =>
		new PointerEvent(type, {
			bubbles: true,
			buttons,
			cancelable: true,
			clientX,
			clientY: box.top + box.height / 2,
			isPrimary: true,
			pointerId: 2,
		});
	row.dispatchEvent(pointer('pointerdown', box.left + 40, 1));
	const start = performance.now();
	row.dispatchEvent(pointer('pointermove', box.left + 60, 1));
	const scriptMs = performance.now() - start;
	const frame = await nextPaint();
	row.dispatchEvent(pointer('pointercancel', box.left + 60, 0));
	return { carried: draggedCount, renderMs: frame.renderMs, scriptMs };
}

/** Types a character no row's label starts with, which reads every label once, and times the key. */
async function measureTypeNavigation() {
	await model.expand(LARGE_FOLDER);
	await settle();
	const start = performance.now();
	pressKey('KeyZ', 'z');
	return { rowCount: model.rowCount, scriptMs: performance.now() - start };
}

Object.defineProperty(window, 'virtualTreeFixture', {
	value: Object.freeze({
		measureDragStart,
		measureExpand,
		measureScroll,
		measureSelectAll,
		measureSelectSiblings,
		measureTypeNavigation,
		collapse: (key: string) => {
			model.collapse(key);
		},
		scrollTop: () => model.scrollTop,
		setFocus: (key: string | null) => {
			model.setFocus(key);
		},
		setScrollTop: (offset: number) => {
			model.setScrollTop(offset);
		},
		state: () => ({
			anchor: model.anchor,
			focus: model.focus,
			rowCount: model.rowCount,
			selection: [...model.selection].sort(),
		}),
		takeDrops: () => drops.splice(0),
		takeRequests: () => requests.splice(0),
	}),
});
