// @vitest-environment jsdom

import { act, cleanup, createEvent, fireEvent, render } from '@testing-library/react';
import { createElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { PanelSplit } from '../../../../../model/panel-workspace-state';
import type {
	SharedSplitResize,
	WorkbenchPanelWorkspaceView,
} from '../../../../../types/panel-workspace-view';
import { WorkbenchPanelSplit } from '../WorkbenchPanelSplit';

const split: PanelSplit = {
	children: [
		{
			id: 'panel-1',
			node: {
				activeChildPresentationId: null,
				childPresentations: [],
				id: 'panel-1',
				kind: 'panel',
				primaryPresentation: {
					closeActivity: 'unknown',
					currentCwd: null,
					launchCwd: null,
					lifecycle: { kind: 'preview' },
					naturalTitle: null,
					presentationId: terminalPresentationId('terminal-1'),
					title: 'Terminal 1',
				},
			},
			weight: 50,
		},
		{
			id: 'panel-2',
			node: {
				activeChildPresentationId: null,
				childPresentations: [],
				id: 'panel-2',
				kind: 'panel',
				primaryPresentation: {
					closeActivity: 'unknown',
					currentCwd: null,
					launchCwd: null,
					lifecycle: { kind: 'preview' },
					naturalTitle: null,
					presentationId: terminalPresentationId('terminal-2'),
					title: 'Terminal 2',
				},
			},
			weight: 50,
		},
	],
	id: 'split-1',
	kind: 'split',
	orientation: 'horizontal',
};

const resizeObserverRegistrations: {
	readonly callback: ResizeObserverCallback;
	readonly observer: ResizeObserverMock;
	readonly target: Element;
}[] = [];

class ResizeObserverMock {
	readonly callback: ResizeObserverCallback;

	constructor(callback: ResizeObserverCallback) {
		this.callback = callback;
	}

	disconnect(): void {
		for (let index = resizeObserverRegistrations.length - 1; index >= 0; index -= 1) {
			if (resizeObserverRegistrations[index]?.observer === this) {
				resizeObserverRegistrations.splice(index, 1);
			}
		}
	}

	observe(target: Element): void {
		resizeObserverRegistrations.push({ callback: this.callback, observer: this, target });
	}

	unobserve(target: Element): void {
		const index = resizeObserverRegistrations.findIndex(
			(registration) => registration.observer === this && registration.target === target,
		);
		if (index >= 0) {
			resizeObserverRegistrations.splice(index, 1);
		}
	}
}

function flushResizeObservers(): void {
	for (const { callback, observer, target } of resizeObserverRegistrations) {
		callback(
			[
				{
					borderBoxSize: [{ blockSize: 100, inlineSize: 1_000 }],
					target,
				} as unknown as ResizeObserverEntry,
			],
			observer as unknown as ResizeObserver,
		);
	}
}

function createBounds(left: number, width: number): DOMRect {
	return {
		bottom: 100,
		height: 100,
		left,
		right: left + width,
		toJSON: () => ({}),
		top: 0,
		width,
		x: left,
		y: 0,
	};
}

function createView(
	commit: SharedSplitResize['commit'],
	maximizedPanelId: string | null = null,
): WorkbenchPanelWorkspaceView {
	const sharedSplitResize: SharedSplitResize = { commit, minimumChildPercent: 10 };
	return { maximizedPanelId, sharedSplitResize } as unknown as WorkbenchPanelWorkspaceView;
}

function prepareHorizontalGeometry(container: HTMLElement): HTMLElement {
	const panels = container.querySelectorAll<HTMLElement>('[data-slot="resizable-panel"]');
	const separator = container.querySelector<HTMLElement>('[data-slot="resizable-handle"]');
	const leadingPanel = panels[0];
	const trailingPanel = panels[1];
	if (leadingPanel === undefined || trailingPanel === undefined || separator === null) {
		throw new Error('Expected a two-panel split with one separator.');
	}
	return separator;
}

beforeEach(() => {
	resizeObserverRegistrations.length = 0;
	vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(
		function (this: HTMLElement) {
			switch (this.dataset['slot']) {
				case 'resizable-handle':
					return 10;
				case 'resizable-panel':
					return 500;
				case 'resizable-panel-group':
					return 1_000;
				default:
					return 0;
			}
		},
	);
	vi.spyOn(HTMLElement.prototype, 'offsetLeft', 'get').mockImplementation(
		function (this: HTMLElement) {
			switch (this.dataset['slot']) {
				case 'resizable-handle':
					return 495;
				case 'resizable-panel':
					return this.id.endsWith('panel-2') ? 500 : 0;
				default:
					return 0;
			}
		},
	);
	vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(
		function (this: HTMLElement) {
			switch (this.dataset['slot']) {
				case 'resizable-handle':
					return createBounds(495, 10);
				case 'resizable-panel':
					return createBounds(this.id.endsWith('panel-2') ? 500 : 0, 500);
				case 'resizable-panel-group':
					return createBounds(0, 1_000);
				default:
					return createBounds(0, 0);
			}
		},
	);
	Object.defineProperty(window, 'ResizeObserver', {
		configurable: true,
		value: ResizeObserverMock,
	});
	Object.defineProperty(window, 'matchMedia', {
		configurable: true,
		value: (query: string) => ({ matches: false, media: query }),
	});
});

afterEach(() => {
	cleanup();
	resizeObserverRegistrations.length = 0;
	vi.restoreAllMocks();
});

describe('WorkbenchPanelSplit', () => {
	it('keeps every panel DOM node mounted while projecting one maximized panel', () => {
		const commit = vi.fn<SharedSplitResize['commit']>(() => Promise.resolve());
		const renderNode = (node: PanelSplit['children'][number]['node']) =>
			createElement('span', { 'data-rendered-panel': node.id });
		const { container, rerender } = render(
			createElement(WorkbenchPanelSplit, {
				renderNode,
				split,
				view: createView(commit),
			}),
		);
		const firstPanelContent = container.querySelector('[data-rendered-panel="panel-1"]');
		const secondPanelContent = container.querySelector('[data-rendered-panel="panel-2"]');

		rerender(
			createElement(WorkbenchPanelSplit, {
				renderNode,
				split,
				view: createView(commit, 'panel-2'),
			}),
		);

		const panels = container.querySelectorAll<HTMLElement>('[data-slot="resizable-panel"]');
		expect(panels[0]?.hidden).toBe(true);
		expect(panels[1]?.hidden).toBe(false);
		expect(container.querySelector<HTMLElement>('[data-slot="resizable-handle"]')?.hidden).toBe(
			true,
		);
		expect(container.querySelector('[data-rendered-panel="panel-1"]')).toBe(firstPanelContent);
		expect(container.querySelector('[data-rendered-panel="panel-2"]')).toBe(secondPanelContent);

		rerender(
			createElement(WorkbenchPanelSplit, {
				renderNode,
				split,
				view: createView(commit),
			}),
		);

		expect(container.querySelector('[data-rendered-panel="panel-1"]')).toBe(firstPanelContent);
		expect(container.querySelector('[data-rendered-panel="panel-2"]')).toBe(secondPanelContent);
	});

	it('withholds descendant focus until maximize and minimize layouts have settled', () => {
		const commit = vi.fn<SharedSplitResize['commit']>(() => Promise.resolve());
		const settlements: Array<boolean | undefined> = [];
		const renderNode = (node: PanelSplit['children'][number]['node'], layoutSettled?: boolean) => {
			if (node.id === 'panel-2') settlements.push(layoutSettled);
			return createElement('span', { 'data-rendered-panel': node.id });
		};
		const { rerender } = render(
			createElement(WorkbenchPanelSplit, {
				renderNode,
				split,
				view: createView(commit),
			}),
		);
		expect(settlements.at(-1)).toBe(true);

		settlements.length = 0;
		rerender(
			createElement(WorkbenchPanelSplit, {
				renderNode,
				split,
				view: createView(commit, 'panel-2'),
			}),
		);
		expect(settlements).toContain(false);
		expect(settlements.at(-1)).toBe(true);

		settlements.length = 0;
		rerender(
			createElement(WorkbenchPanelSplit, {
				renderNode,
				split,
				view: createView(commit),
			}),
		);
		expect(settlements).toContain(false);
		expect(settlements.at(-1)).toBe(true);
	});

	it('withholds descendant focus while an initially maximized layout mounts', () => {
		const settlements: Array<boolean | undefined> = [];
		render(
			createElement(WorkbenchPanelSplit, {
				renderNode: (node, layoutSettled) => {
					if (node.id === 'panel-2') settlements.push(layoutSettled);
					return createElement('span', { 'data-rendered-panel': node.id });
				},
				split,
				view: createView(() => Promise.resolve(), 'panel-2'),
			}),
		);

		expect(settlements).toContain(false);
		expect(settlements.at(-1)).toBe(true);
	});

	it('leaves split creation unconstrained while the resize controller owns the pixel minimum', () => {
		const commit = vi.fn<SharedSplitResize['commit']>(() => Promise.resolve());
		const { container } = render(
			createElement(WorkbenchPanelSplit, {
				renderNode: () => createElement('span'),
				split,
				view: createView(commit),
			}),
		);

		expect(container.querySelectorAll('[data-slot="resizable-panel"]')).toHaveLength(2);
		expect(container.querySelector('[data-slot="resizable-handle"]')?.getAttribute('id')).toBe(
			'split-1--panel-1--panel-2',
		);
		expect(container.querySelector('[data-default-size]')).toBeNull();
		expect(container.querySelector('[data-min-size]')).toBeNull();
	});

	it('prevents unsupported separator keys before they reach the real primitive', () => {
		const commit = vi.fn<SharedSplitResize['commit']>(() => Promise.resolve());
		const { container } = render(
			createElement(WorkbenchPanelSplit, {
				renderNode: () => createElement('span'),
				split,
				view: createView(commit),
			}),
		);
		const separator = prepareHorizontalGeometry(container);
		act(flushResizeObservers);

		for (const key of ['Home', 'End', 'F6', 'Enter']) {
			const event = createEvent.keyDown(separator, {
				bubbles: true,
				cancelable: true,
				key,
			});
			fireEvent(separator, event);
			expect(event.defaultPrevented).toBe(true);
		}

		expect(commit).not.toHaveBeenCalled();
	});

	it('does not install a document-global cursor while the pointer crosses a separator', () => {
		const insertCursorRule = vi.spyOn(CSSStyleSheet.prototype, 'insertRule');
		const commit = vi.fn<SharedSplitResize['commit']>(() => Promise.resolve());
		const { container } = render(
			createElement(WorkbenchPanelSplit, {
				renderNode: () => createElement('span'),
				split,
				view: createView(commit),
			}),
		);
		const separator = prepareHorizontalGeometry(container);
		act(flushResizeObservers);

		fireEvent.pointerMove(separator, {
			buttons: 0,
			clientX: 500,
			clientY: 50,
			pointerType: 'mouse',
		});

		expect(separator.dataset['separator']).toBe('hover');
		expect(insertCursorRule).not.toHaveBeenCalled();
	});

	it('submits a real primitive arrow-key layout when the key is released', () => {
		const commit = vi.fn<SharedSplitResize['commit']>(() => Promise.resolve());
		const { container } = render(
			createElement(WorkbenchPanelSplit, {
				renderNode: () => createElement('span'),
				split,
				view: createView(commit),
			}),
		);
		const separator = prepareHorizontalGeometry(container);
		act(flushResizeObservers);

		fireEvent.keyDown(separator, { key: 'ArrowRight' });
		fireEvent.keyUp(separator, { key: 'ArrowRight' });

		expect(commit).toHaveBeenCalledExactlyOnceWith('split-1', 55);
	});

	it('uses the real primitive as the sole source of pointer layout candidates', () => {
		const commit = vi.fn<SharedSplitResize['commit']>(() => Promise.resolve());
		const { container } = render(
			createElement(WorkbenchPanelSplit, {
				renderNode: () => createElement('span'),
				split,
				view: createView(commit),
			}),
		);
		const separator = prepareHorizontalGeometry(container);
		act(flushResizeObservers);

		fireEvent.pointerDown(separator, {
			button: 0,
			buttons: 1,
			clientX: 500,
			clientY: 50,
			pointerId: 7,
			pointerType: 'mouse',
		});
		fireEvent.pointerMove(document, {
			buttons: 1,
			clientX: 550,
			clientY: 50,
			movementX: 50,
			pointerId: 7,
			pointerType: 'mouse',
		});
		fireEvent.pointerUp(document, {
			button: 0,
			buttons: 0,
			clientX: 550,
			clientY: 50,
			pointerId: 7,
			pointerType: 'mouse',
		});

		expect(commit).toHaveBeenCalledExactlyOnceWith('split-1', 55);
	});
});
