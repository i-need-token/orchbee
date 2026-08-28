import { ActionRuntime } from '@orchbee/actions';
import { StrictMode, useState } from 'react';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TAB_BOUNDS_ANIMATION_DURATION_MS } from '../utils/tab-strip-internals';
import { DRAG_START_ANIMATION_DURATION_MS } from '../dragging/dragged-tabs-container-layout';
import { requireDefined } from '../utils/requireDefined';
import { TabStripOrientation } from '../types/tab-strip-types';
import type { DragSessionData } from '../dragging/drag-session-data';
import type {
	TabStripMoveGroupToWindowCallback,
	TabStripMoveTabsToWindowCallback,
} from '../types/tab-strip-options';
import { ThumbnailCaptureReadiness } from '../components/tab-strip-view/components/hover-card/model/tab-hover-card-controller';
import type {
	HoverCardThumbnailSource,
	HoverCardThumbnailSubscription,
} from '../components/tab-strip-view/components/hover-card/model/tab-hover-card-thumbnail-observer';
import { TabModel, type TabGroupData } from '../model/tab-model';
import { TabStripComboButtonController } from '../model/tab-strip-combo-button';
import { VerticalTabStripExpandOnHoverController } from '../model/vertical-tab-strip-expand-on-hover';
import { TabStrip } from '../TabStrip';

class FakeThumbnailSource implements HoverCardThumbnailSource<string> {
	public callback: ((image: string) => void) | null = null;
	public requested = 0;
	public sizeHint: { width: number; height: number } | null = null;

	public hasData() {
		return true;
	}
	public getCaptureReadiness() {
		return ThumbnailCaptureReadiness.ReadyForFinalCapture;
	}
	public requestThumbnailImage() {
		this.requested += 1;
	}
	public subscribe(): HoverCardThumbnailSubscription<string> {
		return {
			setSizeHint: (size) => {
				this.sizeHint = { ...size };
			},
			setImageCallback: (callback) => {
				this.callback = callback;
			},
			dispose: vi.fn<() => void>(),
		};
	}
	public emit(image: string) {
		this.callback?.(image);
	}
}

function createModel() {
	return new TabModel({
		tabs: [
			{ id: 'pinned', title: 'Pinned', pinned: true, group: null },
			{ id: 'research-a', title: 'Research A', pinned: false, group: 'research' },
			{ id: 'research-b', title: 'Research B', pinned: false, group: 'research' },
			{ id: 'loose', title: 'Loose', pinned: false, group: null },
		],
		groups: [{ id: 'research', title: 'Research', color: 'blue', collapsed: false }],
		activeTabId: 'research-a',
	});
}

function setRect(element: Element, x: number, width: number, y = 0, height = 40): void {
	Object.defineProperty(element, 'getBoundingClientRect', {
		configurable: true,
		value: () => ({
			x,
			y,
			left: x,
			top: y,
			right: x + width,
			bottom: y + height,
			width,
			height,
			toJSON: () => ({}),
		}),
	});
}

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

