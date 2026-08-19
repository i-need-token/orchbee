import { describe, expect, it } from 'vitest';

import { HistoryScrollPresentationTransaction } from '../HistoryScrollPresentationTransaction';

interface MotionHarness {
	readonly canvas: HTMLCanvasElement;
	readonly frames: Map<number, FrameRequestCallback>;
	readonly root: HTMLElement;
	flush(timestamp: number): void;
}

function createMotionHarness(): MotionHarness {
	let nextFrame = 1;
	const frames = new Map<number, FrameRequestCallback>();
	const children: HTMLCanvasElement[] = [];
	const view = {
		cancelAnimationFrame: (handle: number) => frames.delete(handle),
		requestAnimationFrame: (callback: FrameRequestCallback) => {
			const handle = nextFrame++;
			frames.set(handle, callback);
			return handle;
		},
	} as unknown as Window;
	const document = {
		createElement: () => createCanvas(document as unknown as Document, children),
		defaultView: view,
	} as unknown as Document;
	const canvas = createCanvas(document, children);
	canvas.width = 720;
	canvas.height = 360;
	const root = {
		appendChild: (child: HTMLCanvasElement) => {
			children.push(child);
			return child;
		},
		getBoundingClientRect: () => rectangle(0, 0, 720, 360),
		ownerDocument: document,
	} as unknown as HTMLElement;
	return {
		canvas,
		frames,
		flush: (timestamp) => {
			const callbacks = [...frames.values()];
			frames.clear();
			for (const callback of callbacks) callback(timestamp);
		},
		root,
	};
}

function createCanvas(document: Document, children: HTMLCanvasElement[]): HTMLCanvasElement {
	const style: Record<string, string> = {
		transform: '',
		transformOrigin: '',
		willChange: '',
	};
	const canvas = {
		dataset: {},
		getBoundingClientRect: () => rectangle(0, 0, 720, 360),
		getContext: () => ({ drawImage: () => undefined }),
		height: 0,
		ownerDocument: document,
		remove: () => {
			const index = children.indexOf(canvas as unknown as HTMLCanvasElement);
			if (index >= 0) children.splice(index, 1);
		},
		setAttribute: () => undefined,
		style,
		width: 0,
	} as unknown as HTMLCanvasElement;
	return canvas;
}

function rectangle(left: number, top: number, width: number, height: number): DOMRect {
	return {
		bottom: top + height,
		height,
		left,
		right: left + width,
		top,
		width,
		x: left,
		y: top,
		toJSON: () => ({}),
	};
}

describe('HistoryScrollPresentationTransaction', () => {
	it('projects bounded motion in both directions and settles on the committed canvas', () => {
		const harness = createMotionHarness();
		const transaction = new HistoryScrollPresentationTransaction(harness.root, harness.canvas);

		expect(
			transaction.prepare({
				fromTop: 10n,
				reducedMotion: false,
				rowHeight: 20,
				toTop: 9n,
				visibleRows: 2,
			}),
		).toBe(true);
		expect(transaction.commit()).toBe(true);
		expect(harness.canvas.style.transform).toBe('translateY(-20px)');
		harness.flush(0);
		harness.flush(50);
		expect(Number.parseFloat(harness.canvas.style.transform.slice(11))).toBeLessThan(0);
		harness.flush(100);
		expect(harness.canvas.style.transform).toBe('');

		expect(
			transaction.prepare({
				fromTop: 9n,
				reducedMotion: false,
				rowHeight: 20,
				toTop: 10n,
				visibleRows: 2,
			}),
		).toBe(true);
		expect(transaction.commit()).toBe(true);
		expect(harness.canvas.style.transform).toBe('translateY(20px)');
		transaction.cancel();
		expect(harness.canvas.style.transform).toBe('');
		expect(harness.frames.size).toBe(0);
	});

	it('keeps reduced, invalid, and over-viewport motion on the canonical frame', () => {
		const harness = createMotionHarness();
		const transaction = new HistoryScrollPresentationTransaction(harness.root, harness.canvas);
		const base = {
			fromTop: 10n,
			reducedMotion: false,
			rowHeight: 20,
			toTop: 9n,
			visibleRows: 2,
		};

		expect(transaction.prepare({ ...base, reducedMotion: true })).toBe(false);
		expect(transaction.prepare({ ...base, rowHeight: 0 })).toBe(false);
		expect(transaction.prepare({ ...base, toTop: 7n })).toBe(false);
		expect(transaction.prepare({ ...base, toTop: 10n })).toBe(false);
		expect(harness.canvas.style.transform).toBe('');
	});
});
