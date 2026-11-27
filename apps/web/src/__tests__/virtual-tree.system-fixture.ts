import '@orchbee/ui/styles.css';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import {
	VirtualTree,
	VirtualTreeModel,
	type VirtualTreeProps,
	type VirtualTreeRow,
	type VirtualTreeSource,
} from '../virtual-tree';

// The render benchmark's page: a tree over the reference scenario's folder of 100,000 files, and the
// measurements `system-tests/virtual-tree.spec.ts` runs in it. A measurement runs in the page, so
// timers read the main thread directly: script time ends once React's synchronous render commits,
// and a frame's rendering ends when a task posted from its animation frame callback runs.

interface FixtureItem {
	readonly key: string;
	readonly label: string;
	readonly children: readonly FixtureItem[] | null;
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
const input: FixtureItem = {
	children: [
		{ children: files, key: LARGE_FOLDER, label: LARGE_FOLDER },
		{ children: null, key: 'package.json', label: 'package.json' },
		{ children: null, key: 'README.md', label: 'README.md' },
	],
	key: '',
	label: '',
};
const source: VirtualTreeSource<string, FixtureItem> = {
	children: (item) => item.children ?? [],
	hasChildren: (item) => item.children !== null,
	key: (item) => item.key,
};
const model = new VirtualTreeModel({ input, source });
void model.refresh(null);

const host = document.createElement('div');
host.id = 'virtual-tree-host';
host.style.height = `${TREE_HEIGHT}px`;
host.style.width = `${TREE_WIDTH}px`;
document.body.replaceChildren(host);
document.body.style.margin = '0';

const props: VirtualTreeProps<string, FixtureItem> = {
	label: 'Benchmark tree',
	model,
	renderRow: (row: VirtualTreeRow<string, FixtureItem>) => row.element.label,
};
createRoot(host).render(createElement(VirtualTree<string, FixtureItem>, props));

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

Object.defineProperty(window, 'virtualTreeFixture', {
	value: Object.freeze({
		measureExpand,
		measureScroll,
		measureSelectAll,
		scrollTop: () => model.scrollTop,
		setFocus: (key: string | null) => {
			model.setFocus(key);
		},
		setScrollTop: (offset: number) => {
			model.setScrollTop(offset);
		},
	}),
});