describe('TabStrip', () => {
	const getHoverCard = (): HTMLElement => {
		const card = document.querySelector<HTMLElement>('[data-tab-strip-hover-card]');
		if (!card) throw new Error('Expected tab hover card to be mounted.');
		return card;
	};

	it('renders one rule for a shared boundary between inactive tabs', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'a', title: 'A', pinned: false, group: null },
				{ id: 'b', title: 'B', pinned: false, group: null },
				{ id: 'c', title: 'C', pinned: false, group: null },
			],
			activeTabId: 'c',
		});
		render(<TabStrip model={model} />);

		const trailing = requireDefined(
			document.querySelector<HTMLElement>('[data-tab-id="a"] [data-tab-separator="trailing"]'),
		);
		const leading = requireDefined(
			document.querySelector<HTMLElement>('[data-tab-id="b"] [data-tab-separator="leading"]'),
		);

		expect(trailing.style.getPropertyValue('--ob-comp-tab-strip-runtime-opacity')).toBe('1');
		expect(leading.style.getPropertyValue('--ob-comp-tab-strip-runtime-opacity')).toBe('0');
	});

	it('keeps structural separator opacity immediate while hover owns the fade', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'a', title: 'A', pinned: false, group: null },
				{ id: 'b', title: 'B', pinned: false, group: null },
				{ id: 'c', title: 'C', pinned: false, group: null },
			],
			activeTabId: 'c',
		});
		render(<TabStrip model={model} />);

		const a = screen.getByRole('tab', { name: /^A/ });
		const trailing = requireDefined(
			a.querySelector<HTMLElement>('[data-tab-separator="trailing"]'),
		);
		expect(trailing.style.getPropertyValue('--ob-comp-tab-strip-runtime-opacity')).toBe('1');

		fireEvent.mouseEnter(a);

		expect(trailing.style.getPropertyValue('--ob-comp-tab-strip-runtime-opacity')).toBe('1');
		expect(trailing.hasAttribute('data-hover-suppressed')).toBe(true);
	});

	it('snaps horizontal slots when the tab count changes', () => {
		const rect = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 800,
			bottom: 40,
			width: 800,
			height: 40,
			toJSON: () => ({}),
		} as DOMRect);
		try {
			const model = createModel();
			render(<TabStrip model={model} />);

			act(() => {
				model.addTab(
					{ title: 'Added', pinned: false, group: null },
					{ id: 'added', activate: false },
				);
			});
			expect(screen.getByRole('tab', { name: /^Added/ })).toBeTruthy();
			expect(document.querySelector('[data-slot-animating]')).toBeNull();

			act(() => model.closeTab('added'));
			expect(screen.queryByRole('tab', { name: /^Added/ })).toBeNull();
			expect(document.querySelector('[data-slot-animating]')).toBeNull();
		} finally {
			rect.mockRestore();
		}
	});

	it('snaps a steady-state active-width change instead of starting bounds animation', () => {
		const requestAnimationFrameDescriptor = Object.getOwnPropertyDescriptor(
			window,
			'requestAnimationFrame',
		);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		Object.defineProperty(window, 'requestAnimationFrame', {
			configurable: true,
			value: requestAnimationFrame,
		});
		const rect = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 120,
			bottom: 40,
			width: 120,
			height: 40,
			toJSON: () => ({}),
		} as DOMRect);
		try {
			const model = new TabModel({
				tabs: [
					{ id: 'a', title: 'A', pinned: false, group: null },
					{ id: 'b', title: 'B', pinned: false, group: null },
					{ id: 'c', title: 'C', pinned: false, group: null },
				],
				activeTabId: 'a',
			});
			render(<TabStrip model={model} showNewTabButton={false} />);
			expect(requestAnimationFrame).not.toHaveBeenCalled();

			act(() => model.selectTab('b'));

			expect(model.getSnapshot().activeTabId).toBe('b');
			expect(document.querySelector('[data-slot-animating]')).toBeNull();
			expect(requestAnimationFrame).not.toHaveBeenCalled();
		} finally {
			rect.mockRestore();
			if (requestAnimationFrameDescriptor) {
				Object.defineProperty(window, 'requestAnimationFrame', requestAnimationFrameDescriptor);
			} else {
				Reflect.deleteProperty(window, 'requestAnimationFrame');
			}
		}
	});

	it('retargets an active-width change while bounds motion is already running', () => {
		const requestAnimationFrameDescriptor = Object.getOwnPropertyDescriptor(
			window,
			'requestAnimationFrame',
		);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		Object.defineProperty(window, 'requestAnimationFrame', {
			configurable: true,
			value: requestAnimationFrame,
		});
		const rect = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 120,
			bottom: 40,
			width: 120,
			height: 40,
			toJSON: () => ({}),
		} as DOMRect);
		try {
			const model = new TabModel({
				tabs: [
					{ id: 'a', title: 'A', pinned: false, group: null },
					{ id: 'b', title: 'B', pinned: false, group: null },
					{ id: 'c', title: 'C', pinned: false, group: null },
				],
				activeTabId: 'a',
			});
			render(<TabStrip model={model} showNewTabButton={false} />);

			act(() => model.shiftTabRelative('c', -1));
			expect(document.querySelector('[data-slot-animating]')).not.toBeNull();
			expect(requestAnimationFrame).toHaveBeenCalledOnce();

			act(() => model.selectTab('b'));

			expect(model.getSnapshot().activeTabId).toBe('b');
			expect(document.querySelector('[data-slot-animating]')).not.toBeNull();
			expect(requestAnimationFrame).toHaveBeenCalledTimes(2);
		} finally {
			rect.mockRestore();
			if (requestAnimationFrameDescriptor) {
				Object.defineProperty(window, 'requestAnimationFrame', requestAnimationFrameDescriptor);
			} else {
				Reflect.deleteProperty(window, 'requestAnimationFrame');
			}
		}
	});

	it('does not arm a horizontal drag while tab-slot bounds are animating', () => {
		const requestAnimationFrameDescriptor = Object.getOwnPropertyDescriptor(
			window,
			'requestAnimationFrame',
		);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		Object.defineProperty(window, 'requestAnimationFrame', {
			configurable: true,
			value: requestAnimationFrame,
		});
		const rect = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 360,
			bottom: 40,
			width: 360,
			height: 40,
			toJSON: () => ({}),
		} as DOMRect);
		try {
			const model = new TabModel({
				tabs: [
					{ id: 'a', title: 'A', pinned: false, group: null },
					{ id: 'b', title: 'B', pinned: false, group: null },
					{ id: 'c', title: 'C', pinned: false, group: null },
				],
				activeTabId: 'a',
			});
			render(<TabStrip model={model} showNewTabButton={false} />);
			const b = screen.getByRole('tab', { name: /^B/ });

			act(() => model.shiftTabRelative('c', -1));
			expect(document.querySelector('[data-slot-animating]')).not.toBeNull();

			fireEvent.pointerDown(b, { button: 0, pointerId: 101, clientX: 180, clientY: 20 });
			fireEvent.pointerMove(window, { pointerId: 101, clientX: 220, clientY: 20 });

			expect(document.querySelector('[data-horizontal-drag-context]')).toBeNull();
			expect(screen.getByRole('tablist', { name: 'Tabs' }).hasAttribute('data-dragging')).toBe(
				false,
			);
			fireEvent.pointerUp(window, { pointerId: 101, clientX: 220, clientY: 20 });
			expect(model.getSnapshot().activeTabId).toBe('b');
		} finally {
			rect.mockRestore();
			if (requestAnimationFrameDescriptor) {
				Object.defineProperty(window, 'requestAnimationFrame', requestAnimationFrameDescriptor);
			} else {
				Reflect.deleteProperty(window, 'requestAnimationFrame');
			}
		}
	});

	it('completes residual horizontal bounds motion when an armed drag activates', () => {
		const requestAnimationFrameDescriptor = Object.getOwnPropertyDescriptor(
			window,
			'requestAnimationFrame',
		);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		Object.defineProperty(window, 'requestAnimationFrame', {
			configurable: true,
			value: requestAnimationFrame,
		});
		const rect = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 360,
			bottom: 40,
			width: 360,
			height: 40,
			toJSON: () => ({}),
		} as DOMRect);
		try {
			const model = new TabModel({
				tabs: [
					{ id: 'a', title: 'A', pinned: false, group: null },
					{ id: 'b', title: 'B', pinned: false, group: null },
					{ id: 'c', title: 'C', pinned: false, group: null },
				],
				activeTabId: 'a',
			});
			render(<TabStrip model={model} showNewTabButton={false} />);
			const a = screen.getByRole('tab', { name: /^A/ });

			fireEvent.pointerDown(a, { button: 0, pointerId: 102, clientX: 50, clientY: 20 });
			act(() => model.shiftTabRelative('c', -1));
			expect(document.querySelector('[data-slot-animating]')).not.toBeNull();

			fireEvent.pointerMove(window, { pointerId: 102, clientX: 61, clientY: 20 });

			expect(document.querySelector('[data-horizontal-drag-context]')).not.toBeNull();
			expect(document.querySelector('[data-slot-animating]')).toBeNull();
			fireEvent.pointerCancel(window, { pointerId: 102, clientX: 61, clientY: 20 });
		} finally {
			rect.mockRestore();
			if (requestAnimationFrameDescriptor) {
				Object.defineProperty(window, 'requestAnimationFrame', requestAnimationFrameDescriptor);
			} else {
				Reflect.deleteProperty(window, 'requestAnimationFrame');
			}
		}
	});

	it('reconciles hover beneath a stationary pointer after a programmatic reorder', () => {
		const requestAnimationFrameDescriptor = Object.getOwnPropertyDescriptor(
			window,
			'requestAnimationFrame',
		);
		const elementFromPointDescriptor = Object.getOwnPropertyDescriptor(
			document,
			'elementFromPoint',
		);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		Object.defineProperty(window, 'requestAnimationFrame', {
			configurable: true,
			value: requestAnimationFrame,
		});
		const rect = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 480,
			bottom: 40,
			width: 480,
			height: 40,
			toJSON: () => ({}),
		} as DOMRect);
		try {
			const model = new TabModel({
				tabs: [
					{ id: 'a', title: 'A', pinned: false, group: null },
					{ id: 'b', title: 'B', pinned: false, group: null },
					{ id: 'c', title: 'C', pinned: false, group: null },
				],
				activeTabId: 'a',
			});
			render(<TabStrip model={model} />);
			const a = screen.getByRole('tab', { name: /^A/ });
			const b = screen.getByRole('tab', { name: /^B/ });
			Object.defineProperty(document, 'elementFromPoint', {
				configurable: true,
				value: () => b,
			});
			fireEvent.mouseEnter(a, { clientX: 160, clientY: 20 });
			expect(a.getAttribute('data-hovered')).toBe('true');

			fireEvent.keyDown(a, { key: 'ArrowRight', ctrlKey: true });
			expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['b', 'a', 'c']);
			const animationFrame = requireDefined(requestAnimationFrame.mock.calls.at(-1)?.[0]);
			act(() => animationFrame(performance.now() + TAB_BOUNDS_ANIMATION_DURATION_MS));

			expect(b.getAttribute('data-hovered')).toBe('true');
		} finally {
			rect.mockRestore();
			if (requestAnimationFrameDescriptor) {
				Object.defineProperty(window, 'requestAnimationFrame', requestAnimationFrameDescriptor);
			} else {
				Reflect.deleteProperty(window, 'requestAnimationFrame');
			}
			if (elementFromPointDescriptor) {
				Object.defineProperty(document, 'elementFromPoint', elementFromPointDescriptor);
			} else {
				Reflect.deleteProperty(document, 'elementFromPoint');
			}
		}
	});

	it('gives inactive horizontal tab content an opaque DOM surface', () => {
		const model = createModel();
		render(<TabStrip model={model} />);

		const loose = screen.getByRole('tab', { name: /^Loose/ });
		const surface = requireDefined(loose.querySelector<HTMLElement>('[data-tab-surface]'));
		expect(surface.tagName).toBe('SPAN');
		expect(surface.querySelector('[data-tab-content]')).not.toBeNull();
		expect(loose.querySelector('[data-tab-hit-target]')).not.toBeNull();
		expect(surface.getAttribute('data-tab-background-fill')).toBe('');
		expect(loose.querySelector('[data-tab-active-shadow]')).toBeNull();
	});

	it('uses a separate vertical hover layer from immediate selection paint', () => {
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);

		const loose = screen.getByRole('tab', { name: /^Loose/ });
		const background = requireDefined(loose.querySelector<HTMLElement>('[data-tab-state-layer]'));
		expect(background.getAttribute('data-tab-state-layer')).toBe('');
		const hoverLayer = requireDefined(
			background.querySelector<HTMLElement>('[data-tab-hover-layer]'),
		);
		expect(hoverLayer.className).not.toBe('');
	});

	it('hosts horizontal leading actions on the tab-strip surface before the tab track', () => {
		const model = createModel();
		render(
			<TabStrip
				horizontalLeadingActions={<button type='button'>Toggle sidebar</button>}
				model={model}
			/>,
		);

		const root = requireDefined(document.querySelector<HTMLElement>('[data-ct-root]'));
		const button = screen.getByRole('button', { name: 'Toggle sidebar' });
		const leadingActions = requireDefined(
			button.closest<HTMLElement>('[data-horizontal-leading-actions]'),
		);
		const track = requireDefined(document.querySelector<HTMLElement>('[data-horizontal-track]'));
		expect(root.contains(leadingActions)).toBe(true);
		expect(leadingActions.nextElementSibling).toBe(track);
	});

	it('selects with the pointer but leaves unmodified focus-navigation keys to Views accessibility', () => {
		const model = createModel();
		render(<TabStrip model={model} />);

		const loose = screen.getByRole('tab', { name: /^Loose/ });
		fireEvent.pointerDown(loose, { button: 0, pointerId: 1, clientX: 100, clientY: 10 });
		fireEvent.pointerUp(window, { button: 0, pointerId: 1, clientX: 100, clientY: 10 });
		expect(model.getSnapshot().activeTabId).toBe('loose');

		loose.focus();
		fireEvent.keyDown(loose, { key: 'ArrowLeft' });
		fireEvent.keyDown(loose, { key: 'Home' });
		expect(model.getSnapshot().activeTabId).toBe('loose');
		expect(document.activeElement).toBe(loose);
		expect(loose.getAttribute('tabindex')).toBe('-1');
	});

	it('selects touch tabs on gesture-down and starts dragging after crossing the threshold', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 12, 216, 110, 30);

		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 61,
			pointerType: 'touch',
			clientX: 80,
			clientY: 125,
		});
		expect(model.getSnapshot().activeTabId).toBe('loose');
		fireEvent.pointerUp(window, {
			button: 0,
			pointerId: 61,
			pointerType: 'touch',
			clientX: 80,
			clientY: 125,
		});
		expect(model.getSnapshot().activeTabId).toBe('loose');

		model.selectTab('research-a');
		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 62,
			pointerType: 'touch',
			clientX: 80,
			clientY: 125,
		});
		fireEvent.pointerMove(window, {
			pointerId: 62,
			pointerType: 'touch',
			clientX: 80,
			clientY: 145,
		});
		expect(model.getSnapshot().activeTabId).toBe('loose');
		expect(document.querySelector('[data-tab-strip-drag-ghost]')).not.toBeNull();
		fireEvent.pointerCancel(window, { pointerId: 62, pointerType: 'touch' });
	});

	it('opens source-matching editors for a touch long tap without collapsing a group', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });

		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 64,
			pointerType: 'touch',
			clientX: 80,
			clientY: 125,
		});
		act(() => vi.advanceTimersByTime(500));
		fireEvent.pointerUp(window, {
			button: 0,
			pointerId: 64,
			pointerType: 'touch',
			clientX: 80,
			clientY: 125,
		});
		expect(screen.getByRole('menu')).toBeTruthy();
		fireEvent.pointerDown(document.body);

		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		fireEvent.pointerDown(group, {
			button: 0,
			pointerId: 65,
			pointerType: 'touch',
			clientX: 80,
			clientY: 80,
		});
		act(() => vi.advanceTimersByTime(500));
		fireEvent.pointerUp(window, {
			button: 0,
			pointerId: 65,
			pointerType: 'touch',
			clientX: 80,
			clientY: 80,
		});
		fireEvent.click(group);
		expect(screen.getByRole('textbox', { name: 'Group name' })).toBeTruthy();
		expect(model.getGroup('research')?.collapsed).toBe(false);
	});

	it('does not begin dragging a tab that a selection-modifier press deselected', () => {
		const model = createModel();
		model.setSelection(['research-a', 'research-b'], 'research-a');
		render(<TabStrip model={model} />);
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		setRect(researchB, 150, 100);

		fireEvent.pointerDown(researchB, {
			button: 0,
			pointerId: 71,
			clientX: 200,
			clientY: 20,
			ctrlKey: true,
		});
		fireEvent.pointerMove(window, { pointerId: 71, clientX: 260, clientY: 20 });

		expect(model.getSnapshot().selectedTabIds.has('research-b')).toBe(false);
		expect(document.querySelector('[data-tab-strip-drag-ghost]')).toBeNull();
	});

	it('keeps middle-release containment when the pointer is released over another tab', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const researchA = screen.getByRole('tab', { name: /^Research A/ });
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		const descriptor = Object.getOwnPropertyDescriptor(document, 'elementFromPoint');
		const elementFromPoint = vi.fn<() => Element | null>(() => researchB);
		Object.defineProperty(document, 'elementFromPoint', {
			configurable: true,
			value: elementFromPoint,
		});
		try {
			fireEvent.pointerDown(researchA, { button: 1, pointerId: 81, clientX: 100, clientY: 20 });
			fireEvent.pointerUp(window, { button: 1, pointerId: 81, clientX: 200, clientY: 20 });
			expect(model.getIndexOfTab('research-a')).not.toBeNull();
			expect(model.getIndexOfTab('research-b')).not.toBeNull();

			fireEvent(researchA, new MouseEvent('auxclick', { button: 1, bubbles: true }));
			expect(model.getIndexOfTab('research-a')).toBeNull();
			expect(model.getIndexOfTab('research-b')).not.toBeNull();
		} finally {
			if (descriptor) Object.defineProperty(document, 'elementFromPoint', descriptor);
			else Reflect.deleteProperty(document, 'elementFromPoint');
		}
	});

	it('preserves a range selection when Shift was held on pointer down', () => {
		const model = createModel();
		model.setSelection(['research-a', 'research-b'], 'research-a');
		render(<TabStrip model={model} />);
		const researchB = screen.getByRole('tab', { name: /^Research B/ });

		fireEvent.pointerDown(researchB, {
			button: 0,
			pointerId: 72,
			clientX: 200,
			clientY: 20,
			shiftKey: true,
		});
		fireEvent.pointerUp(window, {
			button: 0,
			pointerId: 72,
			clientX: 200,
			clientY: 20,
			shiftKey: false,
		});

		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a', 'research-b']);
	});

	it('activates with Enter on keydown and Space on keyup', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });

		fireEvent.keyDown(loose, { key: ' ' });
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		fireEvent.keyUp(loose, { key: ' ' });
		expect(model.getSnapshot().activeTabId).toBe('loose');
	});

	it('announces Chromium keyboard reorders and group-boundary changes', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });

		fireEvent.keyDown(loose, { key: 'ArrowLeft', ctrlKey: true });
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.group).toBe('research');
		expect(screen.getByRole('status').textContent).toContain('Tab added to group Research');

		fireEvent.keyDown(screen.getByRole('tab', { name: /^Loose/ }), {
			key: 'ArrowRight',
			ctrlKey: true,
		});
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.group).toBeNull();
		expect(screen.getByRole('status').textContent).toContain('Tab removed from group Research');
	});

	it('does not announce a relative keyboard move rejected at a pin boundary', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'pinned', title: 'Pinned', pinned: true, group: null },
				{ id: 'first', title: 'First', pinned: false, group: null },
			],
			activeTabId: 'first',
		});
		render(<TabStrip model={model} />);

		fireEvent.keyDown(screen.getByRole('tab', { name: /^First/ }), {
			key: 'ArrowLeft',
			ctrlKey: true,
		});

		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['pinned', 'first']);
		expect(screen.getByRole('status').textContent).toBe('');
	});

	it('updates accessible positions immediately while retaining keyboard focus', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'a', title: 'A', pinned: false, group: null },
				{ id: 'b', title: 'B', pinned: false, group: null },
				{ id: 'c', title: 'C', pinned: false, group: null },
			],
			activeTabId: 'a',
		});
		render(<TabStrip model={model} />);
		const a = screen.getByRole('tab', { name: /^A/ });
		a.focus();

		fireEvent.keyDown(a, { key: 'ArrowRight', ctrlKey: true });

		expect(document.activeElement).toBe(a);
		expect(a.getAttribute('aria-posinset')).toBe('2');
		expect(a.getAttribute('aria-setsize')).toBe('3');
		expect(screen.getByRole('tab', { name: /^B/ }).getAttribute('aria-posinset')).toBe('1');
	});

	it('collapses a group while projecting its active child through the group header', () => {
		const model = createModel();
		render(<TabStrip model={model} />);

		fireEvent.click(screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ }));

		expect(model.getGroup('research')?.collapsed).toBe(true);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		const collapsedHeader = screen.getByRole('tablist', { name: /^Research, 2 tabs.*collapsed$/ });
		expect(collapsedHeader.getAttribute('data-active')).toBe('true');
		expect(collapsedHeader.getAttribute('data-active-tab-id')).toBe('research-a');
		const activeSurface = requireDefined(
			collapsedHeader.querySelector<HTMLElement>('[data-group-active-surface]'),
		);
		expect(activeSurface.tagName).toBe('SPAN');
		expect(activeSurface.querySelector('[data-group-chip]')).not.toBeNull();
		expect(collapsedHeader.querySelector('[data-group-active-background]')).toBeNull();
		expect(document.activeElement).toBe(collapsedHeader);

		fireEvent.click(collapsedHeader);
		const expandedHeader = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		expect(expandedHeader.querySelector('[data-group-active-surface]')).toBeNull();
		expect(expandedHeader.querySelector('[data-group-surface]')).not.toBeNull();
		expect(document.activeElement).toBe(expandedHeader);
	});

	it('keeps collapsed tabs and their underline visible until the bounds animation finishes', () => {
		const requestAnimationFrameDescriptor = Object.getOwnPropertyDescriptor(
			window,
			'requestAnimationFrame',
		);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		Object.defineProperty(window, 'requestAnimationFrame', {
			configurable: true,
			value: requestAnimationFrame,
		});
		const rect = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 480,
			bottom: 40,
			width: 480,
			height: 40,
			toJSON: () => ({}),
		} as DOMRect);
		try {
			const model = createModel();
			render(<TabStrip model={model} />);
			const researchA = screen.getByRole('tab', { name: /^Research A/ });
			const wrapper = requireDefined(researchA.closest<HTMLElement>('[data-horizontal-item]'));

			fireEvent.click(screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ }));
			expect(wrapper.hasAttribute('data-group-collapsed-hidden')).toBe(false);
			expect(document.querySelector('[data-group-underline-id="research"]')).toBeTruthy();

			const animationFrame = requireDefined(requestAnimationFrame.mock.calls.at(-1)?.[0]);
			act(() => animationFrame(performance.now() + TAB_BOUNDS_ANIMATION_DURATION_MS));
			expect(wrapper.hasAttribute('data-group-collapsed-hidden')).toBe(true);
			expect(document.querySelector('[data-group-underline-id="research"]')).toBeNull();
		} finally {
			rect.mockRestore();
			if (requestAnimationFrameDescriptor) {
				Object.defineProperty(window, 'requestAnimationFrame', requestAnimationFrameDescriptor);
			} else {
				Reflect.deleteProperty(window, 'requestAnimationFrame');
			}
		}
	});

	it('freezes horizontal width only for pointer-originated group collapse', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		let group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });

		fireEvent.pointerDown(group, { button: 0, pointerId: 91, pointerType: 'mouse' });
		fireEvent.pointerUp(window, { button: 0, pointerId: 91, pointerType: 'mouse' });
		fireEvent.click(group, { detail: 1 });
		expect(
			screen.getByRole('tablist', { name: 'Tabs' }).getAttribute('data-tab-closing-mode'),
		).toBe('mouse');

		group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*collapsed$/ });
		fireEvent.click(group, { detail: 1 });
		expect(
			screen.getByRole('tablist', { name: 'Tabs' }).hasAttribute('data-tab-closing-mode'),
		).toBe(false);

		group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		fireEvent.click(group, { detail: 0 });
		expect(
			screen.getByRole('tablist', { name: 'Tabs' }).hasAttribute('data-tab-closing-mode'),
		).toBe(false);
	});

	it('closes an open group editor without toggling or dragging its header', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });

		fireEvent.contextMenu(group, { clientX: 80, clientY: 20 });
		expect(screen.getByRole('menu')).toBeTruthy();
		fireEvent.pointerDown(group, { button: 0, pointerId: 73, clientX: 80, clientY: 20 });
		fireEvent.pointerUp(group, { button: 0, pointerId: 73, clientX: 80, clientY: 20 });
		fireEvent.click(group);

		expect(screen.queryByRole('menu')).toBeNull();
		expect(model.getGroup('research')?.collapsed).toBe(false);
		expect(document.querySelector('[data-tab-strip-drag-ghost]')).toBeNull();
	});

	it('runs tab context-menu commands through the model', () => {
		const model = createModel();
		render(<TabStrip model={model} />);

		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Loose/ }), {
			clientX: 50,
			clientY: 30,
		});
		fireEvent.click(screen.getByRole('menuitem', { name: 'Duplicate' }));

		expect(model.getSnapshot().tabs.filter((tab) => tab.title === 'Loose')).toHaveLength(2);
		expect(screen.queryByRole('menu')).toBeNull();
	});

	it('routes selected tabs and complete groups to an existing native window', async () => {
		const model = createModel();
		model.setSelection(['research-a', 'research-b'], 'research-a');
		const onMoveTabsToWindow = vi.fn<TabStripMoveTabsToWindowCallback>((context, _input) => {
			context.markCommitted();
			return Promise.resolve({ kind: 'completed' as const });
		});
		const onMoveGroupToWindow = vi.fn<TabStripMoveGroupToWindowCallback>((context, _input) => {
			context.markCommitted();
			return Promise.resolve({ kind: 'completed' as const });
		});
		render(
			<TabStrip
				model={model}
				onMoveGroupToWindow={onMoveGroupToWindow}
				onMoveTabsToWindow={onMoveTabsToWindow}
				windowTargets={[{ label: 'Window 2', windowId: 'desktop-window-2' }]}
			/>,
		);

		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Research A/ }), {
			clientX: 50,
			clientY: 30,
		});
		fireEvent.keyDown(screen.getByRole('menuitem', { name: 'Move tabs to window' }), {
			key: 'ArrowRight',
		});
		fireEvent.click(screen.getByRole('menuitem', { name: 'Window 2' }));
		await vi.waitFor(() =>
			expect(onMoveTabsToWindow).toHaveBeenCalledWith(expect.anything(), {
				destinationWindowId: 'desktop-window-2',
				tabIds: ['research-a', 'research-b'],
			}),
		);

		fireEvent.contextMenu(screen.getByRole('tablist', { name: /^Research, 2 tabs/ }), {
			clientX: 80,
			clientY: 20,
		});
		fireEvent.keyDown(screen.getByRole('menuitem', { name: 'Move group to window' }), {
			key: 'ArrowRight',
		});
		fireEvent.click(screen.getByRole('menuitem', { name: 'Window 2' }));
		await vi.waitFor(() =>
			expect(onMoveGroupToWindow).toHaveBeenCalledWith(expect.anything(), {
				destinationWindowId: 'desktop-window-2',
				groupId: 'research',
			}),
		);
	});

	it('renders only host-supplied shortcuts in the tab context menu trailing slot', () => {
		const model = createModel();
		render(<TabStrip model={model} shortcutLabels={{ closeTab: '⌘W', newTab: '⌘T' }} />);

		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Loose/ }), {
			clientX: 50,
			clientY: 30,
		});

		const shortcuts = Array.from(
			document.querySelectorAll<HTMLElement>('[data-slot="context-menu-shortcut"]'),
		);
		expect(shortcuts.map((shortcut) => shortcut.textContent)).toEqual(['⌘T', '⌘W']);
		expect(shortcuts.map((shortcut) => shortcut.closest('[role="menuitem"]')?.textContent)).toEqual(
			['New tab to the right⌘T', 'Close⌘W'],
		);
		expect(screen.getByRole('menuitem', { name: 'Duplicate' }).textContent).toBe('Duplicate');
		expect(screen.getByRole('menuitem', { name: 'Pin' }).textContent).toBe('Pin');
	});

	it('requests title editing for only the invoked Tab and renders its current shortcut', () => {
		const model = createModel();
		const onSetTabTitle = vi.fn<(tabId: string) => void>();
		render(
			<TabStrip
				model={model}
				onSetTabTitle={onSetTabTitle}
				shortcutLabels={{ setTabTitle: '⇧⌘I' }}
			/>,
		);

		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Loose/ }), {
			clientX: 50,
			clientY: 30,
		});
		const menuItem = screen.getByRole('menuitem', { name: /^Set tab title…/ });
		expect(menuItem.textContent).toBe('Set tab title…⇧⌘I');
		fireEvent.click(menuItem);

		expect(onSetTabTitle).toHaveBeenCalledExactlyOnceWith('loose');
		expect(screen.queryByRole('menu')).toBeNull();
	});

	it('requests the opposite orientation from the blank tab-strip context menu', () => {
		const model = createModel();
		const onOrientationChange = vi.fn<(orientation: TabStripOrientation) => void>();
		const { rerender } = render(
			<TabStrip model={model} onOrientationChange={onOrientationChange} />,
		);

		fireEvent.contextMenu(screen.getByRole('tablist', { name: 'Tabs' }), {
			clientX: 700,
			clientY: 20,
		});
		expect(screen.getByRole('menu', { name: 'Tab strip menu' })).toBeTruthy();
		fireEvent.click(screen.getByRole('menuitem', { name: 'Open vertical tabs' }));
		expect(onOrientationChange).toHaveBeenLastCalledWith(TabStripOrientation.Vertical);

		rerender(
			<TabStrip
				model={model}
				orientation={TabStripOrientation.Vertical}
				onOrientationChange={onOrientationChange}
			/>,
		);
		fireEvent.contextMenu(screen.getByRole('tablist', { name: 'Tabs' }), {
			clientX: 120,
			clientY: 500,
		});
		fireEvent.click(screen.getByRole('menuitem', { name: 'Close vertical tabs' }));
		expect(onOrientationChange).toHaveBeenLastCalledWith(TabStripOrientation.Horizontal);
	});

	it('runs group membership commands from the tab context menu', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		fireEvent.click(screen.getByRole('tab', { name: /^Loose/ }));
		const openLooseMenu = () =>
			fireEvent.contextMenu(screen.getByRole('tab', { name: /^Loose/ }), {
				clientX: 50,
				clientY: 30,
			});

		openLooseMenu();
		fireEvent.click(screen.getByRole('menuitem', { name: 'Add tab to group' }));
		const newGroup = screen.getByRole('menuitem', { name: 'New group' });
		fireEvent.pointerDown(newGroup);
		expect(screen.getByRole('menuitem', { name: 'New group' })).toBeTruthy();
		fireEvent.click(newGroup);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.group).not.toBeNull();
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'research-a',
			'research-b',
			'loose',
		]);
		const groupedLoose = screen.getByRole('tab', { name: /^Loose/ });
		expect(groupedLoose.getAttribute('data-grouped')).toBe('true');
		expect(groupedLoose.querySelector('[style*="--ob-comp-tab-strip-runtime-stroke"]')).toBeNull();
		openLooseMenu();
		fireEvent.click(screen.getByRole('menuitem', { name: 'Remove from group' }));
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.group).toBeNull();
		expect(document.querySelector('[data-group-underline-id="group-1"]')).toBeNull();
		expect(
			screen
				.getByRole('tab', { name: /^Loose/ })
				.querySelector('[style*="--ob-comp-tab-strip-runtime-stroke"]'),
		).toBeNull();
	});

	it('keeps group-name editing isolated from menu navigation and clears group visuals on ungroup', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const groupHeader = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		fireEvent.contextMenu(groupHeader, { clientX: 80, clientY: 20 });
		const input = screen.getByRole('textbox', { name: 'Group name' });
		const outerKeyDown = vi.fn<() => void>();
		document.addEventListener('keydown', outerKeyDown);

		fireEvent.keyDown(input, { key: 'Process', isComposing: true });
		fireEvent.change(input, { target: { value: 'Working' } });
		document.removeEventListener('keydown', outerKeyDown);

		expect(outerKeyDown).not.toHaveBeenCalled();
		expect(model.getGroup('research')?.title).toBe('Working');
		fireEvent.click(screen.getByRole('radio', { name: /^blue$/i }));
		fireEvent.click(screen.getByRole('menuitem', { name: 'Ungroup' }));

		expect(model.getGroup('research')).toBeNull();
		expect(document.querySelector('[data-group-underline-id="research"]')).toBeNull();
		expect(
			document.querySelector('[data-tab-strip-group-header][data-group-id="research"]'),
		).toBeNull();
		expect(
			screen
				.getByRole('tab', { name: /^Research A/ })
				.querySelector('[style*="--ob-comp-tab-strip-runtime-stroke"]'),
		).toBeNull();
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'research-a',
			'research-b',
			'loose',
		]);
	});

	it('uses the shared context-menu keyboard navigation for the tab group submenu', () => {
		const model = createModel();
		render(<TabStrip model={model} />);

		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Loose/ }), {
			clientX: 50,
			clientY: 30,
		});
		const menu = screen.getByRole('menu');

		fireEvent.keyDown(menu, { key: 'ArrowDown' });
		expect(document.activeElement).toBe(
			screen.getByRole('menuitem', { name: 'New tab to the right' }),
		);
		fireEvent.keyDown(menu, { key: 'ArrowDown' });
		const groupTrigger = screen.getByRole('menuitem', { name: 'Add tab to group' });
		expect(document.activeElement).toBe(groupTrigger);

		fireEvent.keyDown(groupTrigger, { key: 'ArrowRight' });
		const submenu = screen.getByRole('menu', { name: 'Add tab to group' });
		expect(within(submenu).getByRole('menuitem', { name: 'New group' })).toBeTruthy();
		fireEvent.keyDown(groupTrigger, { key: 'ArrowLeft' });
		expect(screen.queryByRole('menu', { name: 'Add tab to group' })).toBeNull();
	});

	it('uses orientation-specific relative labels in the tab context menu', () => {
		const horizontalModel = createModel();
		const horizontal = render(<TabStrip model={horizontalModel} />);
		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Loose/ }), {
			clientX: 50,
			clientY: 30,
		});
		expect(screen.getByRole('menuitem', { name: 'New tab to the right' })).toBeTruthy();
		expect(screen.getByRole('menuitem', { name: 'Close tabs to the right' })).toBeTruthy();
		horizontal.unmount();

		const verticalModel = createModel();
		render(<TabStrip model={verticalModel} orientation={TabStripOrientation.Vertical} />);
		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Loose/ }), {
			clientX: 50,
			clientY: 30,
		});
		expect(screen.getByRole('menuitem', { name: 'New tab below' })).toBeTruthy();
		expect(screen.getByRole('menuitem', { name: 'Close tabs below' })).toBeTruthy();
	});

	it('ports selected-tab close commands from TabStripModel', () => {
		const model = createModel();
		model.setSelection(['research-a', 'research-b'], 'research-a');
		render(<TabStrip model={model} />);

		const openSelectedMenu = () =>
			fireEvent.contextMenu(screen.getByRole('tab', { name: /^Research A/ }), {
				clientX: 50,
				clientY: 30,
			});

		openSelectedMenu();
		fireEvent.click(screen.getByRole('menuitem', { name: 'Close tabs to the right' }));
		expect(model.getIndexOfTab('loose')).toBeNull();
		expect(model.getIndexOfTab('research-a')).not.toBeNull();
		expect(model.getIndexOfTab('research-b')).not.toBeNull();

		openSelectedMenu();
		expect(
			screen.getByRole('menuitem', { name: 'Close other tabs' }).getAttribute('aria-disabled'),
		).toBe('true');
		fireEvent.click(screen.getByRole('menuitem', { name: 'Close' }));
		expect(model.getIndexOfTab('research-a')).toBeNull();
		expect(model.getIndexOfTab('research-b')).toBeNull();
		expect(model.getIndexOfTab('pinned')).not.toBeNull();
	});

	it('uses native command labels, roles, and multi-tab pin direction', () => {
		const model = createModel();
		model.setSelection(['research-a', 'research-b'], 'research-a');
		render(<TabStrip model={model} />);

		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Research A/ }), {
			clientX: 50,
			clientY: 30,
		});
		expect(screen.getByRole('menuitem', { name: 'Duplicate' })).toBeTruthy();
		expect(screen.queryByRole('menuitemcheckbox', { name: 'Pin' })).toBeNull();
		fireEvent.pointerDown(document.body);
		model.setSelection(['pinned', 'research-a'], 'research-a');
		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Research A/ }), {
			clientX: 50,
			clientY: 30,
		});
		fireEvent.click(screen.getByRole('menuitem', { name: 'Pin' }));
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'pinned')?.pinned).toBe(true);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'research-a')?.pinned).toBe(true);
	});

	it('unpins pinned tabs selected for a new group like MoveTabsAndSetPropertiesImpl', () => {
		const model = createModel();
		model.setSelection(['pinned', 'loose'], 'loose');
		render(<TabStrip model={model} />);

		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Loose/ }), {
			clientX: 50,
			clientY: 30,
		});
		fireEvent.click(screen.getByRole('menuitem', { name: 'Add tabs to group' }));
		fireEvent.click(screen.getByRole('menuitem', { name: 'New group' }));
		const pinned = model.getSnapshot().tabs.find((tab) => tab.id === 'pinned');
		const loose = model.getSnapshot().tabs.find((tab) => tab.id === 'loose');
		expect(pinned?.pinned).toBe(false);
		expect(pinned?.group).toBe(loose?.group);
		expect(pinned?.group).not.toBeNull();
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['loose']);
	});

	it('applies TabStripViewLayout bounds as positioned vertical regions', () => {
		const model = createModel();
		const { container } = render(
			<TabStrip model={model} orientation={TabStripOrientation.Vertical} />,
		);
		const pinned = container.querySelector<HTMLElement>('[data-vertical-region="pinned"]');
		const unpinned = container.querySelector<HTMLElement>('[data-vertical-region="unpinned"]');
		if (!pinned || !unpinned) throw new Error('Expected both vertical tab regions.');

		expect(pinned.style.getPropertyValue('--ob-comp-tab-strip-inline-start')).toBe('12px');
		expect(pinned.style.getPropertyValue('--ob-comp-tab-strip-block-start')).toBe('0px');
		expect(
			Number.parseFloat(unpinned.style.getPropertyValue('--ob-comp-tab-strip-block-start')),
		).toBeGreaterThan(
			Number.parseFloat(pinned.style.getPropertyValue('--ob-comp-tab-strip-block-start')),
		);
		expect(unpinned.getAttribute('style')).not.toContain('x:');
		expect(unpinned.getAttribute('style')).not.toContain('y:');
	});

	it('marks a vertical tab-count layout change for immediate slot placement', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'a', title: 'A', pinned: false, group: null },
				{ id: 'b', title: 'B', pinned: false, group: null },
				{ id: 'c', title: 'C', pinned: false, group: null },
			],
			activeTabId: 'a',
		});
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const content = requireDefined(
			document.querySelector<HTMLElement>('[data-tab-strip-scroll-content]'),
		);
		const c = requireDefined(
			document.querySelector<HTMLElement>('[data-vertical-unpinned-unit="c"]'),
		);
		expect(content.hasAttribute('data-tab-count-changed')).toBe(false);
		expect(c.style.getPropertyValue('--ob-comp-tab-strip-block-start')).toBe('64px');

		act(() => model.closeTab('b'));

		expect(content.hasAttribute('data-tab-count-changed')).toBe(true);
		expect(c.style.getPropertyValue('--ob-comp-tab-strip-block-start')).toBe('32px');
	});

	it('keeps keyboard-only movement out of the native tab menu', () => {
		const model = createModel();
		render(<TabStrip model={model} />);

		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Pinned/ }), {
			clientX: 20,
			clientY: 20,
		});
		expect(screen.queryByRole('menuitem', { name: 'Move tab to first' })).toBeNull();
		expect(screen.queryByRole('menuitem', { name: 'Move tab to last' })).toBeNull();
		expect(
			screen
				.getByRole('menuitem', { name: 'Move tab to new window' })
				.getAttribute('aria-disabled'),
		).toBe('true');
		fireEvent.pointerDown(document.body);

		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Loose/ }), {
			clientX: 20,
			clientY: 20,
		});
		expect(
			screen
				.getByRole('menuitem', { name: 'Close tabs to the right' })
				.getAttribute('aria-disabled'),
		).toBe('true');
		expect(
			screen.getByRole('menuitem', { name: 'Close other tabs' }).hasAttribute('aria-disabled'),
		).toBe(false);
	});

	it('uses the Chromium hover-card timer, preview/footer data, and immediate re-show buffer', () => {
		vi.useFakeTimers();
		const model = createModel();
		model.updateTab('loose', {
			thumbnailUrl: 'data:image/png;base64,AA==',
			memoryUsageBytes: 125 * 1024 * 1024,
		});
		render(<TabStrip model={model} />);
		const strip = screen.getByRole('tablist', { name: 'Tabs' });
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 300, 64);

		fireEvent.mouseEnter(loose);
		expect(strip.hasAttribute('data-hover-card-waiting')).toBe(true);
		expect(document.querySelector('[data-tab-strip-hover-card]')).toBeNull();
		act(() => vi.advanceTimersByTime(300));
		expect(getHoverCard().textContent).toContain('125 MB');
		expect(getHoverCard().querySelector('img')).toBeTruthy();

		fireEvent.mouseLeave(loose);
		expect(document.querySelector('[data-tab-strip-hover-card]')).toBeNull();
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		setRect(researchB, 180, 64);
		fireEvent.mouseEnter(researchB);
		expect(getHoverCard().textContent).toContain('Research B');
		expect(strip.hasAttribute('data-hover-card-waiting')).toBe(false);
	});

	it('dismisses a hover card for a programmatic selection change', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 300, 100);
		fireEvent.focus(loose);
		expect(getHoverCard().textContent).toContain('Loose');

		act(() => model.selectTab('loose'));

		expect(document.querySelector('[data-tab-strip-hover-card]')).toBeNull();
	});

	it('honors the hover-card memory preference while retaining high-memory alerts', () => {
		const model = createModel();
		model.updateTab('loose', { memoryUsageBytes: 1_000, memoryUsageHigh: false });
		render(<TabStrip model={model} showHoverCardMemoryUsage={false} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 300, 100);

		fireEvent.focus(loose);
		expect(getHoverCard().textContent).not.toContain('Memory usage');
		act(() => model.updateTab('loose', { memoryUsageHigh: true }));
		expect(getHoverCard().textContent).toContain('High memory usage: 1000 bytes');
	});

	it('requests asynchronous browser thumbnails and renders observer updates', () => {
		vi.useFakeTimers();
		const model = createModel();
		const source = new FakeThumbnailSource();
		render(<TabStrip model={model} getHoverCardThumbnailSource={() => source} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 300, 64);

		fireEvent.mouseEnter(loose);
		act(() => vi.advanceTimersByTime(300));
		expect(source.requested).toBe(1);
		expect(source.sizeHint).toEqual({ width: 320, height: 180 });
		expect(
			getHoverCard().querySelector('[data-thumbnail-wait-state="waiting-without-placeholder"]'),
		).toBeTruthy();

		act(() => source.emit('data:image/png;base64,AA=='));
		expect(getHoverCard().querySelector('img')?.getAttribute('src')).toBe(
			'data:image/png;base64,AA==',
		);
	});

	it('keeps asynchronous thumbnail observation alive through StrictMode effect replay', () => {
		vi.useFakeTimers();
		const model = createModel();
		const source = new FakeThumbnailSource();
		render(
			<StrictMode>
				<TabStrip model={model} getHoverCardThumbnailSource={() => source} />
			</StrictMode>,
		);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 300, 64);

		fireEvent.mouseEnter(loose);
		act(() => vi.advanceTimersByTime(300));
		act(() => source.emit('data:image/png;base64,STRICT=='));
		expect(getHoverCard().querySelector('img')?.getAttribute('src')).toBe(
			'data:image/png;base64,STRICT==',
		);
	});

	it('shows a keyboard-focused tab hover card immediately and hides it on any window key press', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 300, 100);

		fireEvent.focus(loose);
		expect(getHoverCard().textContent).toContain('Loose');
		fireEvent.keyDown(window, { key: 'ArrowDown' });
		expect(document.querySelector('[data-tab-strip-hover-card]')).toBeNull();
	});

	it('keeps the hover card visible for keys handled by a focused tab', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 300, 100);

		fireEvent.focus(loose);
		expect(getHoverCard().textContent).toContain('Loose');
		fireEvent.keyDown(loose, { key: ' ' });
		expect(getHoverCard().textContent).toContain('Loose');
	});

	it('forwards tab keyboard focus entry and strip exit to the host controller', () => {
		const model = createModel();
		const onTabKeyboardFocusChanged = vi.fn<() => void>();
		render(<TabStrip model={model} onTabKeyboardFocusChanged={onTabKeyboardFocusChanged} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });

		fireEvent.focus(loose);
		expect(onTabKeyboardFocusChanged).toHaveBeenLastCalledWith('loose');
		fireEvent.blur(loose, { relatedTarget: document.body });
		expect(onTabKeyboardFocusChanged).toHaveBeenLastCalledWith(null);
	});

	it('shows a group hover card with its first five member titles', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		setRect(group, 60, 64);

		fireEvent.mouseEnter(group);
		act(() => vi.advanceTimersByTime(300));
		const card = getHoverCard();
		expect(card.textContent).toContain('Research · 2 tabs');
		expect(card.textContent).toContain('Research A');
		expect(card.textContent).toContain('Research B');
		expect(card.querySelector('img')).toBeNull();
	});

	it('falls back to the native group tooltip when group hover cards are disabled', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} showGroupHoverCards={false} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		setRect(group, 60, 64);

		expect(group.getAttribute('title')).toBe('Research - Research A and 1 other tab');
		fireEvent.mouseEnter(group);
		act(() => vi.advanceTimersByTime(500));
		expect(document.querySelector('[data-tab-strip-hover-card]')).toBeNull();
		fireEvent.focus(group);
		expect(document.querySelector('[data-tab-strip-hover-card]')).toBeNull();
	});

	it('uses singular overflow text for a six-tab group hover card', () => {
		vi.useFakeTimers();
		const model = createModel();
		for (let index = 0; index < 4; index += 1) {
			model.addTab({ title: `Extra ${index + 1}`, pinned: false, group: 'research' });
		}
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', { name: /^Research, 6 tabs/ });
		setRect(group, 60, 64);

		fireEvent.mouseEnter(group);
		act(() => vi.advanceTimersByTime(300));
		expect(getHoverCard().textContent).toContain('+ 1 more');
	});

	it('uses singular count text for a one-tab group hover card', () => {
		vi.useFakeTimers();
		const model = createModel();
		model.moveTabsToGroup(['research-b'], null);
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', { name: /^Research, 1 tab/ });
		setRect(group, 60, 64);

		fireEvent.mouseEnter(group);
		act(() => vi.advanceTimersByTime(300));
		expect(getHoverCard().textContent).toContain('Research · 1 tab');
		expect(getHoverCard().textContent).not.toContain('1 tabs');
	});

	it('does not mount an empty hover-card domain label when URL display is disabled', () => {
		const model = createModel();
		model.updateTab('loose', { shouldDisplayUrl: false });
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 300, 100);

		fireEvent.focus(loose);
		expect(getHoverCard().querySelector('[data-hover-card-domain]')).toBeNull();
	});

	it('closes an unpinned tab immediately with the middle mouse button', () => {
		const model = createModel();
		render(<TabStrip model={model} />);

		fireEvent(
			screen.getByRole('tab', { name: /^Loose/ }),
			new MouseEvent('auxclick', { button: 1, bubbles: true }),
		);

		expect(model.getIndexOfTab('loose')).toBeNull();
		expect(
			screen.getByRole('tablist', { name: 'Tabs' }).getAttribute('data-tab-closing-mode'),
		).toBe('mouse');
		expect(screen.getByRole('status').textContent).toBe('Closed Loose');
	});

	it('does not close a tab when a middle-button press is released elsewhere', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });

		fireEvent.pointerDown(loose, { button: 1, pointerId: 31, clientX: 300, clientY: 20 });
		fireEvent.pointerUp(window, { button: 1, pointerId: 31, clientX: -10, clientY: -10 });

		expect(model.getIndexOfTab('loose')).not.toBeNull();
	});

	it('activates the replacement immediately when the active tab closes', () => {
		const model = createModel();
		model.selectTab('loose');
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		expect(loose.hasAttribute('data-active')).toBe(true);
		fireEvent(loose, new MouseEvent('auxclick', { button: 1, bubbles: true }));
		expect(model.getSnapshot().activeTabId).toBe('research-b');
		expect(researchB.hasAttribute('data-active')).toBe(true);
	});

	it('holds touch closing mode for two seconds and exits after the delay', () => {
		vi.useFakeTimers();
		const model = createModel();
		model.selectTab('loose');
		render(<TabStrip model={model} tabStripDeclutter={false} />);
		const close = within(screen.getByRole('tab', { name: /^Loose/ })).getByRole('button', {
			name: 'Close',
		});

		fireEvent.pointerDown(close, { button: 0, pointerType: 'touch', clientX: 14, clientY: 14 });
		fireEvent.click(close);
		expect(
			screen.getByRole('tablist', { name: 'Tabs' }).getAttribute('data-tab-closing-mode'),
		).toBe('touch');
		act(() => vi.advanceTimersByTime(1_999));
		expect(
			screen.getByRole('tablist', { name: 'Tabs' }).getAttribute('data-tab-closing-mode'),
		).toBe('touch');
		act(() => vi.advanceTimersByTime(1));
		expect(
			screen.getByRole('tablist', { name: 'Tabs' }).hasAttribute('data-tab-closing-mode'),
		).toBe(false);
	});

	it('does not treat horizontal drag settling as an active touch drag', () => {
		vi.useFakeTimers();
		const model = new TabModel({
			tabs: [
				{ id: 'a', title: 'A', pinned: false, group: null },
				{ id: 'b', title: 'B', pinned: false, group: null },
				{ id: 'c', title: 'C', pinned: false, group: null },
			],
			activeTabId: 'c',
		});
		render(<TabStrip model={model} tabStripDeclutter={false} />);
		const close = within(screen.getByRole('tab', { name: /^C/ })).getByRole('button', {
			name: 'Close',
		});

		fireEvent.pointerDown(close, {
			button: 0,
			pointerType: 'touch',
			clientX: 250,
			clientY: 20,
		});
		fireEvent.click(close);
		const strip = screen.getByRole('tablist', { name: 'Tabs' });
		expect(strip.getAttribute('data-tab-closing-mode')).toBe('touch');

		const a = screen.getByRole('tab', { name: /^A/ });
		const b = screen.getByRole('tab', { name: /^B/ });
		setRect(a, 0, 100);
		setRect(b, 100, 100);
		fireEvent.pointerDown(b, {
			button: 0,
			pointerId: 92,
			pointerType: 'mouse',
			clientX: 150,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 92,
			pointerType: 'mouse',
			clientX: 20,
			clientY: 20,
		});
		fireEvent.pointerUp(window, {
			pointerId: 92,
			pointerType: 'mouse',
			clientX: 20,
			clientY: 20,
		});
		expect(document.querySelector('[data-horizontal-drag-context][data-settling]')).not.toBeNull();

		act(() => vi.advanceTimersByTime(2_000));
		expect(strip.hasAttribute('data-tab-closing-mode')).toBe(false);
	});

	it('suspends the mouse closing-mode watcher while the group editor bubble is open', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		fireEvent(
			screen.getByRole('tab', { name: /^Loose/ }),
			new MouseEvent('auxclick', { button: 1, bubbles: true }),
		);
		const strip = screen.getByRole('tablist', { name: 'Tabs' });
		expect(strip.getAttribute('data-tab-closing-mode')).toBe('mouse');

		fireEvent.contextMenu(screen.getByRole('tablist', { name: /^Research, 2 tabs/ }), {
			clientX: 80,
			clientY: 20,
		});
		expect(screen.getByRole('textbox', { name: 'Group name' })).toBeTruthy();
		fireEvent.pointerMove(window, { clientX: -100, clientY: -100 });
		expect(strip.getAttribute('data-tab-closing-mode')).toBe('mouse');

		fireEvent.keyDown(screen.getByRole('textbox', { name: 'Group name' }), { key: 'Escape' });
		expect(screen.queryByRole('menu')).toBeNull();
		fireEvent.pointerMove(window, { clientX: -100, clientY: -100 });
		expect(strip.hasAttribute('data-tab-closing-mode')).toBe(false);
	});

	it('exits closing mode when a tab is inserted', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} />);
		fireEvent(
			screen.getByRole('tab', { name: /^Loose/ }),
			new MouseEvent('auxclick', { button: 1, bubbles: true }),
		);
		expect(
			screen.getByRole('tablist', { name: 'Tabs' }).getAttribute('data-tab-closing-mode'),
		).toBe('mouse');
		act(() => model.addTab({ title: 'Inserted', pinned: false, group: null }));
		expect(
			screen.getByRole('tablist', { name: 'Tabs' }).hasAttribute('data-tab-closing-mode'),
		).toBe(false);
	});

	it('renders the collapsed vertical strip with preserved accessible labels', () => {
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} verticalCollapsed />);

		expect(screen.getByRole('tablist', { name: 'Tabs' }).getAttribute('aria-orientation')).toBe(
			'vertical',
		);
		expect(screen.getByRole('tab', { name: /^Research A/ })).toBeTruthy();
		expect(screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ })).toBeTruthy();
	});

	it('marks a vertical group fully collapsed only when its grid transition finishes', () => {
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		fireEvent.click(screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ }));

		const group = requireDefined(
			document.querySelector<HTMLElement>('[data-tab-strip-group][data-group-id="research"]'),
		);
		const groupTabs = requireDefined(
			group.querySelector<HTMLElement>('[data-tab-strip-group-tabs]'),
		);
		expect(group.hasAttribute('data-group-collapsing')).toBe(true);
		expect(group.hasAttribute('data-group-collapsed')).toBe(false);
		expect(groupTabs.hasAttribute('aria-hidden')).toBe(false);

		fireEvent.transitionEnd(groupTabs, { propertyName: 'grid-template-rows' });
		expect(group.hasAttribute('data-group-collapsing')).toBe(false);
		expect(group.hasAttribute('data-group-collapsed')).toBe(true);
		expect(groupTabs.getAttribute('aria-hidden')).toBe('true');
	});

	it('moves the following vertical slot as soon as group collapse starts', () => {
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const groupUnit = requireDefined(
			document.querySelector<HTMLElement>('[data-vertical-unpinned-unit="group-research"]'),
		);
		const looseUnit = requireDefined(
			document.querySelector<HTMLElement>('[data-vertical-unpinned-unit="loose"]'),
		);
		expect(groupUnit.style.getPropertyValue('--ob-comp-tab-strip-block-size')).toBe('96px');
		expect(looseUnit.style.getPropertyValue('--ob-comp-tab-strip-block-start')).toBe('98px');

		fireEvent.click(screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ }));

		expect(groupUnit.style.getPropertyValue('--ob-comp-tab-strip-block-size')).toBe('34px');
		expect(looseUnit.style.getPropertyValue('--ob-comp-tab-strip-block-start')).toBe('36px');
		expect(
			document
				.querySelector<HTMLElement>('[data-tab-strip-group][data-group-id="research"]')
				?.hasAttribute('data-group-collapsing'),
		).toBe(true);
	});

	it('snaps an expanded vertical group closed before live drag reordering begins', () => {
		const requestAnimationFrameDescriptor = Object.getOwnPropertyDescriptor(
			window,
			'requestAnimationFrame',
		);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		Object.defineProperty(window, 'requestAnimationFrame', {
			configurable: true,
			value: requestAnimationFrame,
		});
		try {
			const model = createModel();
			const moveGroupByDrag = vi.spyOn(model, 'moveGroupByDrag');
			render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
			const strip = screen.getByRole('tablist', { name: 'Tabs' });
			const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
			const groupView = requireDefined(group.closest<HTMLElement>('[data-tab-strip-group]'));
			const researchA = screen.getByRole('tab', { name: /^Research A/ });
			const researchB = screen.getByRole('tab', { name: /^Research B/ });
			const loose = screen.getByRole('tab', { name: /^Loose/ });
			setRect(strip, 0, 240, 0, 500);
			setRect(groupView, 0, 216, 50, 96);
			setRect(group, 0, 216, 50, 26);
			setRect(researchA, 12, 204, 82, 30);
			setRect(researchB, 12, 204, 114, 30);
			setRect(loose, 0, 216, 148, 30);

			const scheduledBeforeDrag = requestAnimationFrame.mock.calls.length;
			fireEvent.pointerDown(group, { button: 0, pointerId: 193, clientX: 100, clientY: 65 });
			fireEvent.pointerMove(window, { pointerId: 193, clientX: 112, clientY: 65 });

			const scrollContent = requireDefined(
				document.querySelector<HTMLElement>('[data-tab-strip-scroll-content]'),
			);
			expect(model.getGroup('research')?.collapsed).toBe(true);
			expect(scrollContent.hasAttribute('data-group-drag-pickup')).toBe(true);
			expect(moveGroupByDrag).not.toHaveBeenCalled();

			const pickupFrame = requireDefined(
				requestAnimationFrame.mock.calls[scheduledBeforeDrag]?.[0],
			);
			act(() => pickupFrame(performance.now()));

			expect(scrollContent.hasAttribute('data-group-drag-pickup')).toBe(false);
			expect(moveGroupByDrag).toHaveBeenCalledOnce();
			fireEvent.pointerUp(window, { pointerId: 193, clientX: 112, clientY: 65 });
			expect(model.getGroup('research')?.collapsed).toBe(false);
		} finally {
			if (requestAnimationFrameDescriptor) {
				Object.defineProperty(window, 'requestAnimationFrame', requestAnimationFrameDescriptor);
			} else {
				Reflect.deleteProperty(window, 'requestAnimationFrame');
			}
		}
	});

	it('does not let a completed vertical group drag swallow the next real toggle', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		const groupView = requireDefined(group.closest<HTMLElement>('[data-tab-strip-group]'));
		setRect(groupView, 0, 216, 50, 96);
		setRect(group, 0, 216, 54, 26);

		fireEvent.pointerDown(group, { button: 0, pointerId: 191, clientX: 100, clientY: 65 });
		fireEvent.pointerMove(window, { pointerId: 191, clientX: 100, clientY: 130 });
		fireEvent.pointerUp(window, { pointerId: 191, clientX: 100, clientY: 300 });
		expect(model.getGroup('research')?.collapsed).toBe(false);

		act(() => vi.advanceTimersByTime(0));
		fireEvent.click(screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ }));
		expect(model.getGroup('research')?.collapsed).toBe(true);
	});

	it('exposes the vertical group editor button separately from collapse', () => {
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const options = screen.getByRole('button', { name: 'More options', hidden: true });
		fireEvent.click(options);
		expect(screen.getByRole('menu')).toBeTruthy();
		expect(screen.getByRole('textbox', { name: 'Group name' })).toBeTruthy();
		expect(model.getSnapshot().groups[0]?.collapsed).toBe(false);
	});

	it('dismisses vertical hover cards and the group editor when tabs scroll', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 0, 216, 100, 32);
		fireEvent.mouseEnter(loose);
		act(() => vi.advanceTimersByTime(300));
		expect(getHoverCard()).toBeTruthy();

		const scroller = requireDefined(
			document.querySelector('[data-vertical-region="unpinned"] [data-tab-strip-scroll-viewport]'),
		);
		fireEvent.scroll(scroller);
		expect(document.querySelector('[data-tab-strip-hover-card]')).toBeNull();

		fireEvent.click(screen.getByRole('button', { name: 'More options', hidden: true }));
		expect(screen.getByRole('textbox', { name: 'Group name' })).toBeTruthy();
		fireEvent.scroll(scroller);
		expect(screen.getByRole('textbox', { name: 'Group name' })).toBeTruthy();
		act(() => vi.advanceTimersByTime(250));
		fireEvent.scroll(scroller);
		expect(screen.queryByRole('textbox', { name: 'Group name' })).toBeNull();
	});

	it('closes the group editor when the tab-strip orientation changes', () => {
		const model = createModel();
		const { rerender } = render(
			<TabStrip model={model} orientation={TabStripOrientation.Vertical} />,
		);
		fireEvent.click(screen.getByRole('button', { name: 'More options', hidden: true }));
		expect(screen.getByRole('textbox', { name: 'Group name' })).toBeTruthy();

		rerender(<TabStrip model={model} orientation={TabStripOrientation.Horizontal} />);
		expect(screen.queryByRole('textbox', { name: 'Group name' })).toBeNull();
	});

	it('renders collapsed group-attention visuals', () => {
		const model = createModel();
		model.selectTab('loose');
		model.updateGroup('research', { attention: true, collapsed: true });
		render(<TabStrip model={model} />);

		const header = screen.getByRole('tablist', { name: /^Research, 2 tabs.*collapsed/ });
		const chip = header.querySelector<HTMLElement>('[data-group-chip]');
		expect(
			Number.parseFloat(chip?.style.getPropertyValue('--ob-comp-tab-strip-inline-size') ?? ''),
		).toBeGreaterThan(0);
		expect(chip?.hasAttribute('data-attention')).toBe(true);
		expect(chip?.querySelector('[data-group-attention]')).toBeTruthy();

		fireEvent.click(header);
		expect(header.querySelector('[data-group-attention]')).toBeNull();
	});

	it('runs group-editor color, new-tab, and move-window commands', () => {
		const model = createModel();
		const onMove = vi.fn<() => void>();
		render(<TabStrip model={model} onMoveGroupToNewWindow={onMove} />);
		const openEditor = () =>
			fireEvent.contextMenu(screen.getByRole('tablist', { name: /^Research, \d+ tabs/ }), {
				clientX: 20,
				clientY: 20,
			});

		openEditor();
		const blue = screen.getByRole('radio', { name: 'blue' });
		fireEvent.keyDown(blue, { key: 'ArrowRight' });
		expect(model.getGroup('research')?.color).toBe('red');
		expect(document.activeElement).toBe(screen.getByRole('radio', { name: 'red' }));
		fireEvent.click(screen.getByRole('menuitem', { name: 'New tab in group' }));
		expect(model.getSnapshot().tabs.filter((tab) => tab.group === 'research')).toHaveLength(3);

		openEditor();
		fireEvent.click(screen.getByRole('menuitem', { name: 'Move group to new window' }));
		expect(onMove).toHaveBeenCalledWith('research');
	});

	it('hides move-window for the only group', () => {
		const model = createModel();
		model.closeTabs(['pinned', 'loose']);
		const onMove = vi.fn<() => void>();
		render(<TabStrip model={model} onMoveGroupToNewWindow={onMove} />);

		fireEvent.contextMenu(screen.getByRole('tablist', { name: /^Research, 2 tabs/ }), {
			clientX: 20,
			clientY: 20,
		});
		expect(screen.queryByRole('menuitem', { name: 'Move group to new window' })).toBeNull();
	});

	it('adds a new tab to the most recently activated group, not the last group in model order', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'work', title: 'Work tab', pinned: false, group: 'work-group' },
				{ id: 'read', title: 'Read tab', pinned: false, group: 'read-group' },
				{ id: 'outside', title: 'Outside', pinned: false, group: null },
			],
			groups: [
				{ id: 'read-group', title: 'Read', color: 'green', collapsed: false },
				{ id: 'work-group', title: 'Work', color: 'blue', collapsed: false },
			],
			activeTabId: 'work',
		});
		model.selectTab('read');
		model.selectTab('outside');
		render(<TabStrip model={model} />);

		fireEvent.contextMenu(screen.getByRole('button', { name: 'New tab' }), {
			clientX: 20,
			clientY: 20,
		});
		fireEvent.click(screen.getByRole('menuitem', { name: 'New tab in Read' }));

		expect(model.getSnapshot().tabs.filter((tab) => tab.group === 'read-group')).toHaveLength(2);
		expect(model.getSnapshot().tabs.filter((tab) => tab.group === 'work-group')).toHaveLength(1);
	});

	it('wires the vertical top-container actions and expand-on-hover preference menu', () => {
		const model = createModel();
		const onCollapsedChange = vi.fn<() => void>();
		const onExpandOnHoverChange = vi.fn<() => void>();
		render(
			<TabStrip
				model={model}
				orientation={TabStripOrientation.Vertical}
				onVerticalCollapsedChange={onCollapsedChange}
				onVerticalExpandOnHoverChange={onExpandOnHoverChange}
			/>,
		);

		fireEvent.click(screen.getByRole('button', { name: 'Collapse tab strip' }));
		expect(onCollapsedChange).toHaveBeenCalledWith(true);

		fireEvent.contextMenu(screen.getByRole('button', { name: 'Collapse tab strip' }), {
			clientX: 20,
			clientY: 20,
		});
		const preference = screen.getByRole('menuitemcheckbox', { name: 'Expand tab strip on hover' });
		expect(preference.getAttribute('aria-checked')).toBe('true');
		fireEvent.click(preference);
		expect(onExpandOnHoverChange).toHaveBeenCalledWith(false);
	});

	it('shows unpinned Tab Search while its bubble is open and hides it after the ephemeral delay', () => {
		vi.useFakeTimers();
		const controller = new TabStripComboButtonController({ richAnimations: false });
		const { rerender } = render(
			<TabStrip
				model={createModel()}
				orientation={TabStripOrientation.Vertical}
				verticalComboButtonController={controller}
			/>,
		);
		expect(screen.queryByRole('button', { name: 'Search tabs' })).toBeNull();

		rerender(
			<TabStrip
				model={createModel()}
				orientation={TabStripOrientation.Vertical}
				verticalComboButtonController={controller}
				verticalTabSearchOpen
			/>,
		);
		expect(screen.getByRole('button', { name: 'Search tabs' })).toBeTruthy();

		rerender(
			<TabStrip
				model={createModel()}
				orientation={TabStripOrientation.Vertical}
				verticalComboButtonController={controller}
			/>,
		);
		act(() => vi.advanceTimersByTime(1_999));
		expect(screen.getByRole('button', { name: 'Search tabs' })).toBeTruthy();
		act(() => vi.advanceTimersByTime(1));
		expect(screen.queryByRole('button', { name: 'Search tabs' })).toBeNull();
	});

	it('holds the vertical expand-on-hover state while the Tab Search bubble is open', () => {
		const comboController = new TabStripComboButtonController({ richAnimations: false });
		const expandController = new VerticalTabStripExpandOnHoverController();
		const model = createModel();
		const { rerender } = render(
			<TabStrip
				model={model}
				orientation={TabStripOrientation.Vertical}
				verticalCollapsed
				verticalComboButtonController={comboController}
				verticalExpandOnHoverController={expandController}
				verticalTabSearchOpen
			/>,
		);
		expect(expandController.getSnapshot().keepCurrentStateLocks).toBe(1);

		rerender(
			<TabStrip
				model={model}
				orientation={TabStripOrientation.Vertical}
				verticalCollapsed
				verticalComboButtonController={comboController}
				verticalExpandOnHoverController={expandController}
			/>,
		);
		expect(expandController.getSnapshot().keepCurrentStateLocks).toBe(0);
	});

	it('notifies the host before invoking the New Tab action', () => {
		const model = createModel();
		const order: string[] = [];
		const runtime = new ActionRuntime();
		const parentActionScope = runtime.createScope('workbench');
		render(
			<TabStrip
				actionRuntime={runtime}
				model={model}
				orientation={TabStripOrientation.Vertical}
				onNewTabButtonPressed={() => order.push('pressed')}
				onNewTab={(request) => {
					order.push('action');
					return runtime.execute('test.host-tab.create', undefined, {
						scope: parentActionScope,
						source: request.source,
					});
				}}
				parentActionScope={parentActionScope}
			/>,
		);

		fireEvent.click(screen.getByRole('button', { name: 'New tab' }));
		expect(order).toEqual(['pressed', 'action']);
	});

	it('runs the Chromium new-tab button context menu commands', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const newTabButton = screen.getByRole('button', { name: 'New tab' });

		fireEvent.contextMenu(newTabButton, { clientX: 1000, clientY: 30 });
		expect(screen.getByRole('menu', { name: 'New tab menu' })).toBeTruthy();
		fireEvent.click(screen.getByRole('menuitem', { name: 'New tab in Research' }));
		expect(model.getSnapshot().tabs.filter((tab) => tab.group === 'research')).toHaveLength(3);

		fireEvent.contextMenu(newTabButton, { clientX: 400, clientY: 30 });
		fireEvent.click(screen.getByRole('menuitem', { name: 'New group' }));
		expect(model.getSnapshot().groups).toHaveLength(2);
		expect(
			model.getSnapshot().tabs.find((tab) => tab.id === model.getSnapshot().activeTabId)?.group,
		).toBe('group-1');
	});

	it('labels an unnamed most-recent group by its tab count', () => {
		const model = createModel();
		model.updateGroup('research', { title: '' });
		render(<TabStrip model={model} />);

		fireEvent.contextMenu(screen.getByRole('button', { name: 'New tab' }), {
			clientX: 400,
			clientY: 30,
		});
		expect(screen.getByRole('menuitem', { name: 'New tab in 2 tabs' })).toBeTruthy();
	});

	it("expands a collapsed vertical strip after Chromium's inside-edge velocity delay", () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} verticalCollapsed />);
		const strip = screen.getByRole('tablist', { name: 'Tabs' });

		fireEvent.mouseEnter(strip);
		expect(strip.hasAttribute('data-waiting-to-expand')).toBe(true);
		act(() => vi.advanceTimersByTime(209));
		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(false);
		act(() => vi.advanceTimersByTime(1));
		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(true);
		expect(strip.hasAttribute('data-collapsed')).toBe(false);
		expect(screen.getByRole('button', { name: 'Expand tab strip' })).toBeTruthy();

		fireEvent.mouseLeave(strip);
		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(false);
		expect(strip.hasAttribute('data-collapsed')).toBe(true);
	});

	it('collapses immediately when its own control holds pointer and keyboard focus', () => {
		const model = createModel();
		const ControlledVerticalTabStrip = () => {
			const [collapsed, setCollapsed] = useState(false);
			return (
				<TabStrip
					model={model}
					orientation={TabStripOrientation.Vertical}
					verticalCollapsed={collapsed}
					onVerticalCollapsedChange={setCollapsed}
				/>
			);
		};
		render(<ControlledVerticalTabStrip />);
		const strip = screen.getByRole('tablist', { name: 'Tabs' });
		const collapseButton = screen.getByRole('button', { name: 'Collapse tab strip' });

		fireEvent.mouseEnter(strip);
		fireEvent.focus(collapseButton);
		fireEvent.click(collapseButton);

		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(false);
		expect(strip.hasAttribute('data-collapsed')).toBe(true);
	});

	it('falls back safely when Fast Refresh preserves a pre-velocity controller', () => {
		const setHovered = vi.fn<() => void>();
		const snapshot = Object.freeze({
			revision: 0,
			expanded: false,
			waitingToExpand: false,
			forceCollapseLocks: 0,
			keepCurrentStateLocks: 0,
			keepExpandedLocks: 0,
		});
		const legacyController = {
			subscribe: () => () => undefined,
			getSnapshot: () => snapshot,
			setEnvironment: vi.fn<() => void>(),
			setHovered,
			setFocused: vi.fn<() => void>(),
			getLock: vi.fn<() => void>(),
		} as unknown as VerticalTabStripExpandOnHoverController;
		render(
			<TabStrip
				model={createModel()}
				orientation={TabStripOrientation.Vertical}
				verticalCollapsed
				verticalExpandOnHoverController={legacyController}
			/>,
		);

		expect(() => fireEvent.mouseEnter(screen.getByRole('tablist', { name: 'Tabs' }))).not.toThrow();
		expect(setHovered).toHaveBeenCalledWith(true);
	});

	it('keeps hover expansion locked while a tab menu is open', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} verticalCollapsed />);
		const strip = screen.getByRole('tablist', { name: 'Tabs' });
		fireEvent.mouseEnter(strip);
		act(() => vi.advanceTimersByTime(210));

		fireEvent.contextMenu(screen.getByRole('tab', { name: /^Loose/ }), {
			clientX: 50,
			clientY: 30,
		});
		fireEvent.mouseEnter(screen.getByRole('menu'));
		fireEvent.mouseLeave(strip);
		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(true);

		fireEvent.click(screen.getByRole('menuitem', { name: 'Duplicate' }));
		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(false);
	});

	it('reorders live after the scaled drag threshold and restores on cancel', () => {
		const model = createModel();
		const onDraggingChange = vi.fn<() => void>();
		render(<TabStrip model={model} onDraggingChange={onDraggingChange} />);
		const pinned = screen.getByRole('tab', { name: /^Pinned/ });
		const researchA = screen.getByRole('tab', { name: /^Research A/ });
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(pinned, 0, 50);
		setRect(researchA, 50, 100);
		setRect(researchB, 150, 100);
		setRect(loose, 250, 100);

		fireEvent.pointerDown(loose, { button: 0, pointerId: 9, clientX: 300, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 9, clientX: 60, clientY: 20 });
		expect(onDraggingChange).toHaveBeenLastCalledWith(true);
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'loose',
			'research-a',
			'research-b',
		]);

		fireEvent.pointerCancel(window, { pointerId: 9, clientX: 60, clientY: 20 });
		expect(onDraggingChange).toHaveBeenLastCalledWith(false);
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'research-a',
			'research-b',
			'loose',
		]);
		expect(
			model
				.getSnapshot()
				.tabs.filter((tab) => tab.group === 'research')
				.map((tab) => tab.id),
		).toEqual(['research-a', 'research-b']);
	});

	it('keeps the new-tab button fixed and does not resubmit a horizontal reorder', () => {
		const requestAnimationFrameDescriptor = Object.getOwnPropertyDescriptor(
			window,
			'requestAnimationFrame',
		);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		Object.defineProperty(window, 'requestAnimationFrame', {
			configurable: true,
			value: requestAnimationFrame,
		});
		const rect = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 800,
			bottom: 40,
			width: 800,
			height: 40,
			toJSON: () => ({}),
		} as DOMRect);
		try {
			const model = new TabModel({
				tabs: [
					{ id: 'a', title: 'A', pinned: false, group: null },
					{ id: 'b', title: 'B', pinned: false, group: null },
				],
				activeTabId: 'b',
			});
			const moveTabsByDrag = vi.spyOn(model, 'moveTabsByDrag');
			render(<TabStrip model={model} />);
			const a = screen.getByRole('tab', { name: /^A/ });
			const b = screen.getByRole('tab', { name: /^B/ });
			const newTabButton = screen.getByRole('button', { name: 'New tab' });
			const initialNewTabButtonLeft = newTabButton.style.getPropertyValue(
				'--ob-comp-tab-strip-inline-start',
			);
			expect(initialNewTabButtonLeft).not.toBe('');
			setRect(a, 0, 100);
			setRect(b, 100, 100);

			fireEvent.pointerDown(b, { button: 0, pointerId: 90, clientX: 150, clientY: 20 });
			fireEvent.pointerMove(window, { pointerId: 90, clientX: 20, clientY: 20 });
			expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['b', 'a']);
			expect(moveTabsByDrag).toHaveBeenCalledOnce();
			expect(requestAnimationFrame).toHaveBeenCalledOnce();
			expect(newTabButton.style.getPropertyValue('--ob-comp-tab-strip-inline-start')).toBe(
				initialNewTabButtonLeft,
			);

			fireEvent.pointerUp(window, { pointerId: 90, clientX: 20, clientY: 20 });
			expect(moveTabsByDrag).toHaveBeenCalledOnce();
			expect(newTabButton.style.getPropertyValue('--ob-comp-tab-strip-inline-start')).toBe(
				initialNewTabButtonLeft,
			);
			const settlingContext = requireDefined(
				document.querySelector<HTMLElement>('[data-horizontal-drag-context][data-settling]'),
			);
			const animationFrame = requireDefined(requestAnimationFrame.mock.calls.at(-1)?.[0]);
			act(() => animationFrame(performance.now() + TAB_BOUNDS_ANIMATION_DURATION_MS));
			const scheduledFrames = requestAnimationFrame.mock.calls.length;
			fireEvent.transitionEnd(settlingContext, { propertyName: 'left' });

			expect(document.querySelector('[data-horizontal-drag-context]')).toBeNull();
			expect(newTabButton.style.getPropertyValue('--ob-comp-tab-strip-inline-start')).toBe(
				initialNewTabButtonLeft,
			);
			expect(requestAnimationFrame).toHaveBeenCalledTimes(scheduledFrames);
		} finally {
			rect.mockRestore();
			if (requestAnimationFrameDescriptor) {
				Object.defineProperty(window, 'requestAnimationFrame', requestAnimationFrameDescriptor);
			} else {
				Reflect.deleteProperty(window, 'requestAnimationFrame');
			}
		}
	});

	it('keeps the hysteresis anchor until the model order changes', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'loose', title: 'Loose', pinned: false, group: null },
				{ id: 'group-a', title: 'Group A', pinned: false, group: 'research' },
				{ id: 'group-b', title: 'Group B', pinned: false, group: 'research' },
			],
			groups: [{ id: 'research', title: 'Research', color: 'red', collapsed: false }],
			activeTabId: 'loose',
		});
		const moveTabsByDrag = vi.spyOn(model, 'moveTabsByDrag');
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		const groupA = screen.getByRole('tab', { name: /^Group A/ });
		const groupB = screen.getByRole('tab', { name: /^Group B/ });
		setRect(loose, 0, 100);
		setRect(group, 100, 70);
		setRect(groupA, 170, 100);
		setRect(groupB, 270, 100);

		fireEvent.pointerDown(loose, { button: 0, pointerId: 10, clientX: 50, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 10, clientX: 120, clientY: 20 });
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.group).toBe('research');
		expect(moveTabsByDrag).toHaveBeenCalledOnce();
		expect(document.querySelector('[data-drag-group-underline]')).toBeNull();
		const horizontalDragContext = requireDefined(
			document.querySelector<HTMLElement>('[data-horizontal-drag-context]'),
		);
		expect(horizontalDragContext.style.getPropertyValue('--ob-comp-tab-strip-runtime-layer')).toBe(
			'24',
		);
		expect(
			document
				.querySelector<HTMLElement>('[data-group-underline-id="research"]')
				?.style.getPropertyValue('--ob-comp-tab-strip-runtime-layer'),
		).toBe('25');

		fireEvent.pointerMove(window, { pointerId: 10, clientX: 125, clientY: 20 });
		expect(moveTabsByDrag).toHaveBeenCalledTimes(2);
		fireEvent.pointerCancel(window, { pointerId: 10, clientX: 125, clientY: 20 });
	});

	it('submits the first attached drag position before applying horizontal hysteresis', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'a', title: 'A', pinned: false, group: null },
				{ id: 'b', title: 'B', pinned: false, group: null },
			],
			activeTabId: 'b',
		});
		const moveTabsByDrag = vi.spyOn(model, 'moveTabsByDrag');
		render(<TabStrip model={model} />);
		const a = screen.getByRole('tab', { name: /^A/ });
		const b = screen.getByRole('tab', { name: /^B/ });
		setRect(a, 0, 200);
		setRect(b, 200, 200);

		fireEvent.pointerDown(b, { button: 0, pointerId: 91, clientX: 250, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 91, clientX: 261, clientY: 20 });
		expect(moveTabsByDrag).toHaveBeenCalledOnce();

		fireEvent.pointerMove(window, { pointerId: 91, clientX: 265, clientY: 20 });
		expect(moveTabsByDrag).toHaveBeenCalledOnce();

		fireEvent.pointerMove(window, { pointerId: 91, clientX: 267, clientY: 20 });
		expect(moveTabsByDrag).toHaveBeenCalledTimes(2);
		fireEvent.pointerCancel(window, { pointerId: 91, clientX: 267, clientY: 20 });
	});

	it('expands the normal group underline when an outside tab joins', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'group-a', title: 'Group A', pinned: false, group: 'research' },
				{ id: 'group-b', title: 'Group B', pinned: false, group: 'research' },
				{ id: 'loose', title: 'Loose', pinned: false, group: null },
			],
			groups: [{ id: 'research', title: 'Research', color: 'red', collapsed: false }],
			activeTabId: 'loose',
		});
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		const groupA = screen.getByRole('tab', { name: /^Group A/ });
		const groupB = screen.getByRole('tab', { name: /^Group B/ });
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		const underline = requireDefined(
			document.querySelector<HTMLElement>('[data-group-underline-id="research"]'),
		);
		const initialUnderlineWidth = underline.style.getPropertyValue(
			'--ob-comp-tab-strip-inline-size',
		);
		setRect(group, 0, 70);
		setRect(groupA, 70, 100);
		setRect(groupB, 170, 100);
		setRect(loose, 270, 100);
		setRect(underline, 18, Number(initialUnderlineWidth.slice(0, -2)), 1, 2);

		fireEvent.pointerDown(loose, { button: 0, pointerId: 11, clientX: 320, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 11, clientX: 260, clientY: 20 });
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.group).toBe('research');
		const expandedUnderlineWidth = requireDefined(
			document.querySelector<HTMLElement>(
				'[data-group-underline-id="research"]:not([data-drag-group-underline])',
			),
		).style.getPropertyValue('--ob-comp-tab-strip-inline-size');
		expect(Number(expandedUnderlineWidth.slice(0, -2))).toBeGreaterThan(
			Number(initialUnderlineWidth.slice(0, -2)),
		);
		expect(document.querySelector('[data-drag-group-underline]')).toBeNull();

		fireEvent.pointerMove(window, { pointerId: 11, clientX: 280, clientY: 20 });
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.group).toBe('research');
		expect(
			document
				.querySelector<HTMLElement>(
					'[data-group-underline-id="research"]:not([data-drag-group-underline])',
				)
				?.style.getPropertyValue('--ob-comp-tab-strip-inline-size'),
		).toBe(expandedUnderlineWidth);
		fireEvent.pointerCancel(window, { pointerId: 11, clientX: 280, clientY: 20 });
	});

	it('forwards the React pointer lifecycle and screen coordinates to a cross-window runtime', () => {
		const model = createModel();
		const runtime = {
			start: vi.fn<() => void>(),
			update: vi.fn<() => void>(),
			end: vi.fn<() => void>(),
			getSnapshot: vi.fn<() => { targetActive: boolean }>(() => ({ targetActive: true })),
		};
		const createRuntime = vi.fn<
			(_session: DragSessionData<Readonly<TabGroupData>>) => typeof runtime
		>(() => runtime);
		render(<TabStrip model={model} createCrossWindowDragRuntime={createRuntime} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 250, 100);

		fireEvent.pointerDown(loose, { button: 0, pointerId: 40, clientX: 300, clientY: 20 });
		const session = requireDefined(requireDefined(createRuntime.mock.calls[0])[0]);
		expect(session.sourceViewDragData.contentsId).toBe('loose');
		expect(session.numDraggingTabs).toBe(1);

		fireEvent.pointerMove(window, {
			pointerId: 40,
			clientX: 60,
			clientY: 20,
			screenX: 460,
			screenY: 220,
		});
		expect(runtime.start).toHaveBeenCalledOnce();
		expect(runtime.update).toHaveBeenCalledWith({ x: 460, y: 220 });
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'research-a',
			'research-b',
			'loose',
		]);

		fireEvent.pointerUp(window, { pointerId: 40, clientX: 60, clientY: 20 });
		expect(runtime.end).toHaveBeenCalledWith('complete');
	});

	it('cancels a started cross-window runtime on Escape', () => {
		const model = createModel();
		const runtime = {
			start: vi.fn<() => void>(),
			update: vi.fn<() => void>(),
			end: vi.fn<() => void>(),
			getSnapshot: vi.fn<() => { targetActive: boolean }>(() => ({ targetActive: false })),
		};
		render(<TabStrip model={model} createCrossWindowDragRuntime={() => runtime} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 250, 100);

		fireEvent.pointerDown(loose, { button: 0, pointerId: 41, clientX: 300, clientY: 20 });
		fireEvent.pointerMove(window, {
			pointerId: 41,
			clientX: 60,
			clientY: 20,
			screenX: 60,
			screenY: 20,
		});
		fireEvent.keyDown(window, { key: 'Escape' });

		expect(runtime.end).toHaveBeenCalledWith('cancel');
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'research-a',
			'research-b',
			'loose',
		]);
	});

	it('ends a drag as tab-destroyed when a dragged source tab disappears', () => {
		const model = createModel();
		const runtime = {
			start: vi.fn<() => void>(),
			update: vi.fn<() => void>(),
			end: vi.fn<() => void>(),
			getSnapshot: vi.fn<
				() => {
					targetActive: boolean;
					sourceContextId: string;
					attachedContextId: string;
				}
			>(() => ({
				targetActive: false,
				sourceContextId: 'source',
				attachedContextId: 'source',
			})),
		};
		render(<TabStrip model={model} createCrossWindowDragRuntime={() => runtime} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 250, 100);

		fireEvent.pointerDown(loose, { button: 0, pointerId: 42, clientX: 300, clientY: 20 });
		fireEvent.pointerMove(window, {
			pointerId: 42,
			clientX: 60,
			clientY: 20,
			screenX: 60,
			screenY: 20,
		});
		expect(document.querySelector('[data-horizontal-drag-context]')).not.toBeNull();
		act(() => model.closeTab('loose'));

		expect(runtime.end).toHaveBeenCalledWith('tab-destroyed');
		expect(document.querySelector('[data-horizontal-drag-context]')).toBeNull();
	});

	it('cancels an armed drag before activation when a dragged tab disappears', () => {
		const model = createModel();
		const runtime = {
			start: vi.fn<() => void>(),
			update: vi.fn<() => void>(),
			end: vi.fn<() => void>(),
			getSnapshot: vi.fn<
				() => {
					targetActive: boolean;
					sourceContextId: string;
					attachedContextId: string;
				}
			>(() => ({
				targetActive: false,
				sourceContextId: 'source',
				attachedContextId: 'source',
			})),
		};
		render(<TabStrip model={model} createCrossWindowDragRuntime={() => runtime} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 250, 100);

		fireEvent.pointerDown(loose, { button: 0, pointerId: 103, clientX: 300, clientY: 20 });
		act(() => model.closeTab('loose'));
		fireEvent.pointerMove(window, {
			pointerId: 103,
			clientX: 340,
			clientY: 20,
			screenX: 340,
			screenY: 20,
		});

		expect(runtime.start).not.toHaveBeenCalled();
		expect(runtime.end).not.toHaveBeenCalled();
		expect(document.querySelector('[data-horizontal-drag-context]')).toBeNull();
	});

	it('cancels and restores a live drag when the React strip unmounts', () => {
		const model = createModel();
		const runtime = {
			start: vi.fn<() => void>(),
			update: vi.fn<() => void>(),
			end: vi.fn<() => void>(),
			getSnapshot: vi.fn<() => { targetActive: boolean }>(() => ({ targetActive: false })),
		};
		const { unmount } = render(
			<TabStrip model={model} createCrossWindowDragRuntime={() => runtime} />,
		);
		const pinned = screen.getByRole('tab', { name: /^Pinned/ });
		const researchA = screen.getByRole('tab', { name: /^Research A/ });
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(pinned, 0, 50);
		setRect(researchA, 50, 100);
		setRect(researchB, 150, 100);
		setRect(loose, 250, 100);

		fireEvent.pointerDown(loose, { button: 0, pointerId: 43, clientX: 300, clientY: 20 });
		fireEvent.pointerMove(window, {
			pointerId: 43,
			clientX: 60,
			clientY: 20,
			screenX: 60,
			screenY: 20,
		});
		expect(model.getSnapshot().tabs[1]?.id).toBe('loose');
		unmount();

		expect(runtime.end).toHaveBeenCalledWith('cancel');
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'research-a',
			'research-b',
			'loose',
		]);
	});

	it('moves every view of an expanded group while its backing model is temporarily collapsed', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		const pinned = screen.getByRole('tab', { name: /^Pinned/ });
		const researchA = screen.getByRole('tab', { name: /^Research A/ });
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(pinned, 0, 50);
		setRect(group, 50, 70);
		setRect(researchA, 120, 100);
		setRect(researchB, 220, 100);
		setRect(loose, 320, 100);

		fireEvent.pointerDown(group, { button: 0, pointerId: 12, clientX: 80, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 12, clientX: 500, clientY: 20 });
		expect(model.getGroup('research')?.collapsed).toBe(true);
		expect(document.querySelector('[data-group-id="research"]')).toBe(group);
		expect(document.querySelector('[data-tab-id="research-a"]')).toBe(researchA);
		expect(document.querySelector('[data-tab-id="research-b"]')).toBe(researchB);
		const dragContext = document.querySelector<HTMLElement>('[data-horizontal-drag-context]');
		expect(dragContext?.style.getPropertyValue('--ob-comp-tab-strip-inline-start')).toBe('470px');
		expect(dragContext?.style.getPropertyValue('--ob-comp-tab-strip-inline-size')).toBe('234px');
		expect(dragContext?.style.getPropertyValue('--ob-comp-tab-strip-runtime-layer')).toBe('25');
		expect(document.querySelectorAll('[data-horizontal-drag-view]')).toHaveLength(3);
		expect(dragContext?.querySelector('[data-drag-group-highlight]')).toBeNull();
		expect(dragContext?.querySelector('[data-drag-group-underline]')).not.toBeNull();
		expect(
			document.querySelectorAll(
				'[data-group-underline-id="research"]:not([data-drag-group-underline])',
			),
		).toHaveLength(0);
		fireEvent.pointerMove(window, { pointerId: 12, clientX: 540, clientY: 20 });
		expect(dragContext?.style.getPropertyValue('--ob-comp-tab-strip-inline-start')).toBe('510px');
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'loose',
			'research-a',
			'research-b',
		]);
		expect(
			model
				.getSnapshot()
				.tabs.filter((tab) => tab.group === 'research')
				.map((tab) => tab.id),
		).toEqual(['research-a', 'research-b']);

		fireEvent.pointerUp(window, { pointerId: 12, clientX: 540, clientY: 20 });
		expect(model.getGroup('research')?.collapsed).toBe(false);
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		const settlingContext = document.querySelector<HTMLElement>(
			'[data-horizontal-drag-context][data-settling]',
		);
		expect(settlingContext).not.toBeNull();
		expect(settlingContext?.querySelector('[data-drag-group-highlight]')).toBeNull();
		expect(document.querySelectorAll('[data-horizontal-drag-view]')).toHaveLength(3);
		expect(settlingContext?.hasAttribute('data-group-settling')).toBe(false);
	});

	it('resets horizontal group-header drag selection to its recorded active tab', () => {
		const model = createModel();
		model.setSelection(['pinned', 'loose'], 'loose');
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		const pinned = screen.getByRole('tab', { name: /^Pinned/ });
		const researchA = screen.getByRole('tab', { name: /^Research A/ });
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(pinned, 0, 50);
		setRect(group, 50, 70);
		setRect(researchA, 120, 100);
		setRect(researchB, 220, 100);
		setRect(loose, 320, 100);

		fireEvent.pointerDown(group, { button: 0, pointerId: 62, clientX: 80, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 62, clientX: 500, clientY: 20 });
		fireEvent.pointerUp(window, { pointerId: 62, clientX: 500, clientY: 20 });

		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
	});

	it('activates the first tab without expanding a collapsed horizontal group drag', () => {
		const model = createModel();
		model.selectTab('loose');
		model.updateGroup('research', { collapsed: true });
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*collapsed$/ });
		const researchA = requireDefined(
			document.querySelector<HTMLElement>('[data-tab-id="research-a"]'),
		);
		const researchB = requireDefined(
			document.querySelector<HTMLElement>('[data-tab-id="research-b"]'),
		);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(group, 50, 70);
		setRect(researchA, 120, 100);
		setRect(researchB, 220, 100);
		setRect(loose, 320, 100);

		fireEvent.pointerDown(group, { button: 0, pointerId: 63, clientX: 80, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 63, clientX: 500, clientY: 20 });

		expect(model.getGroup('research')?.collapsed).toBe(true);
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a', 'research-b']);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		const dragContext = document.querySelector<HTMLElement>('[data-horizontal-drag-context]');
		expect(dragContext?.style.getPropertyValue('--ob-comp-tab-strip-inline-size')).toBe('110px');
		expect(document.querySelectorAll('[data-horizontal-drag-view]')).toHaveLength(1);
		expect(dragContext?.querySelector('[data-drag-group-highlight]')).toBeNull();

		fireEvent.pointerUp(window, { pointerId: 63, clientX: 500, clientY: 20 });
		expect(model.getGroup('research')?.collapsed).toBe(true);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		expect(
			screen
				.getByRole('tablist', { name: /^Research, 2 tabs.*collapsed$/ })
				.getAttribute('data-active-tab-id'),
		).toBe('research-a');
		expect(document.querySelector('[data-tab-strip-drag-ghost][data-group-settling]')).toBeNull();
	});

	it('keeps each fully selected group underline independent during a mixed drag', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'a', title: 'A', pinned: false, group: 'one' },
				{ id: 'b', title: 'B', pinned: false, group: 'one' },
				{ id: 'c', title: 'C', pinned: false, group: 'two' },
				{ id: 'd', title: 'D', pinned: false, group: 'two' },
			],
			groups: [
				{ id: 'one', title: 'One', color: 'blue', collapsed: false },
				{ id: 'two', title: 'Two', color: 'red', collapsed: false },
			],
			activeTabId: 'a',
		});
		model.setSelection(['a', 'b', 'c', 'd'], 'a');
		render(<TabStrip model={model} />);
		const one = screen.getByRole('tablist', { name: /^One, 2 tabs/ });
		const two = screen.getByRole('tablist', { name: /^Two, 2 tabs/ });
		const a = screen.getByRole('tab', { name: /^A/ });
		const b = screen.getByRole('tab', { name: /^B/ });
		const c = screen.getByRole('tab', { name: /^C/ });
		const d = screen.getByRole('tab', { name: /^D/ });
		setRect(one, 0, 60);
		setRect(a, 60, 100);
		setRect(b, 160, 100);
		setRect(two, 260, 60);
		setRect(c, 320, 100);
		setRect(d, 420, 100);

		fireEvent.pointerDown(a, { button: 0, pointerId: 86, clientX: 100, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 86, clientX: 120, clientY: 20 });

		const dragContext = document.querySelector('[data-horizontal-drag-context]');
		expect(dragContext?.querySelectorAll('[data-drag-group-highlight]')).toHaveLength(0);
		expect(dragContext?.querySelectorAll('[data-drag-group-underline]')).toHaveLength(2);
		expect(
			dragContext
				?.querySelector<HTMLElement>('[data-group-underline-id="one"]')
				?.style.getPropertyValue('--ob-comp-tab-strip-group-color'),
		).toBe('#1a73e8');
		expect(
			dragContext
				?.querySelector<HTMLElement>('[data-group-underline-id="two"]')
				?.style.getPropertyValue('--ob-comp-tab-strip-group-color'),
		).toBe('#d93025');
		fireEvent.pointerUp(window, { pointerId: 86, clientX: 120, clientY: 20 });
	});

	it('coalesces a nonconsecutive selection on the first drag move before the width threshold', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'a', title: 'A', pinned: false, group: null },
				{ id: 'b', title: 'B', pinned: false, group: null },
				{ id: 'c', title: 'C', pinned: false, group: null },
				{ id: 'd', title: 'D', pinned: false, group: null },
			],
			activeTabId: 'a',
		});
		model.setSelection(['a', 'c'], 'a');
		render(<TabStrip model={model} />);
		const a = screen.getByRole('tab', { name: /^A/ });
		const b = screen.getByRole('tab', { name: /^B/ });
		const c = screen.getByRole('tab', { name: /^C/ });
		const d = screen.getByRole('tab', { name: /^D/ });
		setRect(a, 0, 100);
		setRect(b, 100, 100);
		setRect(c, 200, 100);
		setRect(d, 300, 100);

		fireEvent.pointerDown(a, { button: 0, pointerId: 87, clientX: 50, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 87, clientX: 61, clientY: 20 });

		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['a', 'c', 'b', 'd']);
		expect(document.querySelector('[data-source][data-compacting]')).toBeNull();
		expect(document.querySelectorAll('[data-horizontal-drag-view][data-compacting]')).toHaveLength(
			1,
		);
		fireEvent.pointerUp(window, { pointerId: 87, clientX: 61, clientY: 20 });
	});

	it('keeps a later source tab under the pointer while nonconsecutive peers compact toward it', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'a', title: 'A', pinned: false, group: null },
				{ id: 'b', title: 'B', pinned: false, group: null },
				{ id: 'c', title: 'C', pinned: false, group: null },
				{ id: 'd', title: 'D', pinned: false, group: null },
			],
			activeTabId: 'c',
			selectedTabIds: ['a', 'c'],
		});
		render(<TabStrip model={model} />);
		const a = screen.getByRole('tab', { name: /^A/ });
		const b = screen.getByRole('tab', { name: /^B/ });
		const c = screen.getByRole('tab', { name: /^C/ });
		const d = screen.getByRole('tab', { name: /^D/ });
		setRect(a, 0, 100);
		setRect(b, 100, 100);
		setRect(c, 200, 100);
		setRect(d, 300, 100);

		fireEvent.pointerDown(c, { button: 0, pointerId: 88, clientX: 250, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 88, clientX: 261, clientY: 20 });

		const dragContext = document.querySelector<HTMLElement>('[data-horizontal-drag-context]');
		const source = document.querySelector<HTMLElement>('[data-horizontal-drag-view][data-source]');
		expect(dragContext?.style.getPropertyValue('--ob-comp-tab-strip-inline-start')).toBe('129px');
		expect(source?.style.getPropertyValue('--ob-comp-tab-strip-inline-start')).toBe('211px');
		expect(source?.hasAttribute('data-compacting')).toBe(false);
		fireEvent.pointerUp(window, { pointerId: 88, clientX: 261, clientY: 20 });
	});

	it('continues horizontal compaction while pointer updates retarget the composite', () => {
		const requestAnimationFrameDescriptor = Object.getOwnPropertyDescriptor(
			window,
			'requestAnimationFrame',
		);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		Object.defineProperty(window, 'requestAnimationFrame', {
			configurable: true,
			value: requestAnimationFrame,
		});
		try {
			const model = new TabModel({
				tabs: [
					{ id: 'a', title: 'A', pinned: false, group: null },
					{ id: 'b', title: 'B', pinned: false, group: null },
					{ id: 'c', title: 'C', pinned: false, group: null },
					{ id: 'd', title: 'D', pinned: false, group: null },
				],
				activeTabId: 'c',
				selectedTabIds: ['a', 'c'],
			});
			let session: DragSessionData<Readonly<TabGroupData>> | null = null;
			render(
				<TabStrip
					model={model}
					createCrossWindowDragRuntime={(candidate) => {
						session = candidate;
						return null;
					}}
				/>,
			);
			const a = screen.getByRole('tab', { name: /^A/ });
			const b = screen.getByRole('tab', { name: /^B/ });
			const c = screen.getByRole('tab', { name: /^C/ });
			const d = screen.getByRole('tab', { name: /^D/ });
			setRect(a, 0, 100);
			setRect(b, 100, 100);
			setRect(c, 200, 100);
			setRect(d, 300, 100);

			fireEvent.pointerDown(c, { button: 0, pointerId: 153, clientX: 250, clientY: 20 });
			fireEvent.pointerMove(window, { pointerId: 153, clientX: 261, clientY: 20 });
			const startedSession = requireDefined<DragSessionData<Readonly<TabGroupData>>>(session);
			const startedAt = startedSession.dragStartTime;
			const callbacks = requestAnimationFrame.mock.calls.flatMap((call) =>
				call[0] === undefined ? [] : [call[0]],
			);
			act(() =>
				callbacks.forEach((callback) => callback(startedAt + DRAG_START_ANIMATION_DURATION_MS / 2)),
			);

			const draggedA = requireDefined(
				document.querySelector<HTMLElement>(
					'[data-horizontal-drag-view="tab"]:has([data-tab-id="a"])',
				),
			);
			const source = requireDefined(
				document.querySelector<HTMLElement>('[data-horizontal-drag-view="tab"][data-source]'),
			);
			const relativeAtMidpoint =
				Number.parseFloat(draggedA.style.getPropertyValue('--ob-comp-tab-strip-inline-start')) -
				Number.parseFloat(source.style.getPropertyValue('--ob-comp-tab-strip-inline-start'));
			expect(relativeAtMidpoint).toBe(-111);

			fireEvent.pointerMove(window, { pointerId: 153, clientX: 271, clientY: 20 });
			const relativeAfterRetarget =
				Number.parseFloat(draggedA.style.getPropertyValue('--ob-comp-tab-strip-inline-start')) -
				Number.parseFloat(source.style.getPropertyValue('--ob-comp-tab-strip-inline-start'));
			expect(relativeAfterRetarget).toBe(relativeAtMidpoint);
			expect(startedSession.dragStartTime).toBe(startedAt);
			fireEvent.pointerUp(window, { pointerId: 153, clientX: 271, clientY: 20 });
		} finally {
			if (requestAnimationFrameDescriptor) {
				Object.defineProperty(window, 'requestAnimationFrame', requestAnimationFrameDescriptor);
			} else {
				Reflect.deleteProperty(window, 'requestAnimationFrame');
			}
		}
	});

	it('snaps nonconsecutive drag compaction for reduced motion', () => {
		const matchMediaDescriptor = Object.getOwnPropertyDescriptor(window, 'matchMedia');
		Object.defineProperty(window, 'matchMedia', {
			configurable: true,
			value: (media: string): MediaQueryList => ({
				matches: media === '(prefers-reduced-motion: reduce)',
				media,
				onchange: null,
				addListener: vi.fn<MediaQueryList['addListener']>(),
				removeListener: vi.fn<MediaQueryList['removeListener']>(),
				addEventListener: vi.fn<MediaQueryList['addEventListener']>(),
				removeEventListener: vi.fn<MediaQueryList['removeEventListener']>(),
				dispatchEvent: () => true,
			}),
		});
		try {
			const model = new TabModel({
				tabs: [
					{ id: 'a', title: 'A', pinned: false, group: null },
					{ id: 'b', title: 'B', pinned: false, group: null },
					{ id: 'c', title: 'C', pinned: false, group: null },
				],
				activeTabId: 'c',
				selectedTabIds: ['a', 'c'],
			});
			render(<TabStrip model={model} />);
			const a = screen.getByRole('tab', { name: /^A/ });
			const b = screen.getByRole('tab', { name: /^B/ });
			const c = screen.getByRole('tab', { name: /^C/ });
			setRect(a, 0, 100);
			setRect(b, 100, 100);
			setRect(c, 200, 100);

			fireEvent.pointerDown(c, { button: 0, pointerId: 154, clientX: 250, clientY: 20 });
			fireEvent.pointerMove(window, { pointerId: 154, clientX: 261, clientY: 20 });
			expect(document.querySelector('[data-horizontal-drag-view][data-compacting]')).toBeNull();
			fireEvent.pointerUp(window, { pointerId: 154, clientX: 261, clientY: 20 });
		} finally {
			if (matchMediaDescriptor) {
				Object.defineProperty(window, 'matchMedia', matchMediaDescriptor);
			} else {
				Reflect.deleteProperty(window, 'matchMedia');
			}
		}
	});

	it('uses the packed mouse offset for the insertion target of a later source tab', () => {
		const model = new TabModel({
			tabs: ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'].map((id) => ({
				id,
				title: id.toUpperCase(),
				pinned: false,
				group: null,
			})),
			activeTabId: 'g',
			selectedTabIds: ['c', 'g'],
		});
		render(<TabStrip model={model} />);
		const renderedTabs = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'].map((name, index) => {
			const tab = screen.getByRole('tab', { name: new RegExp(`^${name}`) });
			setRect(tab, index * 100, 100);
			return tab;
		});

		fireEvent.pointerDown(requireDefined(renderedTabs[6]), {
			button: 0,
			pointerId: 89,
			clientX: 650,
			clientY: 20,
		});
		fireEvent.pointerMove(window, { pointerId: 89, clientX: 661, clientY: 20 });

		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'a',
			'b',
			'd',
			'e',
			'f',
			'c',
			'g',
			'h',
		]);
		fireEvent.pointerUp(window, { pointerId: 89, clientX: 661, clientY: 20 });
	});

	it('cancels a local group-header drag before a model tab is added', () => {
		const model = createModel();
		const onDraggingChange = vi.fn<() => void>();
		render(<TabStrip model={model} onDraggingChange={onDraggingChange} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		const researchA = screen.getByRole('tab', { name: /^Research A/ });
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(group, 50, 70);
		setRect(researchA, 120, 100);
		setRect(researchB, 220, 100);
		setRect(loose, 320, 100);

		fireEvent.pointerDown(group, { button: 0, pointerId: 44, clientX: 80, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 44, clientX: 500, clientY: 20 });
		expect(document.querySelector('[data-horizontal-drag-context]')).not.toBeNull();
		expect(model.getGroup('research')?.collapsed).toBe(true);

		act(() => {
			model.addTab({ title: 'Added', pinned: false, group: null }, { id: 'added' });
		});
		expect(document.querySelector('[data-horizontal-drag-context]')).toBeNull();
		expect(model.getGroup('research')?.collapsed).toBe(false);
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'research-a',
			'research-b',
			'loose',
			'added',
		]);
		expect(model.getSnapshot().activeTabId).toBe('added');
		expect(onDraggingChange).toHaveBeenLastCalledWith(false);
	});

	it('keeps a partial group drag underline local to the dragged tab', () => {
		const model = createModel();
		model.setSelection(['research-a'], 'pinned');
		render(<TabStrip model={model} />);
		const pinned = screen.getByRole('tab', { name: /^Pinned/ });
		const researchA = screen.getByRole('tab', { name: /^Research A/ });
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		const underline = requireDefined(
			document.querySelector<HTMLElement>('[data-group-underline-id="research"]'),
		);
		setRect(pinned, 0, 50);
		setRect(researchA, 50, 100);
		setRect(researchB, 150, 100);
		setRect(loose, 250, 100);
		setRect(underline, 28, 154, 39, 2);

		fireEvent.pointerDown(researchA, { button: 0, pointerId: 17, clientX: 100, clientY: 20 });
		fireEvent.pointerMove(window, { pointerId: 17, clientX: 111, clientY: 20 });

		const dragUnderline = document.querySelector<HTMLElement>('[data-drag-group-underline]');
		expect(dragUnderline?.style.getPropertyValue('--ob-comp-tab-strip-inline-start')).toBe('18px');
		expect(dragUnderline?.style.getPropertyValue('--ob-comp-tab-strip-inline-size')).toBe('64px');

		fireEvent.pointerMove(window, { pointerId: 17, clientX: 500, clientY: 20 });
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'research-a')?.group).toBeNull();
		expect(document.querySelector('[data-drag-group-underline]')).toBeNull();
		expect(document.querySelector('[data-horizontal-drag-context]')).not.toBeNull();

		fireEvent.pointerUp(window, { pointerId: 17, clientX: 500, clientY: 20 });
	});

	it('keeps a previously collapsed group collapsed throughout header dragging', () => {
		const model = createModel();
		model.selectTab('loose');
		model.updateGroup('research', { collapsed: true });
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*collapsed$/ });
		setRect(group, 0, 216, 50, 30);

		fireEvent.pointerDown(group, { button: 0, pointerId: 13, clientX: 100, clientY: 60 });
		fireEvent.pointerMove(window, { pointerId: 13, clientX: 100, clientY: 120 });
		expect(model.getGroup('research')?.collapsed).toBe(true);
		expect(
			document
				.querySelector<HTMLElement>('[data-tab-strip-drag-ghost]')
				?.style.getPropertyValue('--ob-comp-tab-strip-block-size'),
		).toBe('36px');
		expect(
			document
				.querySelector<HTMLElement>('[data-vertical-drag-view="group-visual"]')
				?.style.getPropertyValue('--ob-comp-tab-strip-block-size'),
		).toBe('34px');
		fireEvent.pointerUp(window, { pointerId: 13, clientX: 100, clientY: 120 });
		expect(model.getGroup('research')?.collapsed).toBe(true);
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		expect(
			screen
				.getByRole('tablist', { name: /^Research, 2 tabs.*collapsed$/ })
				.getAttribute('data-active-tab-id'),
		).toBe('research-a');
	});

	it('keeps an expanded group visible in the vertical header-drag presentation', () => {
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		const groupView = requireDefined(group.closest<HTMLElement>('[data-tab-strip-group]'));
		const researchA = screen.getByRole('tab', { name: /^Research A/ });
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		setRect(groupView, 0, 216, 50, 96);
		setRect(group, 0, 216, 50, 26);
		setRect(researchA, 12, 204, 82, 30);
		setRect(researchB, 12, 204, 114, 30);

		fireEvent.pointerDown(group, { button: 0, pointerId: 194, clientX: 100, clientY: 65 });
		fireEvent.pointerMove(window, { pointerId: 194, clientX: 100, clientY: 120 });

		expect(model.getGroup('research')?.collapsed).toBe(true);
		const draggedGroupView = requireDefined(
			document.querySelector<HTMLElement>('[data-vertical-drag-view="group-visual"]'),
		);
		expect(draggedGroupView.style.getPropertyValue('--ob-comp-tab-strip-block-size')).toBe('96px');
		const draggedGroup = requireDefined(
			draggedGroupView.querySelector<HTMLElement>('[data-tab-strip-group]'),
		);
		expect(draggedGroup.hasAttribute('data-group-collapsed')).toBe(false);
		expect(
			[...draggedGroup.querySelectorAll<HTMLElement>('[data-tab-id]')].map(
				(tab) => tab.dataset['tabId'],
			),
		).toEqual(['research-a', 'research-b']);
		expect(
			draggedGroup
				.querySelector<HTMLElement>('[data-tab-strip-group-tabs]')
				?.hasAttribute('aria-hidden'),
		).toBe(false);

		fireEvent.pointerUp(window, { pointerId: 194, clientX: 100, clientY: 120 });
		expect(model.getGroup('research')?.collapsed).toBe(false);
	});

	it('restores the common-handler group-header selection rule when a vertical drag is canceled', () => {
		const model = createModel();
		model.selectTab('loose');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		setRect(requireDefined(group.closest<HTMLElement>('[data-tab-strip-group]')), 0, 216, 50, 100);
		setRect(group, 0, 216, 54, 26);

		fireEvent.pointerDown(group, { button: 0, pointerId: 131, clientX: 100, clientY: 65 });
		fireEvent.pointerMove(window, { pointerId: 131, clientX: 100, clientY: 130 });
		fireEvent.pointerCancel(window, { pointerId: 131, clientX: 100, clientY: 130 });

		expect(model.getGroup('research')?.collapsed).toBe(false);
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
	});

	it('anchors and stacks selected vertical tabs with Chromium dragged-container geometry', () => {
		const model = createModel();
		model.setSelection(['research-a', 'research-b'], 'research-b');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const group = screen.getByRole('tablist', { name: /^Research, 2 tabs.*expanded$/ });
		const groupView = requireDefined(group.closest<HTMLElement>('[data-tab-strip-group]'));
		const researchA = screen.getByRole('tab', { name: /^Research A/ });
		const researchB = screen.getByRole('tab', { name: /^Research B/ });
		setRect(groupView, 0, 216, 64, 100);
		setRect(group, 12, 216, 68, 30);
		setRect(researchA, 12, 216, 100, 30);
		setRect(researchB, 12, 216, 132, 30);

		fireEvent.pointerDown(researchB, { button: 0, pointerId: 14, clientX: 100, clientY: 147 });
		fireEvent.pointerMove(window, { pointerId: 14, clientX: 100, clientY: 200 });

		const ghost = document.querySelector<HTMLElement>(
			'[data-tab-strip-drag-ghost][data-layout="vertical"]',
		);
		expect(ghost?.style.getPropertyValue('--ob-comp-tab-strip-inline-start')).toBe('0px');
		expect(ghost?.style.getPropertyValue('--ob-comp-tab-strip-block-start')).toBe('117px');
		const draggedGroup = ghost?.querySelector<HTMLElement>(
			'[data-vertical-drag-view="group-visual"] [data-tab-strip-group]',
		);
		expect(draggedGroup?.querySelector('[data-group-id="research"]')).not.toBeNull();
		expect(
			[...(draggedGroup?.querySelectorAll<HTMLElement>('[data-tab-id]') ?? [])].map(
				(tab) => tab.dataset['tabId'],
			),
		).toEqual(['research-a', 'research-b']);
		expect(ghost?.querySelectorAll('[data-horizontal-drag-view]')).toHaveLength(0);
		expect(
			document
				.querySelector<HTMLElement>('[data-tab-strip-group][data-group-id="research"]')
				?.hasAttribute('data-drag-source'),
		).toBe(true);
		expect(
			screen
				.getByRole('tab', { name: /^Research A/ })
				.closest('[data-tab-strip-tab-wrapper]')
				?.hasAttribute('data-drag-source'),
		).toBe(false);
		expect(
			screen
				.getByRole('tab', { name: /^Research B/ })
				.closest('[data-tab-strip-tab-wrapper]')
				?.hasAttribute('data-drag-source'),
		).toBe(false);

		fireEvent.pointerUp(window, { pointerId: 14, clientX: 100, clientY: 200 });
	});

	it('drags a normal vertical tab as the real tab view instead of a generic card', () => {
		const model = createModel();
		model.selectTab('loose');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		setRect(loose, 12, 216, 140, 30);

		fireEvent.pointerDown(loose, { button: 0, pointerId: 151, clientX: 90, clientY: 150 });
		fireEvent.pointerMove(window, { pointerId: 151, clientX: 90, clientY: 210 });

		const draggedTab = document.querySelector<HTMLElement>(
			'[data-vertical-drag-view="tab"] [data-tab-id="loose"]',
		);
		expect(draggedTab).not.toBeNull();
		expect(draggedTab?.querySelector('[data-tab-icon]')).not.toBeNull();
		expect(
			document
				.querySelector<HTMLElement>('[data-tab-strip-scroll-content] [data-tab-id="loose"]')
				?.closest('[data-tab-strip-tab-wrapper]')
				?.hasAttribute('data-drag-source'),
		).toBe(true);
		fireEvent.pointerUp(window, { pointerId: 151, clientX: 90, clientY: 210 });
	});

	it('keeps displaced vertical units mounted during an upward live reorder', async () => {
		const model = new TabModel({
			tabs: [
				{ id: 'a', title: 'A', pinned: false, group: null },
				{ id: 'b', title: 'B', pinned: false, group: null },
				{ id: 'c', title: 'C', pinned: false, group: null },
			],
			activeTabId: 'c',
		});
		const moveTabsByDrag = vi.spyOn(model, 'moveTabsByDrag');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const strip = screen.getByRole('tablist', { name: 'Tabs' });
		const a = screen.getByRole('tab', { name: /^A/ });
		const b = screen.getByRole('tab', { name: /^B/ });
		const c = screen.getByRole('tab', { name: /^C/ });
		const bUnit = requireDefined(
			document.querySelector<HTMLElement>('[data-vertical-unpinned-unit="b"]'),
		);
		const cUnit = requireDefined(
			document.querySelector<HTMLElement>('[data-vertical-unpinned-unit="c"]'),
		);
		const scrollContent = requireDefined(
			document.querySelector<HTMLElement>('[data-tab-strip-scroll-content]'),
		);
		const removedUnitIds: string[] = [];
		const observer = new MutationObserver((records) => {
			for (const record of records) {
				for (const node of record.removedNodes) {
					if (!(node instanceof HTMLElement)) continue;
					const id = node.getAttribute('data-vertical-unpinned-unit');
					if (id !== null) removedUnitIds.push(id);
				}
			}
		});
		observer.observe(scrollContent, { childList: true });
		setRect(strip, 0, 240, 0, 500);
		setRect(a, 12, 216, 100, 30);
		setRect(b, 12, 216, 132, 30);
		setRect(c, 12, 216, 164, 30);
		expect(bUnit.style.getPropertyValue('--ob-comp-tab-strip-block-start')).toBe('32px');

		fireEvent.pointerDown(c, { button: 0, pointerId: 192, clientX: 90, clientY: 179 });
		fireEvent.pointerMove(window, { pointerId: 192, clientX: 90, clientY: 100 });
		expect(moveTabsByDrag).toHaveBeenCalledOnce();
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['c', 'a', 'b']);
		expect(bUnit.style.getPropertyValue('--ob-comp-tab-strip-block-start')).toBe('64px');
		await Promise.resolve();
		expect(removedUnitIds).toEqual(['c']);
		const liveCUnit = requireDefined(
			document.querySelector<HTMLElement>('[data-vertical-unpinned-unit="c"]'),
		);
		expect(liveCUnit).not.toBe(cUnit);
		expect(liveCUnit.hasAttribute('data-drag-source-unit')).toBe(true);
		observer.disconnect();

		fireEvent.pointerUp(window, { pointerId: 192, clientX: 90, clientY: 100 });
		expect(moveTabsByDrag).toHaveBeenCalledOnce();
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['c', 'a', 'b']);
		expect(
			document
				.querySelector<HTMLElement>('[data-vertical-unpinned-unit="c"]')
				?.hasAttribute('data-drag-source-unit'),
		).toBe(false);
	});

	it('animates non-source vertical views from their source-relative positions', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'a', title: 'A', pinned: false, group: null },
				{ id: 'b', title: 'B', pinned: false, group: null },
				{ id: 'c', title: 'C', pinned: false, group: null },
			],
			activeTabId: 'c',
		});
		model.setSelection(['a', 'c'], 'c');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const a = screen.getByRole('tab', { name: /^A/ });
		const b = screen.getByRole('tab', { name: /^B/ });
		const c = screen.getByRole('tab', { name: /^C/ });
		setRect(a, 12, 216, 100, 30);
		setRect(b, 12, 216, 132, 30);
		setRect(c, 12, 216, 164, 30);

		fireEvent.pointerDown(c, { button: 0, pointerId: 152, clientX: 90, clientY: 179 });
		fireEvent.pointerMove(window, { pointerId: 152, clientX: 90, clientY: 220 });

		const draggedA = document.querySelector<HTMLElement>(
			'[data-vertical-drag-view="tab"]:has([data-tab-id="a"])',
		);
		const draggedC = document.querySelector<HTMLElement>(
			'[data-vertical-drag-view="tab"]:has([data-tab-id="c"])',
		);
		expect(draggedA?.hasAttribute('data-compacting')).toBe(true);
		expect(draggedA?.style.getPropertyValue('--ob-comp-tab-strip-block-start')).toBe('-32px');
		expect(draggedC?.hasAttribute('data-source')).toBe(true);
		expect(draggedC?.hasAttribute('data-compacting')).toBe(false);
		fireEvent.pointerUp(window, { pointerId: 152, clientX: 90, clientY: 220 });
	});

	it('keeps squashed vertical peers visible until drag-start compaction completes', () => {
		const requestAnimationFrameDescriptor = Object.getOwnPropertyDescriptor(
			window,
			'requestAnimationFrame',
		);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		Object.defineProperty(window, 'requestAnimationFrame', {
			configurable: true,
			value: requestAnimationFrame,
		});
		try {
			const model = new TabModel({
				tabs: [
					{ id: 'a', title: 'A', pinned: false, group: null },
					{ id: 'b', title: 'B', pinned: false, group: null },
					{ id: 'c', title: 'C', pinned: false, group: null },
				],
				activeTabId: 'c',
				selectedTabIds: ['a', 'c'],
			});
			let session: DragSessionData<Readonly<TabGroupData>> | null = null;
			render(
				<TabStrip
					model={model}
					orientation={TabStripOrientation.Vertical}
					verticalCollapsed
					createCrossWindowDragRuntime={(candidate) => {
						session = candidate;
						return null;
					}}
				/>,
			);
			const a = screen.getByRole('tab', { name: /^A/ });
			const b = screen.getByRole('tab', { name: /^B/ });
			const c = screen.getByRole('tab', { name: /^C/ });
			setRect(a, 12, 40, 100, 30);
			setRect(b, 12, 40, 132, 30);
			setRect(c, 12, 40, 164, 30);

			fireEvent.pointerDown(c, { button: 0, pointerId: 155, clientX: 30, clientY: 179 });
			fireEvent.pointerMove(window, { pointerId: 155, clientX: 30, clientY: 220 });
			expect(document.querySelectorAll('[data-vertical-drag-view="tab"]')).toHaveLength(2);

			const animationFrames = requestAnimationFrame.mock.calls.flatMap((call) =>
				call[0] === undefined ? [] : [call[0]],
			);
			act(() =>
				animationFrames.forEach((animationFrame) =>
					animationFrame(requireDefined(session).dragStartTime + DRAG_START_ANIMATION_DURATION_MS),
				),
			);
			expect(document.querySelectorAll('[data-vertical-drag-view="tab"]')).toHaveLength(1);
			fireEvent.pointerUp(window, { pointerId: 155, clientX: 30, clientY: 220 });
		} finally {
			if (requestAnimationFrameDescriptor) {
				Object.defineProperty(window, 'requestAnimationFrame', requestAnimationFrameDescriptor);
			} else {
				Reflect.deleteProperty(window, 'requestAnimationFrame');
			}
		}
	});

	it('preserves collection order when an ungrouped tab precedes a complete vertical group', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'loose', title: 'Loose', pinned: false, group: null },
				{ id: 'ga', title: 'Group A', pinned: false, group: 'g' },
				{ id: 'gb', title: 'Group B', pinned: false, group: 'g' },
			],
			groups: [{ id: 'g', title: 'Group', color: 'blue', collapsed: false }],
			activeTabId: 'gb',
		});
		model.setSelection(['loose', 'ga', 'gb'], 'gb');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const loose = screen.getByRole('tab', { name: /^Loose/ });
		const ga = screen.getByRole('tab', { name: /^Group A/ });
		const gb = screen.getByRole('tab', { name: /^Group B/ });
		const groupView = requireDefined(
			screen
				.getByRole('tablist', { name: /^Group, 2 tabs/ })
				.closest<HTMLElement>('[data-tab-strip-group]'),
		);
		setRect(loose, 12, 216, 100, 30);
		setRect(groupView, 0, 216, 132, 100);
		setRect(ga, 12, 216, 168, 30);
		setRect(gb, 12, 216, 200, 30);

		fireEvent.pointerDown(gb, { button: 0, pointerId: 153, clientX: 90, clientY: 215 });
		fireEvent.pointerMove(window, { pointerId: 153, clientX: 90, clientY: 260 });

		const ghost = requireDefined(
			document.querySelector<HTMLElement>('[data-tab-strip-drag-ghost]'),
		);
		const views = [...ghost.children].filter(
			(child): child is HTMLElement =>
				child instanceof HTMLElement && child.hasAttribute('data-vertical-drag-view'),
		);
		expect(
			views.map((view) => {
				if (view.querySelector('[data-tab-id="loose"]')) return 'loose';
				if (view.querySelector('[data-group-id="g"]')) return 'group:g';
				return 'unknown';
			}),
		).toEqual(['loose', 'group:g']);
		fireEvent.pointerUp(window, { pointerId: 153, clientX: 90, clientY: 260 });
	});

	it('uses a squashed two-axis drag target in the expanded vertical pinned grid', () => {
		const model = new TabModel({
			tabs: [
				{ id: 'p1', title: 'P1', pinned: true, group: null },
				{ id: 'p2', title: 'P2', pinned: true, group: null },
				{ id: 'p3', title: 'P3', pinned: true, group: null },
				{ id: 'p4', title: 'P4', pinned: true, group: null },
				{ id: 'loose', title: 'Loose', pinned: false, group: null },
			],
			activeTabId: 'p4',
		});
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const strip = screen.getByRole('tablist', { name: 'Tabs' });
		setRect(strip, 0, 240, 0, 500);
		const positions = [
			[12, 10],
			[116, 10],
			[12, 46],
			[116, 46],
		] as const;
		['P1', 'P2', 'P3', 'P4'].forEach((name, index) => {
			setRect(
				screen.getByRole('tab', { name: new RegExp(`^${name}`) }),
				requireDefined(positions[index])[0],
				100,
				requireDefined(positions[index])[1],
				30,
			);
		});
		const p4 = screen.getByRole('tab', { name: /^P4/ });

		fireEvent.pointerDown(p4, { button: 0, pointerId: 16, clientX: 160, clientY: 61 });
		fireEvent.pointerMove(window, { pointerId: 16, clientX: 20, clientY: 20 });
		expect(document.querySelector('[data-tab-strip-drag-ghost]')?.getAttribute('data-layout')).toBe(
			'squash',
		);
		expect(
			model
				.getSnapshot()
				.tabs.slice(0, 4)
				.map((tab) => tab.id),
		).toEqual(['p4', 'p1', 'p2', 'p3']);
		fireEvent.pointerUp(window, { pointerId: 16, clientX: 20, clientY: 20 });
	});
});
