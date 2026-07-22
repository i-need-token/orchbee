import { requireDefined, TabStripOrientation } from '../../../common';
import { StrictMode } from 'react';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { DragSessionData } from '../../../dragging';
import {
	ThumbnailCaptureReadiness,
	type HoverCardThumbnailSource,
	type HoverCardThumbnailSubscription,
} from '../../../hovercard';
import { TabModel, type TabGroupData } from '../../../model';
import { TabStripComboButtonController } from '../../../shared';
import { VerticalTabStripExpandOnHoverController } from '../../../vertical';
import { TabStrip } from '../TabStrip';
class FakeThumbnailSource implements HoverCardThumbnailSource<string> {
	public callback: ((image: string) => void) | null = null;
	public requested = 0;
	public sizeHint: {
		width: number;
		height: number;
	} | null = null;
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
				this.sizeHint = {
					...size,
				};
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
			{
				id: 'pinned',
				title: 'Pinned',
				pinned: true,
				group: null,
				split: null,
			},
			{
				id: 'research-a',
				title: 'Research A',
				pinned: false,
				group: 'research',
				split: null,
			},
			{
				id: 'research-b',
				title: 'Research B',
				pinned: false,
				group: 'research',
				split: null,
			},
			{
				id: 'loose',
				title: 'Loose',
				pinned: false,
				group: null,
				split: null,
			},
		],
		groups: [
			{
				id: 'research',
				title: 'Research',
				color: 'blue',
				collapsed: false,
			},
		],
		activeTabId: 'research-a',
	});
}
function setRect(element: Element, x: number, width: number, y = 0, height = 41): void {
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
function urlDataTransfer(url: string): DataTransfer {
	return {
		dropEffect: 'none',
		effectAllowed: 'all',
		files: [] as unknown as FileList,
		items: [] as unknown as DataTransferItemList,
		types: ['text/uri-list'],
		clearData: vi.fn<() => void>(),
		getData: vi.fn<(type: string) => string>((type: string) =>
			type === 'text/uri-list' ? url : '',
		),
		setData: vi.fn<() => void>(),
		setDragImage: vi.fn<() => void>(),
	};
}
function fireUrlDrag(
	type: 'dragover' | 'drop',
	target: Element,
	dataTransfer: DataTransfer,
	clientX: number,
	clientY: number,
): void {
	const event = new Event(type, {
		bubbles: true,
		cancelable: true,
	});
	Object.defineProperties(event, {
		clientX: {
			value: clientX,
		},
		clientY: {
			value: clientY,
		},
		dataTransfer: {
			value: dataTransfer,
		},
	});
	fireEvent(target, event);
}
afterEach(() => {
	cleanup();
	vi.useRealTimers();
});
describe('TabStrip', () => {
	const getHoverCard = (): HTMLElement => {
		const card = document.querySelector<HTMLElement>('.ct-hover-card');
		if (!card) throw new Error('Expected Chromium hover card to be mounted.');
		return card;
	};
	it('selects with the pointer but leaves unmodified focus-navigation keys to Views accessibility', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 1,
			clientX: 100,
			clientY: 10,
		});
		fireEvent.pointerUp(window, {
			button: 0,
			pointerId: 1,
			clientX: 100,
			clientY: 10,
		});
		expect(model.getSnapshot().activeTabId).toBe('loose');
		loose.focus();
		fireEvent.keyDown(loose, {
			key: 'ArrowLeft',
		});
		fireEvent.keyDown(loose, {
			key: 'Home',
		});
		expect(model.getSnapshot().activeTabId).toBe('loose');
		expect(document.activeElement).toBe(loose);
		expect(loose.getAttribute('tabindex')).toBe('-1');
	});
	it('selects touch tabs on gesture-down and only permits dragging after long press', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
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
		expect(document.querySelector('.ct-drag-ghost')).toBeNull();
		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 63,
			pointerType: 'touch',
			clientX: 80,
			clientY: 125,
		});
		act(() => vi.advanceTimersByTime(500));
		expect(model.getSnapshot().activeTabId).toBe('loose');
		fireEvent.pointerMove(window, {
			pointerId: 63,
			pointerType: 'touch',
			clientX: 80,
			clientY: 145,
		});
		expect(document.querySelector('.ct-drag-ghost')).not.toBeNull();
		fireEvent.pointerCancel(window, {
			pointerId: 63,
			pointerType: 'touch',
		});
	});
	it('opens source-matching editors for a touch long tap without collapsing a group', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
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
		const group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
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
		expect(
			screen.getByRole('textbox', {
				name: 'Group name',
			}),
		).toBeTruthy();
		expect(model.getGroup('research')?.collapsed).toBe(false);
	});
	it('does not begin dragging a tab that a selection-modifier press deselected', () => {
		const model = createModel();
		model.setSelection(['research-a', 'research-b'], 'research-a');
		render(<TabStrip model={model} />);
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
		setRect(researchB, 150, 100);
		fireEvent.pointerDown(researchB, {
			button: 0,
			pointerId: 71,
			clientX: 200,
			clientY: 20,
			ctrlKey: true,
		});
		fireEvent.pointerMove(window, {
			pointerId: 71,
			clientX: 260,
			clientY: 20,
		});
		expect(model.getSnapshot().selectedTabIds.has('research-b')).toBe(false);
		expect(document.querySelector('.ct-drag-ghost')).toBeNull();
	});
	it('ports middle-release containment and the rapid closing-tab fallback', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} />);
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
		const descriptor = Object.getOwnPropertyDescriptor(document, 'elementFromPoint');
		const elementFromPoint = vi.fn<() => Element | null>((): Element | null => researchB);
		Object.defineProperty(document, 'elementFromPoint', {
			configurable: true,
			value: elementFromPoint,
		});
		try {
			fireEvent.pointerDown(researchA, {
				button: 1,
				pointerId: 81,
				clientX: 100,
				clientY: 20,
			});
			fireEvent.pointerUp(window, {
				button: 1,
				pointerId: 81,
				clientX: 200,
				clientY: 20,
			});
			expect(researchA.closest('.ct-horizontal-item')?.hasAttribute('data-closing')).toBe(false);
			expect(researchB.closest('.ct-horizontal-item')?.hasAttribute('data-closing')).toBe(false);
			fireEvent(
				researchA,
				new MouseEvent('auxclick', {
					button: 1,
					bubbles: true,
				}),
			);
			expect(researchA.closest('.ct-horizontal-item')?.hasAttribute('data-closing')).toBe(true);
			fireEvent.pointerDown(researchA, {
				button: 1,
				pointerId: 82,
				clientX: 100,
				clientY: 20,
			});
			fireEvent.pointerUp(window, {
				button: 1,
				pointerId: 82,
				clientX: 200,
				clientY: 20,
			});
			expect(researchB.closest('.ct-horizontal-item')?.hasAttribute('data-closing')).toBe(true);
		} finally {
			if (descriptor) Object.defineProperty(document, 'elementFromPoint', descriptor);
			else Reflect.deleteProperty(document, 'elementFromPoint');
		}
	});
	it('preserves a range selection when Shift was held on pointer down', () => {
		const model = createModel();
		model.setSelection(['research-a', 'research-b'], 'research-a');
		render(<TabStrip model={model} />);
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
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
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		fireEvent.keyDown(loose, {
			key: ' ',
		});
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		fireEvent.keyUp(loose, {
			key: ' ',
		});
		expect(model.getSnapshot().activeTabId).toBe('loose');
	});
	it('announces Chromium keyboard reorders and group-boundary changes', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		fireEvent.keyDown(loose, {
			key: 'ArrowLeft',
			ctrlKey: true,
		});
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.group).toBe('research');
		expect(screen.getByRole('status').textContent).toContain('Tab added to group Research');
		fireEvent.keyDown(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			{
				key: 'ArrowRight',
				ctrlKey: true,
			},
		);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.group).toBeNull();
		expect(screen.getByRole('status').textContent).toContain('Tab removed from group Research');
	});
	it('collapses a group while projecting its active child through the group header', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		fireEvent.click(
			screen.getByRole('tablist', {
				name: /^Research, 2 tabs.*expanded$/,
			}),
		);
		expect(model.getGroup('research')?.collapsed).toBe(true);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		const collapsedHeader = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*collapsed$/,
		});
		expect(collapsedHeader.getAttribute('data-active')).toBe('true');
		expect(collapsedHeader.getAttribute('data-active-tab-id')).toBe('research-a');
		expect(collapsedHeader.querySelector('.ct-group-header__active-background')).toBeTruthy();
		expect(
			collapsedHeader.querySelector('.ct-group-header__active-background-fill')?.getAttribute('d'),
		).toBeTruthy();
		expect(document.activeElement).toBe(collapsedHeader);
		fireEvent.click(collapsedHeader);
		const expandedHeader = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		expect(expandedHeader.querySelector('.ct-group-header__active-background')).toBeNull();
		expect(document.activeElement).toBe(expandedHeader);
	});
	it('keeps collapsed tabs and their underline visible until the bounds animation finishes', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} />);
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		const wrapper = requireDefined(researchA.closest<HTMLElement>('.ct-horizontal-item'));
		fireEvent.click(
			screen.getByRole('tablist', {
				name: /^Research, 2 tabs.*expanded$/,
			}),
		);
		expect(wrapper.hasAttribute('data-group-collapsed-hidden')).toBe(false);
		expect(document.querySelector('[data-group-underline-id="research"]')).toBeTruthy();
		act(() => vi.advanceTimersByTime(199));
		expect(wrapper.hasAttribute('data-group-collapsed-hidden')).toBe(false);
		act(() => vi.advanceTimersByTime(1));
		expect(wrapper.hasAttribute('data-group-collapsed-hidden')).toBe(true);
		expect(document.querySelector('[data-group-underline-id="research"]')).toBeNull();
	});
	it('freezes horizontal width only for pointer-originated group collapse', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		let group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		fireEvent.pointerDown(group, {
			button: 0,
			pointerId: 91,
			pointerType: 'mouse',
		});
		fireEvent.pointerUp(window, {
			button: 0,
			pointerId: 91,
			pointerType: 'mouse',
		});
		fireEvent.click(group, {
			detail: 1,
		});
		expect(
			screen
				.getByRole('tablist', {
					name: 'Browser tabs',
				})
				.getAttribute('data-tab-closing-mode'),
		).toBe('mouse');
		group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*collapsed$/,
		});
		fireEvent.click(group, {
			detail: 1,
		});
		expect(
			screen
				.getByRole('tablist', {
					name: 'Browser tabs',
				})
				.hasAttribute('data-tab-closing-mode'),
		).toBe(false);
		group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		fireEvent.click(group, {
			detail: 0,
		});
		expect(
			screen
				.getByRole('tablist', {
					name: 'Browser tabs',
				})
				.hasAttribute('data-tab-closing-mode'),
		).toBe(false);
	});
	it('closes an open group editor without toggling or dragging its header', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		fireEvent.contextMenu(group, {
			clientX: 80,
			clientY: 20,
		});
		expect(screen.getByRole('menu')).toBeTruthy();
		fireEvent.pointerDown(group, {
			button: 0,
			pointerId: 73,
			clientX: 80,
			clientY: 20,
		});
		fireEvent.pointerUp(group, {
			button: 0,
			pointerId: 73,
			clientX: 80,
			clientY: 20,
		});
		fireEvent.click(group);
		expect(screen.queryByRole('menu')).toBeNull();
		expect(model.getGroup('research')?.collapsed).toBe(false);
		expect(document.querySelector('.ct-drag-ghost')).toBeNull();
	});
	it('runs tab context-menu commands through the model', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Duplicate',
			}),
		);
		expect(model.getSnapshot().tabs.filter((tab) => tab.title === 'Loose')).toHaveLength(2);
		expect(screen.queryByRole('menu')).toBeNull();
	});
	it('runs audio and group membership commands from the tab context menu', () => {
		const model = createModel();
		const onAudio = vi.fn<() => void>();
		model.updateTab('loose', {
			alertState: 'audio-playing',
			url: 'https://example.test/',
		});
		render(<TabStrip model={model} onTabAudioMuteToggle={onAudio} />);
		const openLooseMenu = () =>
			fireEvent.contextMenu(
				screen.getByRole('tab', {
					name: /^Loose/,
				}),
				{
					clientX: 50,
					clientY: 30,
				},
			);
		openLooseMenu();
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Mute site',
			}),
		);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.alertState).toBe(
			'audio-muting',
		);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.siteMuted).toBe(true);
		expect(onAudio).toHaveBeenCalledWith('loose', true);
		openLooseMenu();
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Add tab to group',
			}),
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'New group',
			}),
		);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.group).not.toBeNull();
		openLooseMenu();
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Remove from group',
			}),
		);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.group).toBeNull();
	});
	it('uses native-menu arrow navigation across the tab group submenu', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		const menu = screen.getByRole('menu');
		expect(document.activeElement).toBe(menu);
		fireEvent.keyDown(menu, {
			key: 'ArrowDown',
		});
		expect(document.activeElement).toBe(
			screen.getByRole('menuitem', {
				name: 'New tab to the right',
			}),
		);
		fireEvent.keyDown(menu, {
			key: 'ArrowDown',
		});
		fireEvent.keyDown(menu, {
			key: 'ArrowDown',
		});
		const groupTrigger = screen.getByRole('menuitem', {
			name: 'Add tab to group',
		});
		expect(document.activeElement).toBe(groupTrigger);
		fireEvent.keyDown(groupTrigger, {
			key: 'ArrowRight',
		});
		const submenu = screen.getByRole('menu', {
			name: 'Tab groups',
		});
		expect(document.activeElement).toBe(
			within(submenu).getByRole('menuitem', {
				name: 'New group',
			}),
		);
		fireEvent.keyDown(requireDefined(document.activeElement), {
			key: 'ArrowLeft',
		});
		expect(document.activeElement).toBe(groupTrigger);
		expect(
			screen.queryByRole('menu', {
				name: 'Tab groups',
			}),
		).toBeNull();
	});
	it('uses orientation-specific relative labels in the tab context menu', () => {
		const horizontalModel = createModel();
		const horizontal = render(<TabStrip model={horizontalModel} />);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		expect(
			screen.getByRole('menuitem', {
				name: 'New tab to the right',
			}),
		).toBeTruthy();
		expect(
			screen.getByRole('menuitem', {
				name: 'Close tabs to the right',
			}),
		).toBeTruthy();
		horizontal.unmount();
		const verticalModel = createModel();
		render(<TabStrip model={verticalModel} orientation={TabStripOrientation.Vertical} />);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		expect(
			screen.getByRole('menuitem', {
				name: 'New tab below',
			}),
		).toBeTruthy();
		expect(
			screen.getByRole('menuitem', {
				name: 'Close tabs below',
			}),
		).toBeTruthy();
	});
	it('ports selected-tab close commands from TabStripModel', () => {
		vi.useFakeTimers();
		const model = createModel();
		model.setSelection(['research-a', 'research-b'], 'research-a');
		render(<TabStrip model={model} />);
		const openSelectedMenu = () =>
			fireEvent.contextMenu(
				screen.getByRole('tab', {
					name: /^Research A/,
				}),
				{
					clientX: 50,
					clientY: 30,
				},
			);
		openSelectedMenu();
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Close tabs to the right',
			}),
		);
		act(() => vi.advanceTimersByTime(200));
		expect(model.getIndexOfTab('loose')).toBeNull();
		expect(model.getIndexOfTab('research-a')).not.toBeNull();
		expect(model.getIndexOfTab('research-b')).not.toBeNull();
		openSelectedMenu();
		expect(
			screen
				.getByRole('menuitem', {
					name: 'Close other tabs',
				})
				.hasAttribute('disabled'),
		).toBe(true);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Close',
			}),
		);
		act(() => vi.advanceTimersByTime(200));
		expect(model.getIndexOfTab('research-a')).toBeNull();
		expect(model.getIndexOfTab('research-b')).toBeNull();
		expect(model.getIndexOfTab('pinned')).not.toBeNull();
	});
	it('treats an unselected split as one context-menu command and exclusion unit', () => {
		vi.useFakeTimers();
		const model = createModel();
		model.updateTab('research-a', {
			split: 'research-split',
		});
		model.updateTab('research-b', {
			split: 'research-split',
		});
		model.selectTab('loose');
		render(<TabStrip model={model} />);
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		fireEvent.contextMenu(researchA, {
			clientX: 50,
			clientY: 30,
		});
		expect(
			screen
				.getByRole('menuitem', {
					name: 'Close tabs to the right',
				})
				.hasAttribute('disabled'),
		).toBe(false);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Close tabs to the right',
			}),
		);
		act(() => vi.advanceTimersByTime(200));
		expect(model.getIndexOfTab('research-b')).not.toBeNull();
		expect(model.getIndexOfTab('loose')).toBeNull();
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Research A/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Close',
			}),
		);
		act(() => vi.advanceTimersByTime(200));
		expect(model.getIndexOfTab('research-a')).toBeNull();
		expect(model.getIndexOfTab('research-b')).toBeNull();
	});
	it('creates context-menu splits with the active tab as the pivot', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'New split view with current tab',
			}),
		);
		const snapshot = model.getSnapshot();
		const active = snapshot.tabs.find((tab) => tab.id === 'research-a');
		const loose = snapshot.tabs.find((tab) => tab.id === 'loose');
		expect(active?.split).not.toBeNull();
		expect(loose?.split).toBe(active?.split);
		expect(loose?.group).toBe(active?.group);
		expect(model.getIndexOfTab('loose')).toBe((model.getIndexOfTab('research-a') ?? -1) + 1);
		expect([...snapshot.selectedTabIds]).toEqual(['research-a', 'loose']);
	});
	it('creates a new-tab split when the context target is the active tab', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Research A/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Add tab to new split view',
			}),
		);
		const snapshot = model.getSnapshot();
		const split = snapshot.tabs.find((tab) => tab.id === 'research-a')?.split;
		expect(split).not.toBeNull();
		expect(snapshot.tabs.filter((tab) => tab.split === split)).toHaveLength(2);
		expect(snapshot.activeTabId).toBe('research-a');
	});
	it('runs the source-ordered arrange-split submenu with keyboard traversal', () => {
		const model = createModel();
		model.updateTab('research-a', {
			split: 'research-split',
		});
		model.updateTab('research-b', {
			split: 'research-split',
		});
		render(<TabStrip model={model} />);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Research A/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		const trigger = screen.getByRole('menuitem', {
			name: 'Arrange split view',
		});
		fireEvent.mouseEnter(trigger);
		fireEvent.click(trigger);
		expect(
			screen.getByRole('menu', {
				name: 'Arrange split view',
			}),
		).toBeTruthy();
		fireEvent.keyDown(
			within(
				screen.getByRole('menu', {
					name: 'Arrange split view',
				}),
			).getByRole('menuitem', {
				name: 'Reverse views',
			}),
			{
				key: 'ArrowLeft',
			},
		);
		trigger.focus();
		fireEvent.keyDown(trigger, {
			key: 'ArrowRight',
		});
		const submenu = screen.getByRole('menu', {
			name: 'Arrange split view',
		});
		expect(document.activeElement).toBe(
			within(submenu).getByRole('menuitem', {
				name: 'Reverse views',
			}),
		);
		expect(
			within(submenu)
				.getAllByRole('menuitem')
				.map((item) => item.textContent),
		).toEqual(['Reverse views', 'Close left view', 'Close right view', 'Separate views']);
		fireEvent.click(
			within(submenu).getByRole('menuitem', {
				name: 'Reverse views',
			}),
		);
		expect(
			model
				.getSnapshot()
				.tabs.filter((tab) => tab.split === 'research-split')
				.map((tab) => tab.id),
		).toEqual(['research-b', 'research-a']);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Research A/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Arrange split view',
			}),
		);
		fireEvent.click(
			within(
				screen.getByRole('menu', {
					name: 'Arrange split view',
				}),
			).getByRole('menuitem', {
				name: 'Separate views',
			}),
		);
		expect(
			model
				.getSnapshot()
				.tabs.filter((tab) => tab.id.startsWith('research-'))
				.every((tab) => tab.split === null),
		).toBe(true);
	});
	it('swaps an ordinary context tab into either active split slot', () => {
		const model = createModel();
		model.updateTab('research-a', {
			split: 'research-split',
		});
		model.updateTab('research-b', {
			split: 'research-split',
		});
		model.setSelection(['research-a', 'research-b'], 'research-a');
		render(<TabStrip model={model} />);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Swap with active split',
			}),
		);
		const submenu = screen.getByRole('menu', {
			name: 'Swap with active split',
		});
		fireEvent.click(
			within(submenu).getByRole('menuitem', {
				name: 'Swap right view',
			}),
		);
		const snapshot = model.getSnapshot();
		expect(
			snapshot.tabs.filter((tab) => tab.split === 'research-split').map((tab) => tab.id),
		).toEqual(['research-a', 'loose']);
		expect(snapshot.tabs.find((tab) => tab.id === 'research-b')?.split).toBeNull();
		expect(snapshot.activeTabId).toBe('research-a');
	});
	it('uses native command labels, roles, and multi-tab pin/mute direction', () => {
		const model = createModel();
		const onAudio = vi.fn<() => void>();
		model.updateTab('pinned', {
			url: 'https://pinned.test/',
			siteMuted: true,
		});
		model.updateTab('research-a', {
			url: 'https://a.test/',
			siteMuted: true,
		});
		model.updateTab('research-b', {
			url: 'https://b.test/',
			siteMuted: false,
		});
		model.setSelection(['research-a', 'research-b'], 'research-a');
		render(
			<TabStrip model={model} onReloadTabs={vi.fn<() => void>()} onTabAudioMuteToggle={onAudio} />,
		);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Research A/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		expect(
			screen.getByRole('menuitem', {
				name: 'Reload',
			}),
		).toBeTruthy();
		expect(
			screen.getByRole('menuitem', {
				name: 'Duplicate',
			}),
		).toBeTruthy();
		expect(
			screen.queryByRole('menuitem', {
				name: 'Reload tabs',
			}),
		).toBeNull();
		expect(
			screen.queryByRole('menuitemcheckbox', {
				name: 'Pin',
			}),
		).toBeNull();
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Mute sites',
			}),
		);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'research-a')?.siteMuted).toBe(true);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'research-b')?.siteMuted).toBe(true);
		expect(onAudio).toHaveBeenCalledWith('research-a', true);
		expect(onAudio).toHaveBeenCalledWith('research-b', true);
		model.setSelection(['pinned', 'research-a'], 'research-a');
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Research A/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Pin',
			}),
		);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'pinned')?.pinned).toBe(true);
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'research-a')?.pinned).toBe(true);
	});
	it('unpins pinned tabs selected for a new group like MoveTabsAndSetPropertiesImpl', () => {
		const model = createModel();
		model.setSelection(['pinned', 'loose'], 'loose');
		render(<TabStrip model={model} />);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Add tabs to group',
			}),
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'New group',
			}),
		);
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
		const pinned = container.querySelector<HTMLElement>('.ct-vertical-pinned-scroll-shell');
		const unpinned = container.querySelector<HTMLElement>('.ct-vertical-scroll-shell');
		if (!pinned || !unpinned) throw new Error('Expected both vertical tab regions.');
		expect(pinned.style.left).toBe('12px');
		expect(pinned.style.top).toBe('0px');
		expect(Number.parseFloat(unpinned.style.top)).toBeGreaterThan(
			Number.parseFloat(pinned.style.top),
		);
		expect(unpinned.getAttribute('style')).not.toContain('x:');
		expect(unpinned.getAttribute('style')).not.toContain('y:');
	});
	it('does not mistake wider grouped split tabs for expand-on-hover while collapsed', () => {
		const model = createModel();
		model.updateTab('research-a', {
			split: 'research-split',
		});
		model.updateTab('research-b', {
			split: 'research-split',
		});
		const { container } = render(
			<TabStrip
				model={model}
				orientation={TabStripOrientation.Vertical}
				verticalCollapsed
				verticalExpandOnHover={false}
			/>,
		);
		for (const tabId of ['research-a', 'research-b']) {
			const tab = container.querySelector<HTMLElement>(`[data-tab-id="${tabId}"]`);
			expect(tab?.querySelector('.ct-tab__title')).toBeNull();
		}
	});
	it('keeps keyboard-only movement out of the native tab menu and disables unavailable host commands', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Pinned/,
			}),
			{
				clientX: 20,
				clientY: 20,
			},
		);
		expect(
			screen.queryByRole('menuitem', {
				name: 'Move tab to first',
			}),
		).toBeNull();
		expect(
			screen.queryByRole('menuitem', {
				name: 'Move tab to last',
			}),
		).toBeNull();
		expect(
			screen
				.getByRole('menuitem', {
					name: 'Move tab to new window',
				})
				.hasAttribute('disabled'),
		).toBe(true);
		expect(
			screen
				.getByRole('menuitem', {
					name: 'Reload',
				})
				.hasAttribute('disabled'),
		).toBe(true);
		fireEvent.pointerDown(document.body);
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			{
				clientX: 20,
				clientY: 20,
			},
		);
		expect(
			screen
				.getByRole('menuitem', {
					name: 'Close tabs to the right',
				})
				.hasAttribute('disabled'),
		).toBe(true);
		expect(
			screen
				.getByRole('menuitem', {
					name: 'Close other tabs',
				})
				.hasAttribute('disabled'),
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
		const strip = screen.getByRole('tablist', {
			name: 'Browser tabs',
		});
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 300, 64);
		fireEvent.mouseEnter(loose);
		expect(strip.hasAttribute('data-hover-card-waiting')).toBe(true);
		expect(document.querySelector('.ct-hover-card')).toBeNull();
		act(() => vi.advanceTimersByTime(300));
		expect(getHoverCard().textContent).toContain('125 MB');
		expect(getHoverCard().querySelector('img')).toBeTruthy();
		fireEvent.mouseLeave(loose);
		expect(document.querySelector('.ct-hover-card')).toBeNull();
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
		setRect(researchB, 180, 64);
		fireEvent.mouseEnter(researchB);
		expect(getHoverCard().textContent).toContain('Research B');
		expect(strip.hasAttribute('data-hover-card-waiting')).toBe(false);
	});
	it('honors the hover-card memory preference while retaining high-memory alerts', () => {
		const model = createModel();
		model.updateTab('loose', {
			memoryUsageBytes: 1_000,
			memoryUsageHigh: false,
		});
		render(<TabStrip model={model} showHoverCardMemoryUsage={false} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 300, 100);
		fireEvent.focus(loose);
		expect(getHoverCard().textContent).not.toContain('Memory usage');
		act(() =>
			model.updateTab('loose', {
				memoryUsageHigh: true,
			}),
		);
		expect(getHoverCard().textContent).toContain('High memory usage: 1000 bytes');
	});
	it('requests asynchronous browser thumbnails and renders observer updates', () => {
		vi.useFakeTimers();
		const model = createModel();
		const source = new FakeThumbnailSource();
		render(<TabStrip model={model} getHoverCardThumbnailSource={() => source} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 300, 64);
		fireEvent.mouseEnter(loose);
		act(() => vi.advanceTimersByTime(300));
		expect(source.requested).toBe(1);
		expect(source.sizeHint).toEqual({
			width: 320,
			height: 180,
		});
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
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
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
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 300, 100);
		fireEvent.focus(loose);
		expect(getHoverCard().textContent).toContain('Loose');
		fireEvent.keyDown(window, {
			key: 'ArrowDown',
		});
		expect(document.querySelector('.ct-hover-card')).toBeNull();
	});
	it('keeps the hover card visible for keys handled by a focused tab', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 300, 100);
		fireEvent.focus(loose);
		expect(getHoverCard().textContent).toContain('Loose');
		fireEvent.keyDown(loose, {
			key: ' ',
		});
		expect(getHoverCard().textContent).toContain('Loose');
	});
	it('forwards tab keyboard focus entry and strip exit to the host controller', () => {
		const model = createModel();
		const onTabKeyboardFocusChanged = vi.fn<() => void>();
		render(<TabStrip model={model} onTabKeyboardFocusChanged={onTabKeyboardFocusChanged} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		fireEvent.focus(loose);
		expect(onTabKeyboardFocusChanged).toHaveBeenLastCalledWith('loose');
		fireEvent.blur(loose, {
			relatedTarget: document.body,
		});
		expect(onTabKeyboardFocusChanged).toHaveBeenLastCalledWith(null);
	});
	it('shows a group hover card with its first five member titles', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
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
		const group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		setRect(group, 60, 64);
		expect(group.getAttribute('title')).toBe('Research - Research A and 1 other tab');
		fireEvent.mouseEnter(group);
		act(() => vi.advanceTimersByTime(500));
		expect(document.querySelector('.ct-hover-card')).toBeNull();
		fireEvent.focus(group);
		expect(document.querySelector('.ct-hover-card')).toBeNull();
	});
	it('uses singular overflow text for a six-tab group hover card', () => {
		vi.useFakeTimers();
		const model = createModel();
		for (let index = 0; index < 4; index += 1) {
			model.addTab({
				title: `Extra ${index + 1}`,
				pinned: false,
				group: 'research',
				split: null,
			});
		}
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', {
			name: /^Research, 6 tabs/,
		});
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
		const group = screen.getByRole('tablist', {
			name: /^Research, 1 tab/,
		});
		setRect(group, 60, 64);
		fireEvent.mouseEnter(group);
		act(() => vi.advanceTimersByTime(300));
		expect(getHoverCard().textContent).toContain('Research · 1 tab');
		expect(getHoverCard().textContent).not.toContain('1 tabs');
	});
	it('does not mount an empty hover-card domain label when URL display is disabled', () => {
		const model = createModel();
		model.updateTab('loose', {
			shouldDisplayUrl: false,
		});
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 300, 100);
		fireEvent.focus(loose);
		expect(getHoverCard().querySelector('[data-hover-card-domain]')).toBeNull();
	});
	it('closes an unpinned tab with the middle mouse button', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} />);
		fireEvent(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			new MouseEvent('auxclick', {
				button: 1,
				bubbles: true,
			}),
		);
		expect(model.getIndexOfTab('loose')).not.toBeNull();
		expect(
			screen
				.getByRole('tablist', {
					name: 'Browser tabs',
				})
				.getAttribute('data-tab-closing-mode'),
		).toBe('mouse');
		expect(
			screen
				.getByRole('tab', {
					name: /^Loose/,
				})
				.closest('.ct-horizontal-item')
				?.hasAttribute('data-closing'),
		).toBe(true);
		act(() => vi.advanceTimersByTime(200));
		expect(model.getIndexOfTab('loose')).toBeNull();
		expect(screen.getByRole('status').textContent).toBe('Closed Loose');
	});
	it('keeps tabs selectable but hides close controls and blocks drag/close while OnTask-locked', () => {
		vi.useFakeTimers();
		const model = createModel();
		const { rerender } = render(<TabStrip model={model} lockedForOnTask />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 250, 100);
		expect(
			within(loose).queryByRole('button', {
				name: 'Close',
			}),
		).toBeNull();
		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 32,
			clientX: 300,
			clientY: 20,
		});
		expect(model.getSnapshot().activeTabId).toBe('loose');
		model.selectTab('research-a');
		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 33,
			pointerType: 'touch',
			clientX: 300,
			clientY: 20,
		});
		expect(model.getSnapshot().activeTabId).toBe('loose');
		fireEvent.pointerMove(window, {
			pointerId: 32,
			clientX: 50,
			clientY: 20,
		});
		expect(document.querySelector('.ct-drag-ghost')).toBeNull();
		fireEvent.contextMenu(loose, {
			clientX: 20,
			clientY: 20,
		});
		expect(
			screen
				.getByRole('menuitem', {
					name: 'Close',
				})
				.hasAttribute('disabled'),
		).toBe(true);
		expect(
			screen
				.getByRole('menuitem', {
					name: 'Close other tabs',
				})
				.hasAttribute('disabled'),
		).toBe(true);
		fireEvent.pointerDown(document.body);
		fireEvent(
			loose,
			new MouseEvent('auxclick', {
				button: 1,
				bubbles: true,
			}),
		);
		act(() => vi.advanceTimersByTime(200));
		expect(model.getIndexOfTab('loose')).not.toBeNull();
		rerender(
			<TabStrip
				model={model}
				orientation={TabStripOrientation.Vertical}
				verticalCollapsed
				lockedForOnTask
			/>,
		);
		fireEvent.mouseEnter(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
		);
		expect(
			within(
				screen.getByRole('tab', {
					name: /^Loose/,
				}),
			).queryByRole('button', {
				name: 'Close',
			}),
		).toBeNull();
	});
	it('confirms before closing the final tab in a saved group from the tab UI', () => {
		vi.useFakeTimers();
		const model = createModel();
		model.updateGroup('research', {
			saved: true,
		});
		model.moveTabsToGroup(['research-b'], null);
		render(<TabStrip model={model} />);
		const closeResearch = () =>
			fireEvent(
				screen.getByRole('tab', {
					name: /^Research A/,
				}),
				new MouseEvent('auxclick', {
					button: 1,
					bubbles: true,
				}),
			);
		closeResearch();
		expect(screen.getByRole('alertdialog').textContent).toContain('Close saved group?');
		expect(
			screen
				.getByRole('tab', {
					name: /^Research A/,
				})
				.closest('.ct-horizontal-item')
				?.hasAttribute('data-closing'),
		).toBe(false);
		fireEvent.click(
			screen.getByRole('button', {
				name: 'Cancel',
			}),
		);
		expect(screen.queryByRole('alertdialog')).toBeNull();
		expect(model.getIndexOfTab('research-a')).not.toBeNull();
		closeResearch();
		fireEvent.click(
			screen.getByRole('button', {
				name: 'Close group',
			}),
		);
		expect(screen.queryByRole('alertdialog')).toBeNull();
		expect(
			screen
				.getByRole('tab', {
					name: /^Research A/,
				})
				.closest('.ct-horizontal-item')
				?.hasAttribute('data-closing'),
		).toBe(true);
		act(() => vi.advanceTimersByTime(200));
		expect(model.getIndexOfTab('research-a')).toBeNull();
		expect(model.getSnapshot().groups.some((group) => group.id === 'research')).toBe(false);
	});
	it('closes the final saved-group tab directly when confirmation is disabled', () => {
		vi.useFakeTimers();
		const model = createModel();
		model.updateGroup('research', {
			saved: true,
		});
		model.moveTabsToGroup(['research-b'], null);
		render(<TabStrip model={model} confirmSavedGroupDestructiveActions={false} />);
		fireEvent(
			screen.getByRole('tab', {
				name: /^Research A/,
			}),
			new MouseEvent('auxclick', {
				button: 1,
				bubbles: true,
			}),
		);
		expect(screen.queryByRole('alertdialog')).toBeNull();
		expect(
			screen
				.getByRole('tab', {
					name: /^Research A/,
				})
				.closest('.ct-horizontal-item')
				?.hasAttribute('data-closing'),
		).toBe(true);
		act(() => vi.advanceTimersByTime(200));
		expect(model.getIndexOfTab('research-a')).toBeNull();
		expect(model.getGroup('research')).toBeNull();
	});
	it('does not close a tab when a middle-button press is released elsewhere', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		fireEvent.pointerDown(loose, {
			button: 1,
			pointerId: 31,
			clientX: 300,
			clientY: 20,
		});
		fireEvent.pointerUp(window, {
			button: 1,
			pointerId: 31,
			clientX: -10,
			clientY: -10,
		});
		act(() => vi.advanceTimersByTime(200));
		expect(model.getIndexOfTab('loose')).not.toBeNull();
		expect(loose.closest('.ct-horizontal-item')?.hasAttribute('data-closing')).toBe(false);
	});
	it('renders the still-mounted closing tab as inactive', () => {
		vi.useFakeTimers();
		const model = createModel();
		model.selectTab('loose');
		render(<TabStrip model={model} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		expect(loose.hasAttribute('data-active')).toBe(true);
		fireEvent(
			loose,
			new MouseEvent('auxclick', {
				button: 1,
				bubbles: true,
			}),
		);
		expect(
			screen
				.getByRole('tab', {
					name: /^Loose/,
				})
				.hasAttribute('data-active'),
		).toBe(false);
		act(() => vi.advanceTimersByTime(200));
	});
	it('holds touch closing mode for two seconds and exits after the delay', () => {
		vi.useFakeTimers();
		const model = createModel();
		model.selectTab('loose');
		render(<TabStrip model={model} tabStripDeclutter={false} />);
		const close = within(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
		).getByRole('button', {
			name: 'Close',
		});
		fireEvent.pointerDown(close, {
			button: 0,
			pointerType: 'touch',
			clientX: 14,
			clientY: 14,
		});
		fireEvent.click(close);
		expect(
			screen
				.getByRole('tablist', {
					name: 'Browser tabs',
				})
				.getAttribute('data-tab-closing-mode'),
		).toBe('touch');
		act(() => vi.advanceTimersByTime(1_999));
		expect(
			screen
				.getByRole('tablist', {
					name: 'Browser tabs',
				})
				.getAttribute('data-tab-closing-mode'),
		).toBe('touch');
		act(() => vi.advanceTimersByTime(1));
		expect(
			screen
				.getByRole('tablist', {
					name: 'Browser tabs',
				})
				.hasAttribute('data-tab-closing-mode'),
		).toBe(false);
	});
	it('exits closing mode when a tab is inserted', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} />);
		fireEvent(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			new MouseEvent('auxclick', {
				button: 1,
				bubbles: true,
			}),
		);
		expect(
			screen
				.getByRole('tablist', {
					name: 'Browser tabs',
				})
				.getAttribute('data-tab-closing-mode'),
		).toBe('mouse');
		act(() =>
			model.addTab({
				title: 'Inserted',
				pinned: false,
				group: null,
				split: null,
			}),
		);
		expect(
			screen
				.getByRole('tablist', {
					name: 'Browser tabs',
				})
				.hasAttribute('data-tab-closing-mode'),
		).toBe(false);
	});
	it('toggles audio from the alert indicator without selecting or dragging the tab', () => {
		const model = createModel();
		model.updateTab('loose', {
			alertState: 'audio-playing',
		});
		model.selectTab('loose');
		const onToggle = vi.fn<() => void>();
		render(<TabStrip model={model} tabStripDeclutter onTabAudioMuteToggle={onToggle} />);
		fireEvent.pointerDown(
			screen.getByRole('button', {
				name: 'Mute tab audio',
			}),
			{
				button: 0,
				pointerId: 3,
			},
		);
		fireEvent.click(
			screen.getByRole('button', {
				name: 'Mute tab audio',
			}),
		);
		expect(model.getSnapshot().activeTabId).toBe('loose');
		expect(model.getSnapshot().tabs.find((tab) => tab.id === 'loose')?.alertState).toBe(
			'audio-muting',
		);
		expect(onToggle).toHaveBeenCalledWith('loose', true);
		expect(
			screen.getByRole('button', {
				name: 'Unmute tab audio',
			}),
		).toBeTruthy();
	});
	it('renders the collapsed vertical strip with preserved accessible labels', () => {
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} verticalCollapsed />);
		expect(
			screen
				.getByRole('tablist', {
					name: 'Browser tabs',
				})
				.getAttribute('aria-orientation'),
		).toBe('vertical');
		expect(
			screen.getByRole('tab', {
				name: /^Research A/,
			}),
		).toBeTruthy();
		expect(
			screen.getByRole('tablist', {
				name: /^Research, 2 tabs.*expanded$/,
			}),
		).toBeTruthy();
	});
	it('exposes the vertical group editor button separately from collapse', () => {
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const options = screen.getByRole('button', {
			name: 'More options',
			hidden: true,
		});
		fireEvent.click(options);
		expect(screen.getByRole('menu')).toBeTruthy();
		expect(
			screen.getByRole('textbox', {
				name: 'Group name',
			}),
		).toBeTruthy();
		expect(model.getSnapshot().groups[0]?.collapsed).toBe(false);
	});
	it('dismisses vertical hover cards and the group editor when tabs scroll', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 0, 216, 100, 32);
		fireEvent.mouseEnter(loose);
		act(() => vi.advanceTimersByTime(300));
		expect(getHoverCard()).toBeTruthy();
		const scroller = requireDefined(document.querySelector('.ct-vertical-scroll'));
		fireEvent.scroll(scroller);
		expect(document.querySelector('.ct-hover-card')).toBeNull();
		fireEvent.click(
			screen.getByRole('button', {
				name: 'More options',
				hidden: true,
			}),
		);
		expect(
			screen.getByRole('textbox', {
				name: 'Group name',
			}),
		).toBeTruthy();
		fireEvent.scroll(scroller);
		expect(
			screen.getByRole('textbox', {
				name: 'Group name',
			}),
		).toBeTruthy();
		act(() => vi.advanceTimersByTime(250));
		fireEvent.scroll(scroller);
		expect(
			screen.queryByRole('textbox', {
				name: 'Group name',
			}),
		).toBeNull();
	});
	it('closes the group editor when the tab-strip orientation changes', () => {
		const model = createModel();
		const { rerender } = render(
			<TabStrip model={model} orientation={TabStripOrientation.Vertical} />,
		);
		fireEvent.click(
			screen.getByRole('button', {
				name: 'More options',
				hidden: true,
			}),
		);
		expect(
			screen.getByRole('textbox', {
				name: 'Group name',
			}),
		).toBeTruthy();
		rerender(<TabStrip model={model} orientation={TabStripOrientation.Horizontal} />);
		expect(
			screen.queryByRole('textbox', {
				name: 'Group name',
			}),
		).toBeNull();
	});
	it('lays vertical split tabs side-by-side when expanded and stacks them when collapsed', () => {
		const model = createModel();
		model.updateTab('research-a', {
			split: 'research-split',
		});
		model.updateTab('research-b', {
			split: 'research-split',
		});
		const { rerender } = render(
			<TabStrip model={model} orientation={TabStripOrientation.Vertical} />,
		);
		expect(
			document.querySelector('[data-split-id="research-split"]')?.getAttribute('data-layout'),
		).toBe('side-by-side');
		expect(
			screen.getByRole('tab', {
				name: /^Research A, left view/,
			}),
		).toBeTruthy();
		expect(
			screen.getByRole('tab', {
				name: /^Research B, right view/,
			}),
		).toBeTruthy();
		rerender(
			<TabStrip model={model} orientation={TabStripOrientation.Vertical} verticalCollapsed />,
		);
		expect(
			document.querySelector('[data-split-id="research-split"]')?.getAttribute('data-layout'),
		).toBe('stacked');
		expect(
			screen.getByRole('tab', {
				name: /^Research A, top view/,
			}),
		).toBeTruthy();
		expect(
			screen.getByRole('tab', {
				name: /^Research B, bottom view/,
			}),
		).toBeTruthy();
	});
	it('propagates SplitTabView hover to both inactive child tabs', () => {
		const model = createModel();
		model.updateTab('research-a', {
			split: 'research-split',
		});
		model.updateTab('research-b', {
			split: 'research-split',
		});
		model.selectTab('loose');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const split = requireDefined(
			document.querySelector<HTMLElement>('[data-split-id="research-split"]'),
		);
		const left = screen.getByRole('tab', {
			name: /^Research A, left view/,
		});
		const right = screen.getByRole('tab', {
			name: /^Research B, right view/,
		});
		expect(
			within(left).queryByRole('button', {
				name: 'Close',
			}),
		).toBeNull();
		expect(
			within(right).queryByRole('button', {
				name: 'Close',
			}),
		).toBeNull();
		fireEvent.mouseEnter(split);
		expect(split.hasAttribute('data-hovered')).toBe(true);
		expect(
			within(left).getByRole('button', {
				name: 'Close',
			}),
		).toBeTruthy();
		expect(
			within(right).getByRole('button', {
				name: 'Close',
			}),
		).toBeTruthy();
		fireEvent.mouseLeave(split);
		expect(
			within(left).queryByRole('button', {
				name: 'Close',
			}),
		).toBeNull();
		expect(
			within(right).queryByRole('button', {
				name: 'Close',
			}),
		).toBeNull();
	});
	it('renders shared-group and collapsed-attention visuals with source dimensions', () => {
		const model = createModel();
		model.selectTab('loose');
		model.updateGroup('research', {
			shared: true,
			attention: true,
			collapsed: true,
		});
		render(<TabStrip model={model} />);
		const header = screen.getByRole('tablist', {
			name: /^Shared, Research, 2 tabs.*collapsed/,
		});
		const chip = header.querySelector<HTMLElement>('.ct-group-header__chip');
		expect(Number.parseFloat(chip?.style.width ?? '')).toBeCloseTo(99.6);
		expect(chip?.hasAttribute('data-shared')).toBe(true);
		expect(chip?.hasAttribute('data-attention')).toBe(true);
		expect(chip?.querySelector('.ct-group-header__sync')).toBeTruthy();
		expect(chip?.querySelector('.ct-group-header__attention')).toBeTruthy();
		fireEvent.click(header);
		expect(header.querySelector('.ct-group-header__sync')).toBeTruthy();
		expect(header.querySelector('.ct-group-header__attention')).toBeNull();
	});
	it('opens shared-group management and recent activity from the group editor', () => {
		const model = createModel();
		model.updateGroup('research', {
			shared: true,
			attention: true,
			members: Array.from(
				{
					length: 5,
				},
				(_, index) => ({
					id: `${index}`,
					displayName: `Member ${index}`,
				}),
			),
			recentActivity: [
				{
					id: 'focus',
					scope: 'tab',
					title: 'Research A updated',
					description: 'Alex',
					timeText: '2m',
					action: 'focus-tab',
					tabId: 'research-a',
				},
			],
		});
		model.selectTab('loose');
		const onManage = vi.fn<() => void>();
		render(<TabStrip model={model} onManageGroupSharing={onManage} />);
		fireEvent.contextMenu(
			screen.getByRole('tablist', {
				name: /^Shared, Research/,
			}),
			{
				clientX: 20,
				clientY: 20,
			},
		);
		expect(
			screen.getByRole('menuitem', {
				name: 'Manage group, 5 members',
			}),
		).toBeTruthy();
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Recent activity',
			}),
		);
		expect(model.getGroup('research')?.attention).toBe(false);
		expect(
			screen.getByRole('dialog', {
				name: 'Recent activity',
			}),
		).toBeTruthy();
		fireEvent.click(
			screen.getByRole('row', {
				name: 'Research A updated',
			}),
		);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Manage group, 5 members',
			}),
		);
		expect(onManage).toHaveBeenCalledWith('research');
	});
	it('focuses and unfocuses a group from its editor while collapsing every other horizontal slot', () => {
		const model = createModel();
		model.updateGroup('research', {
			saved: true,
		});
		render(<TabStrip model={model} enableTabGroupFocusing />);
		fireEvent.contextMenu(
			screen.getByRole('tablist', {
				name: /^Research, 2 tabs/,
			}),
			{
				clientX: 20,
				clientY: 20,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Focus group',
			}),
		);
		expect(model.getSnapshot().focusedGroupId).toBe('research');
		expect(
			screen
				.getByRole('tablist', {
					name: 'Browser tabs',
				})
				.getAttribute('data-focused-group'),
		).toBe('research');
		expect(
			screen.getByRole('tab', {
				name: /^Research A/,
			}),
		).toBeTruthy();
		expect(
			screen.queryByRole('tab', {
				name: /^Pinned/,
			}),
		).toBeNull();
		expect(
			screen.queryByRole('tab', {
				name: /^Loose/,
			}),
		).toBeNull();
		fireEvent.contextMenu(
			screen.getByRole('tablist', {
				name: /^Research, 2 tabs/,
			}),
			{
				clientX: 20,
				clientY: 20,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Unfocus group',
			}),
		);
		expect(model.getSnapshot().focusedGroupId).toBeNull();
		expect(
			screen.getByRole('tab', {
				name: /^Pinned/,
			}),
		).toBeTruthy();
		expect(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
		).toBeTruthy();
	});
	it('ports group-editor color traversal, new-tab insertion, and host-only commands', () => {
		const model = createModel();
		model.updateGroup('research', {
			saved: true,
		});
		const onMove = vi.fn<() => void>();
		const onShare = vi.fn<() => void>();
		const onAskGemini = vi.fn<() => void>();
		render(
			<TabStrip
				model={model}
				onMoveGroupToNewWindow={onMove}
				onShareGroup={onShare}
				onAskGeminiAboutGroup={onAskGemini}
			/>,
		);
		const openEditor = () =>
			fireEvent.contextMenu(
				screen.getByRole('tablist', {
					name: /^Research, \d+ tabs/,
				}),
				{
					clientX: 20,
					clientY: 20,
				},
			);
		openEditor();
		const blue = screen.getByRole('radio', {
			name: 'blue',
		});
		fireEvent.keyDown(blue, {
			key: 'ArrowRight',
		});
		expect(model.getGroup('research')?.color).toBe('red');
		expect(document.activeElement).toBe(
			screen.getByRole('radio', {
				name: 'red',
			}),
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'New tab in group',
			}),
		);
		expect(model.getSnapshot().tabs.filter((tab) => tab.group === 'research')).toHaveLength(3);
		openEditor();
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Move group to new window',
			}),
		);
		expect(onMove).toHaveBeenCalledWith('research');
		openEditor();
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Share group',
			}),
		);
		expect(onShare).toHaveBeenCalledWith('research');
		openEditor();
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Ask Gemini',
			}),
		);
		expect(onAskGemini).toHaveBeenCalledWith('research');
	});
	it('hides move-window for the only group and exposes the feature-gated group home command', () => {
		const model = createModel();
		model.closeTabs(['pinned', 'loose']);
		const onMove = vi.fn<() => void>();
		render(<TabStrip model={model} onMoveGroupToNewWindow={onMove} enableTabGroupHome />);
		fireEvent.contextMenu(
			screen.getByRole('tablist', {
				name: /^Research, 2 tabs/,
			}),
			{
				clientX: 20,
				clientY: 20,
			},
		);
		expect(
			screen.queryByRole('menuitem', {
				name: 'Move group to new window',
			}),
		).toBeNull();
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Open tab group home',
			}),
		);
		const home = model.getSnapshot().tabs.find((tab) => tab.url === 'chrome://tab-group-home/');
		expect(home).toMatchObject({
			title: 'Tab Group Home',
			group: null,
		});
		expect(model.getSnapshot().activeTabId).toBe(home?.id);
	});
	it('adds a new tab to the most recently activated group, not the last group in model order', () => {
		const model = new TabModel({
			tabs: [
				{
					id: 'work',
					title: 'Work tab',
					pinned: false,
					group: 'work-group',
					split: null,
				},
				{
					id: 'read',
					title: 'Read tab',
					pinned: false,
					group: 'read-group',
					split: null,
				},
				{
					id: 'outside',
					title: 'Outside',
					pinned: false,
					group: null,
					split: null,
				},
			],
			groups: [
				{
					id: 'read-group',
					title: 'Read',
					color: 'green',
					collapsed: false,
				},
				{
					id: 'work-group',
					title: 'Work',
					color: 'blue',
					collapsed: false,
				},
			],
			activeTabId: 'work',
		});
		model.selectTab('read');
		model.selectTab('outside');
		render(<TabStrip model={model} />);
		fireEvent.contextMenu(
			screen.getByRole('button', {
				name: 'New tab',
			}),
			{
				clientX: 20,
				clientY: 20,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'New tab in Read',
			}),
		);
		expect(model.getSnapshot().tabs.filter((tab) => tab.group === 'read-group')).toHaveLength(2);
		expect(model.getSnapshot().tabs.filter((tab) => tab.group === 'work-group')).toHaveLength(1);
	});
	it('confirms destructive saved-group actions before changing the model', () => {
		const model = createModel();
		model.updateGroup('research', {
			saved: true,
		});
		render(<TabStrip model={model} />);
		fireEvent.contextMenu(
			screen.getByRole('tablist', {
				name: /^Research, 2 tabs/,
			}),
			{
				clientX: 20,
				clientY: 20,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Ungroup',
			}),
		);
		expect(screen.getByRole('alertdialog').textContent).toContain('Ungroup saved group?');
		expect(model.getGroup('research')).not.toBeNull();
		fireEvent.click(
			screen.getByRole('button', {
				name: 'Cancel',
			}),
		);
		expect(model.getGroup('research')).not.toBeNull();
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Ungroup',
			}),
		);
		fireEvent.click(
			screen.getByRole('button', {
				name: 'Ungroup',
			}),
		);
		expect(model.getGroup('research')).toBeNull();
	});
	it('skips saved-group dialogs when the deletion-dialog preference is disabled', () => {
		const model = createModel();
		model.updateGroup('research', {
			saved: true,
		});
		render(<TabStrip model={model} confirmSavedGroupDestructiveActions={false} />);
		fireEvent.contextMenu(
			screen.getByRole('tablist', {
				name: /^Research, 2 tabs/,
			}),
			{
				clientX: 20,
				clientY: 20,
			},
		);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Ungroup',
			}),
		);
		expect(screen.queryByRole('alertdialog')).toBeNull();
		expect(model.getGroup('research')).toBeNull();
	});
	it('converts a saved group only after the host bookmark editor confirms and honors policy', async () => {
		const model = createModel();
		model.updateGroup('research', {
			saved: true,
		});
		const onConvert = vi.fn<() => Promise<boolean>>().mockResolvedValue(true);
		const { rerender } = render(
			<TabStrip model={model} onConvertGroupToBookmark={onConvert} bookmarksEditable={false} />,
		);
		const openEditor = () =>
			fireEvent.contextMenu(
				screen.getByRole('tablist', {
					name: /^Research, 2 tabs/,
				}),
				{
					clientX: 20,
					clientY: 20,
				},
			);
		openEditor();
		expect(
			screen.queryByRole('menuitem', {
				name: 'Convert group to bookmark',
			}),
		).toBeNull();
		fireEvent.pointerDown(document.body);
		rerender(<TabStrip model={model} onConvertGroupToBookmark={onConvert} />);
		openEditor();
		await act(async () => {
			fireEvent.click(
				screen.getByRole('menuitem', {
					name: 'Convert group to bookmark',
				}),
			);
		});
		expect(onConvert).toHaveBeenCalledWith(
			'research',
			expect.arrayContaining([
				expect.objectContaining({
					id: 'research-a',
				}),
			]),
		);
		expect(model.getGroup('research')).toBeNull();
	});
	it('keeps a saved group when bookmark conversion rejects and closes the editor cleanly', async () => {
		const model = createModel();
		model.updateGroup('research', {
			saved: true,
		});
		const onConvert = vi
			.fn<() => Promise<boolean>>()
			.mockRejectedValue(new Error('host editor unavailable'));
		render(<TabStrip model={model} onConvertGroupToBookmark={onConvert} />);
		fireEvent.contextMenu(
			screen.getByRole('tablist', {
				name: /^Research, 2 tabs/,
			}),
			{
				clientX: 20,
				clientY: 20,
			},
		);
		await act(async () => {
			fireEvent.click(
				screen.getByRole('menuitem', {
					name: 'Convert group to bookmark',
				}),
			);
		});
		expect(model.getGroup('research')).not.toBeNull();
		expect(screen.queryByRole('menu')).toBeNull();
	});
	it('uses model focus for vertical filtering, keeps focused collapsed tabs visible, and wires Show all tabs', () => {
		const model = createModel();
		model.updateGroup('research', {
			collapsed: true,
		});
		model.setFocusedGroup('research');
		const onUnfocus = vi.fn<() => void>();
		render(
			<TabStrip
				model={model}
				orientation={TabStripOrientation.Vertical}
				onVerticalUnfocusGroup={onUnfocus}
			/>,
		);
		expect(
			screen.getByRole('tab', {
				name: /^Research A/,
			}),
		).toBeTruthy();
		expect(
			screen.getByRole('tab', {
				name: /^Research B/,
			}),
		).toBeTruthy();
		expect(
			screen
				.getByRole('tab', {
					name: /^Research A/,
				})
				.closest('.ct-vertical-group')
				?.hasAttribute('data-group-collapsed'),
		).toBe(false);
		expect(
			screen.queryByRole('tab', {
				name: /^Pinned/,
			}),
		).toBeNull();
		expect(
			screen.queryByRole('tab', {
				name: /^Loose/,
			}),
		).toBeNull();
		fireEvent.click(
			screen.getByRole('button', {
				name: 'Show all tabs',
			}),
		);
		expect(model.getSnapshot().focusedGroupId).toBeNull();
		expect(onUnfocus).toHaveBeenCalledOnce();
		expect(
			screen.getByRole('tab', {
				name: /^Pinned/,
			}),
		).toBeTruthy();
		expect(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
		).toBeTruthy();
	});
	it('wires the vertical top-container actions and expand-on-hover preference menu', () => {
		const model = createModel();
		const onCollapsedChange = vi.fn<() => void>();
		const onProjectsClick = vi.fn<() => void>();
		const onExpandOnHoverChange = vi.fn<() => void>();
		render(
			<TabStrip
				model={model}
				orientation={TabStripOrientation.Vertical}
				showVerticalProjectsButton
				verticalProjectsOpen
				onVerticalCollapsedChange={onCollapsedChange}
				onVerticalProjectsClick={onProjectsClick}
				onVerticalExpandOnHoverChange={onExpandOnHoverChange}
			/>,
		);
		fireEvent.click(
			screen.getByRole('button', {
				name: 'Collapse tab strip',
			}),
		);
		expect(onCollapsedChange).toHaveBeenCalledWith(true);
		const projects = screen.getByRole('button', {
			name: 'Projects',
		});
		expect(projects.getAttribute('aria-pressed')).toBe('true');
		fireEvent.click(projects);
		expect(onProjectsClick).toHaveBeenCalledOnce();
		fireEvent.contextMenu(
			screen.getByRole('button', {
				name: 'Collapse tab strip',
			}),
			{
				clientX: 20,
				clientY: 20,
			},
		);
		const preference = screen.getByRole('menuitemcheckbox', {
			name: 'Expand tab strip on hover',
		});
		expect(preference.getAttribute('aria-checked')).toBe('true');
		fireEvent.click(preference);
		expect(onExpandOnHoverChange).toHaveBeenCalledWith(false);
	});
	it('unpins a combo button from its context menu and keeps it mounted through the exit animation', () => {
		vi.useFakeTimers();
		render(
			<TabStrip
				model={createModel()}
				orientation={TabStripOrientation.Vertical}
				showVerticalProjectsButton
			/>,
		);
		const projects = screen.getByRole('button', {
			name: 'Projects',
		});
		fireEvent.contextMenu(projects, {
			clientX: 20,
			clientY: 20,
		});
		const unpin = screen.getByRole('menuitemcheckbox', {
			name: 'Unpin Projects',
		});
		expect(unpin.getAttribute('aria-checked')).toBe('true');
		fireEvent.click(unpin);
		expect(
			screen
				.getByRole('button', {
					name: 'Projects',
				})
				.hasAttribute('data-target-visible'),
		).toBe(false);
		act(() => vi.advanceTimersByTime(299));
		expect(
			screen.getByRole('button', {
				name: 'Projects',
			}),
		).toBeTruthy();
		act(() => vi.advanceTimersByTime(1));
		expect(
			screen.queryByRole('button', {
				name: 'Projects',
			}),
		).toBeNull();
	});
	it('shows unpinned Tab Search while its bubble is open and hides it after the ephemeral delay', () => {
		vi.useFakeTimers();
		const controller = new TabStripComboButtonController({
			richAnimations: false,
		});
		const { rerender } = render(
			<TabStrip
				model={createModel()}
				orientation={TabStripOrientation.Vertical}
				verticalComboButtonController={controller}
			/>,
		);
		expect(
			screen.queryByRole('button', {
				name: 'Search tabs',
			}),
		).toBeNull();
		rerender(
			<TabStrip
				model={createModel()}
				orientation={TabStripOrientation.Vertical}
				verticalComboButtonController={controller}
				verticalTabSearchOpen
			/>,
		);
		expect(
			screen.getByRole('button', {
				name: 'Search tabs',
			}),
		).toBeTruthy();
		rerender(
			<TabStrip
				model={createModel()}
				orientation={TabStripOrientation.Vertical}
				verticalComboButtonController={controller}
			/>,
		);
		act(() => vi.advanceTimersByTime(1_999));
		expect(
			screen.getByRole('button', {
				name: 'Search tabs',
			}),
		).toBeTruthy();
		act(() => vi.advanceTimersByTime(1));
		expect(
			screen.queryByRole('button', {
				name: 'Search tabs',
			}),
		).toBeNull();
	});
	it('holds the vertical expand-on-hover state while the Tab Search bubble is open', () => {
		const comboController = new TabStripComboButtonController({
			richAnimations: false,
		});
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
	it('forwards a middle click on the shared new-tab button to the clipboard callback', () => {
		const model = createModel();
		const onNewTabFromClipboard = vi.fn<() => void>();
		render(
			<TabStrip
				model={model}
				orientation={TabStripOrientation.Vertical}
				onNewTabFromClipboard={onNewTabFromClipboard}
			/>,
		);
		fireEvent.pointerDown(
			screen.getByRole('button', {
				name: 'New tab',
			}),
			{
				button: 1,
				pointerId: 2,
			},
		);
		expect(onNewTabFromClipboard).toHaveBeenCalledOnce();
	});
	it('notifies the host before invoking the New Tab action', () => {
		const model = createModel();
		const order: string[] = [];
		render(
			<TabStrip
				model={model}
				orientation={TabStripOrientation.Vertical}
				onNewTabButtonPressed={() => order.push('pressed')}
				onNewTab={() => order.push('action')}
			/>,
		);
		fireEvent.click(
			screen.getByRole('button', {
				name: 'New tab',
			}),
		);
		expect(order).toEqual(['pressed', 'action']);
	});
	it('runs the Chromium new-tab button context menu commands', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const newTabButton = screen.getByRole('button', {
			name: 'New tab',
		});
		fireEvent.contextMenu(newTabButton, {
			clientX: 1000,
			clientY: 30,
		});
		expect(
			screen.getByRole('menu', {
				name: 'New tab menu',
			}),
		).toBeTruthy();
		expect(
			screen.getByRole('menu', {
				name: 'New tab menu',
			}).style.left,
		).toBe('756px');
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'New tab in Research',
			}),
		);
		expect(model.getSnapshot().tabs.filter((tab) => tab.group === 'research')).toHaveLength(3);
		fireEvent.contextMenu(newTabButton, {
			clientX: 400,
			clientY: 30,
		});
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'New split tab with current',
			}),
		);
		const active = model
			.getSnapshot()
			.tabs.find((tab) => tab.id === model.getSnapshot().activeTabId);
		expect(active?.split).not.toBeNull();
		expect(model.getSnapshot().tabs.filter((tab) => tab.split === active?.split)).toHaveLength(2);
		fireEvent.contextMenu(newTabButton, {
			clientX: 400,
			clientY: 30,
		});
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'New group',
			}),
		);
		expect(model.getSnapshot().groups).toHaveLength(2);
		expect(
			model.getSnapshot().tabs.find((tab) => tab.id === model.getSnapshot().activeTabId)?.group,
		).toBe('group-1');
	});
	it('labels an unnamed most-recent group by its tab count', () => {
		const model = createModel();
		model.updateGroup('research', {
			title: '',
		});
		render(<TabStrip model={model} />);
		fireEvent.contextMenu(
			screen.getByRole('button', {
				name: 'New tab',
			}),
			{
				clientX: 400,
				clientY: 30,
			},
		);
		expect(
			screen.getByRole('menuitem', {
				name: 'New tab in 2 tabs',
			}),
		).toBeTruthy();
	});
	it("expands a collapsed vertical strip after Chromium's inside-edge velocity delay", () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} verticalCollapsed />);
		const strip = screen.getByRole('tablist', {
			name: 'Browser tabs',
		});
		fireEvent.mouseEnter(strip);
		expect(strip.hasAttribute('data-waiting-to-expand')).toBe(true);
		act(() => vi.advanceTimersByTime(209));
		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(false);
		act(() => vi.advanceTimersByTime(1));
		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(true);
		expect(strip.hasAttribute('data-collapsed')).toBe(false);
		expect(
			screen.getByRole('button', {
				name: 'Expand tab strip',
			}),
		).toBeTruthy();
		fireEvent.mouseLeave(strip);
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
		expect(() =>
			fireEvent.mouseEnter(
				screen.getByRole('tablist', {
					name: 'Browser tabs',
				}),
			),
		).not.toThrow();
		expect(setHovered).toHaveBeenCalledWith(true);
	});
	it('keeps hover expansion locked while a tab menu is open', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} verticalCollapsed />);
		const strip = screen.getByRole('tablist', {
			name: 'Browser tabs',
		});
		fireEvent.mouseEnter(strip);
		act(() => vi.advanceTimersByTime(210));
		fireEvent.contextMenu(
			screen.getByRole('tab', {
				name: /^Loose/,
			}),
			{
				clientX: 50,
				clientY: 30,
			},
		);
		fireEvent.mouseEnter(screen.getByRole('menu'));
		fireEvent.mouseLeave(strip);
		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(true);
		fireEvent.click(
			screen.getByRole('menuitem', {
				name: 'Duplicate',
			}),
		);
		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(false);
	});
	it('reorders live after the scaled drag threshold and restores on cancel', () => {
		const model = createModel();
		const onDraggingChange = vi.fn<() => void>();
		render(<TabStrip model={model} onDraggingChange={onDraggingChange} />);
		const pinned = screen.getByRole('tab', {
			name: /^Pinned/,
		});
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(pinned, 0, 50);
		setRect(researchA, 50, 100);
		setRect(researchB, 150, 100);
		setRect(loose, 250, 100);
		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 9,
			clientX: 300,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 9,
			clientX: 60,
			clientY: 20,
		});
		expect(onDraggingChange).toHaveBeenLastCalledWith(true);
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'loose',
			'research-a',
			'research-b',
		]);
		fireEvent.pointerCancel(window, {
			pointerId: 9,
			clientX: 60,
			clientY: 20,
		});
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
	it('forwards the React pointer lifecycle and screen coordinates to a cross-window runtime', () => {
		const model = createModel();
		const runtime = {
			start: vi.fn<() => void>(),
			update: vi.fn<() => void>(),
			end: vi.fn<() => void>(),
			getSnapshot: vi.fn<
				() => {
					targetActive: boolean;
				}
			>(() => ({
				targetActive: true,
			})),
		};
		const createRuntime = vi.fn<
			(_session: DragSessionData<Readonly<TabGroupData>>) => typeof runtime
		>((_session: DragSessionData<Readonly<TabGroupData>>) => runtime);
		render(<TabStrip model={model} createCrossWindowDragRuntime={createRuntime} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 250, 100);
		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 40,
			clientX: 300,
			clientY: 20,
		});
		const session = requireDefined(createRuntime.mock.calls[0])[0];
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
		expect(runtime.update).toHaveBeenCalledWith({
			x: 460,
			y: 220,
		});
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual([
			'pinned',
			'research-a',
			'research-b',
			'loose',
		]);
		fireEvent.pointerUp(window, {
			pointerId: 40,
			clientX: 60,
			clientY: 20,
		});
		expect(runtime.end).toHaveBeenCalledWith('complete');
	});
	it('cancels a started cross-window runtime on Escape', () => {
		const model = createModel();
		const runtime = {
			start: vi.fn<() => void>(),
			update: vi.fn<() => void>(),
			end: vi.fn<() => void>(),
			getSnapshot: vi.fn<
				() => {
					targetActive: boolean;
				}
			>(() => ({
				targetActive: false,
			})),
		};
		render(<TabStrip model={model} createCrossWindowDragRuntime={() => runtime} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 250, 100);
		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 41,
			clientX: 300,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 41,
			clientX: 60,
			clientY: 20,
			screenX: 60,
			screenY: 20,
		});
		fireEvent.keyDown(window, {
			key: 'Escape',
		});
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
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 250, 100);
		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 42,
			clientX: 300,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 42,
			clientX: 60,
			clientY: 20,
			screenX: 60,
			screenY: 20,
		});
		expect(document.querySelector('.ct-drag-ghost')).not.toBeNull();
		act(() => model.closeTab('loose'));
		expect(runtime.end).toHaveBeenCalledWith('tab-destroyed');
		expect(document.querySelector('.ct-drag-ghost')).toBeNull();
	});
	it('cancels and restores a live drag when the React strip unmounts', () => {
		const model = createModel();
		const runtime = {
			start: vi.fn<() => void>(),
			update: vi.fn<() => void>(),
			end: vi.fn<() => void>(),
			getSnapshot: vi.fn<
				() => {
					targetActive: boolean;
				}
			>(() => ({
				targetActive: false,
			})),
		};
		const { unmount } = render(
			<TabStrip model={model} createCrossWindowDragRuntime={() => runtime} />,
		);
		const pinned = screen.getByRole('tab', {
			name: /^Pinned/,
		});
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(pinned, 0, 50);
		setRect(researchA, 50, 100);
		setRect(researchB, 150, 100);
		setRect(loose, 250, 100);
		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 43,
			clientX: 300,
			clientY: 20,
		});
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
		const group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		const pinned = screen.getByRole('tab', {
			name: /^Pinned/,
		});
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(pinned, 0, 50);
		setRect(group, 50, 70);
		setRect(researchA, 120, 100);
		setRect(researchB, 220, 100);
		setRect(loose, 320, 100);
		fireEvent.pointerDown(group, {
			button: 0,
			pointerId: 12,
			clientX: 80,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 12,
			clientX: 500,
			clientY: 20,
		});
		expect(model.getGroup('research')?.collapsed).toBe(true);
		const ghost = document.querySelector<HTMLElement>('.ct-drag-ghost[data-layout="horizontal"]');
		expect(ghost?.style.left).toBe('470px');
		expect(ghost?.style.width).toBe('234px');
		expect(ghost?.querySelectorAll('.ct-drag-ghost__group, .ct-drag-ghost__item')).toHaveLength(3);
		expect(ghost?.querySelector('.ct-drag-ghost__highlight path')?.getAttribute('d')).toContain(
			'M 0 41',
		);
		expect(ghost?.querySelector('.ct-drag-ghost__highlight')?.getAttribute('width')).toBe('234');
		expect(ghost?.querySelector('.ct-drag-ghost__underline')).not.toBeNull();
		expect(document.querySelector('.ct-group-drag-highlight')).toBeNull();
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
		fireEvent.pointerUp(window, {
			pointerId: 12,
			clientX: 500,
			clientY: 20,
		});
		expect(model.getGroup('research')?.collapsed).toBe(false);
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		const settlingGhost = document.querySelector<HTMLElement>(
			'.ct-drag-ghost[data-layout="horizontal"][data-settling]',
		);
		expect(settlingGhost).not.toBeNull();
		expect(settlingGhost?.querySelector('.ct-drag-ghost__highlight')).toBeNull();
		expect(
			settlingGhost?.querySelectorAll('.ct-drag-ghost__group, .ct-drag-ghost__item'),
		).toHaveLength(3);
		expect(settlingGhost?.hasAttribute('data-group-settling')).toBe(false);
	});
	it('resets horizontal group-header drag selection to its recorded active tab', () => {
		const model = createModel();
		model.setSelection(['pinned', 'loose'], 'loose');
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		const pinned = screen.getByRole('tab', {
			name: /^Pinned/,
		});
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(pinned, 0, 50);
		setRect(group, 50, 70);
		setRect(researchA, 120, 100);
		setRect(researchB, 220, 100);
		setRect(loose, 320, 100);
		fireEvent.pointerDown(group, {
			button: 0,
			pointerId: 62,
			clientX: 80,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 62,
			clientX: 500,
			clientY: 20,
		});
		fireEvent.pointerUp(window, {
			pointerId: 62,
			clientX: 500,
			clientY: 20,
		});
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
	});
	it('activates the first tab without expanding a collapsed horizontal group drag', () => {
		const model = createModel();
		model.selectTab('loose');
		model.updateGroup('research', {
			collapsed: true,
		});
		render(<TabStrip model={model} />);
		const group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*collapsed$/,
		});
		const researchA = requireDefined(
			document.querySelector<HTMLElement>('[data-tab-id="research-a"]'),
		);
		const researchB = requireDefined(
			document.querySelector<HTMLElement>('[data-tab-id="research-b"]'),
		);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(group, 50, 70);
		setRect(researchA, 120, 100);
		setRect(researchB, 220, 100);
		setRect(loose, 320, 100);
		fireEvent.pointerDown(group, {
			button: 0,
			pointerId: 63,
			clientX: 80,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 63,
			clientX: 500,
			clientY: 20,
		});
		expect(model.getGroup('research')?.collapsed).toBe(true);
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a', 'research-b']);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		const ghost = document.querySelector<HTMLElement>('.ct-drag-ghost[data-layout="horizontal"]');
		expect(ghost?.style.width).toBe('102px');
		expect(ghost?.querySelectorAll('.ct-drag-ghost__group, .ct-drag-ghost__item')).toHaveLength(1);
		fireEvent.pointerUp(window, {
			pointerId: 63,
			clientX: 500,
			clientY: 20,
		});
		expect(model.getGroup('research')?.collapsed).toBe(true);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		expect(
			screen
				.getByRole('tablist', {
					name: /^Research, 2 tabs.*collapsed$/,
				})
				.getAttribute('data-active-tab-id'),
		).toBe('research-a');
		expect(document.querySelector('.ct-drag-ghost[data-group-settling]')).toBeNull();
	});
	it('keeps each fully selected group highlight and underline independent during a mixed drag', () => {
		const model = new TabModel({
			tabs: [
				{
					id: 'a',
					title: 'A',
					pinned: false,
					group: 'one',
					split: null,
				},
				{
					id: 'b',
					title: 'B',
					pinned: false,
					group: 'one',
					split: null,
				},
				{
					id: 'c',
					title: 'C',
					pinned: false,
					group: 'two',
					split: null,
				},
				{
					id: 'd',
					title: 'D',
					pinned: false,
					group: 'two',
					split: null,
				},
			],
			groups: [
				{
					id: 'one',
					title: 'One',
					color: 'blue',
					collapsed: false,
				},
				{
					id: 'two',
					title: 'Two',
					color: 'red',
					collapsed: false,
				},
			],
			activeTabId: 'a',
		});
		model.setSelection(['a', 'b', 'c', 'd'], 'a');
		render(<TabStrip model={model} />);
		const one = screen.getByRole('tablist', {
			name: /^One, 2 tabs/,
		});
		const two = screen.getByRole('tablist', {
			name: /^Two, 2 tabs/,
		});
		const a = screen.getByRole('tab', {
			name: /^A/,
		});
		const b = screen.getByRole('tab', {
			name: /^B/,
		});
		const c = screen.getByRole('tab', {
			name: /^C/,
		});
		const d = screen.getByRole('tab', {
			name: /^D/,
		});
		setRect(one, 0, 60);
		setRect(a, 60, 100);
		setRect(b, 160, 100);
		setRect(two, 260, 60);
		setRect(c, 320, 100);
		setRect(d, 420, 100);
		fireEvent.pointerDown(a, {
			button: 0,
			pointerId: 86,
			clientX: 100,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 86,
			clientX: 120,
			clientY: 20,
		});
		const ghost = document.querySelector('.ct-drag-ghost[data-layout="horizontal"]');
		expect(ghost?.querySelectorAll('.ct-drag-ghost__highlight')).toHaveLength(2);
		expect(ghost?.querySelectorAll('.ct-drag-ghost__underline')).toHaveLength(2);
		expect(
			ghost
				?.querySelector<HTMLElement>('[data-group-underline-id="one"]')
				?.style.getPropertyValue('--ct-group-color'),
		).toBe('#1a73e8');
		expect(
			ghost
				?.querySelector<HTMLElement>('[data-group-underline-id="two"]')
				?.style.getPropertyValue('--ct-group-color'),
		).toBe('#d93025');
		fireEvent.pointerUp(window, {
			pointerId: 86,
			clientX: 120,
			clientY: 20,
		});
	});
	it('coalesces a nonconsecutive selection on the first drag move before the width threshold', () => {
		const model = new TabModel({
			tabs: [
				{
					id: 'a',
					title: 'A',
					pinned: false,
					group: null,
					split: null,
				},
				{
					id: 'b',
					title: 'B',
					pinned: false,
					group: null,
					split: null,
				},
				{
					id: 'c',
					title: 'C',
					pinned: false,
					group: null,
					split: null,
				},
				{
					id: 'd',
					title: 'D',
					pinned: false,
					group: null,
					split: null,
				},
			],
			activeTabId: 'a',
		});
		model.setSelection(['a', 'c'], 'a');
		render(<TabStrip model={model} />);
		const a = screen.getByRole('tab', {
			name: /^A/,
		});
		const b = screen.getByRole('tab', {
			name: /^B/,
		});
		const c = screen.getByRole('tab', {
			name: /^C/,
		});
		const d = screen.getByRole('tab', {
			name: /^D/,
		});
		setRect(a, 0, 100);
		setRect(b, 100, 100);
		setRect(c, 200, 100);
		setRect(d, 300, 100);
		fireEvent.pointerDown(a, {
			button: 0,
			pointerId: 87,
			clientX: 50,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 87,
			clientX: 61,
			clientY: 20,
		});
		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['a', 'c', 'b', 'd']);
		expect(document.querySelector('[data-source][data-compacting]')).toBeNull();
		expect(document.querySelectorAll('[data-compacting]')).toHaveLength(1);
		fireEvent.pointerUp(window, {
			pointerId: 87,
			clientX: 61,
			clientY: 20,
		});
	});
	it('keeps a later source tab under the pointer while nonconsecutive peers compact toward it', () => {
		const model = new TabModel({
			tabs: [
				{
					id: 'a',
					title: 'A',
					pinned: false,
					group: null,
					split: null,
				},
				{
					id: 'b',
					title: 'B',
					pinned: false,
					group: null,
					split: null,
				},
				{
					id: 'c',
					title: 'C',
					pinned: false,
					group: null,
					split: null,
				},
				{
					id: 'd',
					title: 'D',
					pinned: false,
					group: null,
					split: null,
				},
			],
			activeTabId: 'c',
			selectedTabIds: ['a', 'c'],
		});
		render(<TabStrip model={model} />);
		const a = screen.getByRole('tab', {
			name: /^A/,
		});
		const b = screen.getByRole('tab', {
			name: /^B/,
		});
		const c = screen.getByRole('tab', {
			name: /^C/,
		});
		const d = screen.getByRole('tab', {
			name: /^D/,
		});
		setRect(a, 0, 100);
		setRect(b, 100, 100);
		setRect(c, 200, 100);
		setRect(d, 300, 100);
		fireEvent.pointerDown(c, {
			button: 0,
			pointerId: 88,
			clientX: 250,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 88,
			clientX: 261,
			clientY: 20,
		});
		const ghost = document.querySelector<HTMLElement>('.ct-drag-ghost[data-layout="horizontal"]');
		const source = ghost?.querySelector<HTMLElement>('[data-source]');
		expect(ghost?.style.left).toBe('129px');
		expect(source?.style.left).toBe('82px');
		expect(source?.hasAttribute('data-compacting')).toBe(false);
		fireEvent.pointerUp(window, {
			pointerId: 88,
			clientX: 261,
			clientY: 20,
		});
	});
	it('uses the packed mouse offset for the insertion target of a later source tab', () => {
		const model = new TabModel({
			tabs: ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'].map((id) => ({
				id,
				title: id.toUpperCase(),
				pinned: false,
				group: null,
				split: null,
			})),
			activeTabId: 'g',
			selectedTabIds: ['c', 'g'],
		});
		render(<TabStrip model={model} />);
		const renderedTabs = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'].map((name, index) => {
			const tab = screen.getByRole('tab', {
				name: new RegExp(`^${name}`),
			});
			setRect(tab, index * 100, 100);
			return tab;
		});
		fireEvent.pointerDown(requireDefined(renderedTabs[6]), {
			button: 0,
			pointerId: 89,
			clientX: 650,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 89,
			clientX: 661,
			clientY: 20,
		});
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
		fireEvent.pointerUp(window, {
			pointerId: 89,
			clientX: 661,
			clientY: 20,
		});
	});
	it('cancels a local group-header drag before a model tab is added', () => {
		const model = createModel();
		const onDraggingChange = vi.fn<() => void>();
		render(<TabStrip model={model} onDraggingChange={onDraggingChange} />);
		const group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(group, 50, 70);
		setRect(researchA, 120, 100);
		setRect(researchB, 220, 100);
		setRect(loose, 320, 100);
		fireEvent.pointerDown(group, {
			button: 0,
			pointerId: 44,
			clientX: 80,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 44,
			clientX: 500,
			clientY: 20,
		});
		expect(document.querySelector('.ct-drag-ghost')).not.toBeNull();
		expect(model.getGroup('research')?.collapsed).toBe(true);
		act(() => {
			model.addTab(
				{
					title: 'Added',
					pinned: false,
					group: null,
					split: null,
				},
				{
					id: 'added',
				},
			);
		});
		expect(document.querySelector('.ct-drag-ghost')).toBeNull();
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
	it('merges a partial group drag underline with the normal group underline', () => {
		const model = createModel();
		model.setSelection(['research-a'], 'pinned');
		render(<TabStrip model={model} />);
		const pinned = screen.getByRole('tab', {
			name: /^Pinned/,
		});
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		const underline = requireDefined(
			document.querySelector<HTMLElement>('[data-group-underline-id="research"]'),
		);
		setRect(pinned, 0, 50);
		setRect(researchA, 50, 100);
		setRect(researchB, 150, 100);
		setRect(loose, 250, 100);
		setRect(underline, 28, 154, 39, 2);
		fireEvent.pointerDown(researchA, {
			button: 0,
			pointerId: 17,
			clientX: 100,
			clientY: 20,
		});
		fireEvent.pointerMove(window, {
			pointerId: 17,
			clientX: 111,
			clientY: 20,
		});
		const dragUnderline = document.querySelector<HTMLElement>('.ct-drag-ghost__underline');
		expect(dragUnderline?.style.left).toBe('0px');
		expect(dragUnderline?.style.width).toBe('100px');
		fireEvent.pointerUp(window, {
			pointerId: 17,
			clientX: 111,
			clientY: 20,
		});
	});
	it('keeps a previously collapsed group collapsed throughout header dragging', () => {
		const model = createModel();
		model.selectTab('loose');
		model.updateGroup('research', {
			collapsed: true,
		});
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*collapsed$/,
		});
		setRect(group, 0, 216, 50, 30);
		fireEvent.pointerDown(group, {
			button: 0,
			pointerId: 13,
			clientX: 100,
			clientY: 60,
		});
		fireEvent.pointerMove(window, {
			pointerId: 13,
			clientX: 100,
			clientY: 120,
		});
		expect(model.getGroup('research')?.collapsed).toBe(true);
		expect(document.querySelector<HTMLElement>('.ct-drag-ghost')?.style.height).toBe('36px');
		expect(document.querySelector<HTMLElement>('.ct-drag-ghost__group-view')?.style.height).toBe(
			'34px',
		);
		fireEvent.pointerUp(window, {
			pointerId: 13,
			clientX: 100,
			clientY: 120,
		});
		expect(model.getGroup('research')?.collapsed).toBe(true);
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		expect(
			screen
				.getByRole('tablist', {
					name: /^Research, 2 tabs.*collapsed$/,
				})
				.getAttribute('data-active-tab-id'),
		).toBe('research-a');
	});
	it('restores the common-handler group-header selection rule when a vertical drag is canceled', () => {
		const model = createModel();
		model.selectTab('loose');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		setRect(requireDefined(group.closest<HTMLElement>('.ct-vertical-group')), 0, 216, 50, 100);
		setRect(group, 0, 216, 54, 26);
		fireEvent.pointerDown(group, {
			button: 0,
			pointerId: 131,
			clientX: 100,
			clientY: 65,
		});
		fireEvent.pointerMove(window, {
			pointerId: 131,
			clientX: 100,
			clientY: 130,
		});
		fireEvent.pointerCancel(window, {
			pointerId: 131,
			clientX: 100,
			clientY: 130,
		});
		expect(model.getGroup('research')?.collapsed).toBe(false);
		expect([...model.getSnapshot().selectedTabIds]).toEqual(['research-a']);
		expect(model.getSnapshot().activeTabId).toBe('research-a');
	});
	it('anchors and stacks selected vertical tabs with Chromium dragged-container geometry', () => {
		const model = createModel();
		model.setSelection(['research-a', 'research-b'], 'research-b');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		const groupView = requireDefined(group.closest<HTMLElement>('.ct-vertical-group'));
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
		setRect(groupView, 0, 216, 64, 100);
		setRect(group, 12, 216, 68, 30);
		setRect(researchA, 12, 216, 100, 30);
		setRect(researchB, 12, 216, 132, 30);
		fireEvent.pointerDown(researchB, {
			button: 0,
			pointerId: 14,
			clientX: 100,
			clientY: 147,
		});
		fireEvent.pointerMove(window, {
			pointerId: 14,
			clientX: 100,
			clientY: 200,
		});
		const ghost = document.querySelector<HTMLElement>('.ct-drag-ghost[data-layout="vertical"]');
		expect(ghost?.style.left).toBe('0px');
		expect(ghost?.style.top).toBe('117px');
		const draggedGroup = ghost?.querySelector<HTMLElement>(
			'.ct-drag-ghost__group-view .ct-vertical-group',
		);
		expect(draggedGroup?.querySelector('[data-group-id="research"]')).not.toBeNull();
		expect(
			[...(draggedGroup?.querySelectorAll<HTMLElement>('[data-tab-id]') ?? [])].map(
				(tab) => tab.dataset['tabId'],
			),
		).toEqual(['research-a', 'research-b']);
		expect(ghost?.querySelectorAll('.ct-drag-ghost__item')).toHaveLength(0);
		expect(groupView.hasAttribute('data-drag-source')).toBe(true);
		expect(researchA.closest('.ct-vertical-tab-wrap')?.hasAttribute('data-drag-source')).toBe(
			false,
		);
		expect(researchB.closest('.ct-vertical-tab-wrap')?.hasAttribute('data-drag-source')).toBe(
			false,
		);
		fireEvent.pointerUp(window, {
			pointerId: 14,
			clientX: 100,
			clientY: 200,
		});
	});
	it('drags a split as one collection view and preserves both split members', () => {
		const model = createModel();
		model.moveTabsToGroup(['research-a', 'research-b'], null);
		model.updateTab('research-a', {
			split: 'research-split',
		});
		model.updateTab('research-b', {
			split: 'research-split',
		});
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const split = requireDefined(
			document.querySelector<HTMLElement>('[data-split-id="research-split"]'),
		);
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		const researchB = screen.getByRole('tab', {
			name: /^Research B/,
		});
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(split, 16, 216, 100, 30);
		setRect(researchA, 16, 107, 100, 30);
		setRect(researchB, 125, 107, 100, 30);
		setRect(loose, 12, 216, 140, 30);
		fireEvent.pointerDown(researchA, {
			button: 0,
			pointerId: 15,
			clientX: 60,
			clientY: 115,
		});
		fireEvent.pointerMove(window, {
			pointerId: 15,
			clientX: 60,
			clientY: 210,
		});
		const draggedSplit = document.querySelector<HTMLElement>(
			'.ct-drag-ghost__split .ct-vertical-split',
		);
		expect(draggedSplit?.dataset['layout']).toBe('side-by-side');
		expect(draggedSplit?.querySelectorAll('[data-tab-id]')).toHaveLength(2);
		expect(
			[...(draggedSplit?.querySelectorAll<HTMLElement>('[data-tab-id]') ?? [])].map(
				(tab) => tab.dataset['tabId'],
			),
		).toEqual(['research-a', 'research-b']);
		expect(document.querySelectorAll('.ct-drag-ghost__item')).toHaveLength(0);
		expect(split.hasAttribute('data-drag-source')).toBe(true);
		expect(researchA.closest('.ct-vertical-tab-wrap')?.hasAttribute('data-drag-source')).toBe(
			false,
		);
		expect(researchB.closest('.ct-vertical-tab-wrap')?.hasAttribute('data-drag-source')).toBe(
			false,
		);
		fireEvent.pointerUp(window, {
			pointerId: 15,
			clientX: 60,
			clientY: 210,
		});
		expect(
			model
				.getSnapshot()
				.tabs.filter((tab) => tab.split === 'research-split')
				.map((tab) => tab.id),
		).toEqual(['research-a', 'research-b']);
		expect(
			model
				.getSnapshot()
				.tabs.slice(-2)
				.map((tab) => tab.id),
		).toEqual(['research-a', 'research-b']);
	});
	it('drags a normal vertical tab as the real tab view instead of a generic card', () => {
		const model = createModel();
		model.selectTab('loose');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(loose, 12, 216, 140, 30);
		fireEvent.pointerDown(loose, {
			button: 0,
			pointerId: 151,
			clientX: 90,
			clientY: 150,
		});
		fireEvent.pointerMove(window, {
			pointerId: 151,
			clientX: 90,
			clientY: 210,
		});
		const draggedTab = document.querySelector<HTMLElement>(
			'.ct-drag-ghost__tab-view .ct-tab[data-tab-id="loose"]',
		);
		expect(draggedTab).not.toBeNull();
		expect(document.querySelector('.ct-drag-ghost__favicon')).toBeNull();
		expect(
			document
				.querySelector<HTMLElement>('.ct-vertical-scroll__content [data-tab-id="loose"]')
				?.closest('.ct-vertical-tab-wrap')
				?.hasAttribute('data-drag-source'),
		).toBe(true);
		fireEvent.pointerUp(window, {
			pointerId: 151,
			clientX: 90,
			clientY: 210,
		});
	});
	it('animates non-source vertical views from their source-relative positions', () => {
		const model = new TabModel({
			tabs: [
				{
					id: 'a',
					title: 'A',
					pinned: false,
					group: null,
					split: null,
				},
				{
					id: 'b',
					title: 'B',
					pinned: false,
					group: null,
					split: null,
				},
				{
					id: 'c',
					title: 'C',
					pinned: false,
					group: null,
					split: null,
				},
			],
			activeTabId: 'c',
		});
		model.setSelection(['a', 'c'], 'c');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const a = screen.getByRole('tab', {
			name: /^A/,
		});
		const b = screen.getByRole('tab', {
			name: /^B/,
		});
		const c = screen.getByRole('tab', {
			name: /^C/,
		});
		setRect(a, 12, 216, 100, 30);
		setRect(b, 12, 216, 132, 30);
		setRect(c, 12, 216, 164, 30);
		fireEvent.pointerDown(c, {
			button: 0,
			pointerId: 152,
			clientX: 90,
			clientY: 179,
		});
		fireEvent.pointerMove(window, {
			pointerId: 152,
			clientX: 90,
			clientY: 220,
		});
		const draggedA = document.querySelector<HTMLElement>(
			'.ct-drag-ghost__tab-view:has([data-tab-id="a"])',
		);
		const draggedC = document.querySelector<HTMLElement>(
			'.ct-drag-ghost__tab-view:has([data-tab-id="c"])',
		);
		expect(draggedA?.hasAttribute('data-compacting')).toBe(true);
		expect(draggedA?.style.getPropertyValue('--ct-drag-initial-offset-y')).toBe('-32px');
		expect(draggedC?.hasAttribute('data-source')).toBe(true);
		expect(draggedC?.hasAttribute('data-compacting')).toBe(false);
		fireEvent.pointerUp(window, {
			pointerId: 152,
			clientX: 90,
			clientY: 220,
		});
	});
	it('preserves collection order when an ungrouped tab precedes a complete vertical group', () => {
		const model = new TabModel({
			tabs: [
				{
					id: 'loose',
					title: 'Loose',
					pinned: false,
					group: null,
					split: null,
				},
				{
					id: 'ga',
					title: 'Group A',
					pinned: false,
					group: 'g',
					split: null,
				},
				{
					id: 'gb',
					title: 'Group B',
					pinned: false,
					group: 'g',
					split: null,
				},
			],
			groups: [
				{
					id: 'g',
					title: 'Group',
					color: 'blue',
					collapsed: false,
				},
			],
			activeTabId: 'gb',
		});
		model.setSelection(['loose', 'ga', 'gb'], 'gb');
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		const ga = screen.getByRole('tab', {
			name: /^Group A/,
		});
		const gb = screen.getByRole('tab', {
			name: /^Group B/,
		});
		const groupView = requireDefined(
			screen
				.getByRole('tablist', {
					name: /^Group, 2 tabs/,
				})
				.closest<HTMLElement>('.ct-vertical-group'),
		);
		setRect(loose, 12, 216, 100, 30);
		setRect(groupView, 0, 216, 132, 100);
		setRect(ga, 12, 216, 168, 30);
		setRect(gb, 12, 216, 200, 30);
		fireEvent.pointerDown(gb, {
			button: 0,
			pointerId: 153,
			clientX: 90,
			clientY: 215,
		});
		fireEvent.pointerMove(window, {
			pointerId: 153,
			clientX: 90,
			clientY: 260,
		});
		const ghost = requireDefined(document.querySelector<HTMLElement>('.ct-drag-ghost'));
		const views = [...ghost.children].filter(
			(child): child is HTMLElement =>
				child instanceof HTMLElement &&
				(child.classList.contains('ct-drag-ghost__tab-view') ||
					child.classList.contains('ct-drag-ghost__group-view')),
		);
		expect(
			views.map((view) => {
				if (view.querySelector('[data-tab-id="loose"]')) return 'loose';
				if (view.querySelector('[data-group-id="g"]')) return 'group:g';
				return 'unknown';
			}),
		).toEqual(['loose', 'group:g']);
		fireEvent.pointerUp(window, {
			pointerId: 153,
			clientX: 90,
			clientY: 260,
		});
	});
	it('uses a squashed two-axis drag target in the expanded vertical pinned grid', () => {
		const model = new TabModel({
			tabs: [
				{
					id: 'p1',
					title: 'P1',
					pinned: true,
					group: null,
					split: null,
				},
				{
					id: 'p2',
					title: 'P2',
					pinned: true,
					group: null,
					split: null,
				},
				{
					id: 'p3',
					title: 'P3',
					pinned: true,
					group: null,
					split: null,
				},
				{
					id: 'p4',
					title: 'P4',
					pinned: true,
					group: null,
					split: null,
				},
				{
					id: 'loose',
					title: 'Loose',
					pinned: false,
					group: null,
					split: null,
				},
			],
			activeTabId: 'p4',
		});
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} />);
		const strip = screen.getByRole('tablist', {
			name: 'Browser tabs',
		});
		setRect(strip, 0, 240, 0, 500);
		const positions = [
			[12, 10],
			[116, 10],
			[12, 46],
			[116, 46],
		] as const;
		['P1', 'P2', 'P3', 'P4'].forEach((name, index) => {
			setRect(
				screen.getByRole('tab', {
					name: new RegExp(`^${name}`),
				}),
				requireDefined(positions[index])[0],
				100,
				requireDefined(positions[index])[1],
				30,
			);
		});
		const p4 = screen.getByRole('tab', {
			name: /^P4/,
		});
		fireEvent.pointerDown(p4, {
			button: 0,
			pointerId: 16,
			clientX: 160,
			clientY: 61,
		});
		fireEvent.pointerMove(window, {
			pointerId: 16,
			clientX: 20,
			clientY: 20,
		});
		expect(document.querySelector('.ct-drag-ghost')?.getAttribute('data-layout')).toBe('squash');
		expect(
			model
				.getSnapshot()
				.tabs.slice(0, 4)
				.map((tab) => tab.id),
		).toEqual(['p4', 'p1', 'p2', 'p3']);
		fireEvent.pointerUp(window, {
			pointerId: 16,
			clientX: 20,
			clientY: 20,
		});
	});
	it('replaces a tab when a URL is dropped over its center', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const strip = screen.getByRole('tablist', {
			name: 'Browser tabs',
		});
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(strip, 0, 600);
		setRect(loose, 300, 100);
		const dataTransfer = urlDataTransfer('https://www.chromium.org/');
		fireUrlDrag('dragover', loose, dataTransfer, 350, 20);
		expect(document.querySelector('.ct-drop-arrow')).toBeTruthy();
		fireUrlDrag('drop', loose, dataTransfer, 350, 20);
		const updated = model.getSnapshot().tabs.find((tab) => tab.id === 'loose');
		expect(updated).toMatchObject({
			title: 'www.chromium.org',
			url: 'https://www.chromium.org/',
		});
		expect(model.getSnapshot().activeTabId).toBe('loose');
		expect(document.querySelector('.ct-drop-arrow')).toBeNull();
	});
	it('spring-loads a URL-dragged tab after 500ms and cancels over an insertion edge', () => {
		vi.useFakeTimers();
		const model = createModel();
		render(<TabStrip model={model} />);
		const strip = screen.getByRole('tablist', {
			name: 'Browser tabs',
		});
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(strip, 0, 600);
		setRect(loose, 300, 100);
		const dataTransfer = urlDataTransfer('https://example.com/hovered');
		fireUrlDrag('dragover', loose, dataTransfer, 350, 20);
		act(() => vi.advanceTimersByTime(499));
		expect(model.getSnapshot().activeTabId).toBe('research-a');
		act(() => vi.advanceTimersByTime(1));
		expect(model.getSnapshot().activeTabId).toBe('loose');
		act(() => model.selectTab('research-a'));
		fireUrlDrag('dragover', loose, dataTransfer, 350, 20);
		act(() => vi.advanceTimersByTime(300));
		fireUrlDrag('dragover', loose, dataTransfer, 305, 20);
		act(() => vi.advanceTimersByTime(500));
		expect(model.getSnapshot().activeTabId).toBe('research-a');
	});
	it('inserts a dropped URL inside an expanded group at its leading edge', () => {
		const model = createModel();
		render(<TabStrip model={model} />);
		const strip = screen.getByRole('tablist', {
			name: 'Browser tabs',
		});
		const researchA = screen.getByRole('tab', {
			name: /^Research A/,
		});
		setRect(strip, 0, 600);
		setRect(researchA, 100, 100);
		const dataTransfer = urlDataTransfer('https://example.com/article');
		fireUrlDrag('dragover', researchA, dataTransfer, 105, 20);
		fireUrlDrag('drop', researchA, dataTransfer, 105, 20);
		expect(
			model.getSnapshot().tabs.map((tab) => ({
				title: tab.title,
				group: tab.group,
			})),
		).toEqual([
			{
				title: 'Pinned',
				group: null,
			},
			{
				title: 'example.com',
				group: 'research',
			},
			{
				title: 'Research A',
				group: 'research',
			},
			{
				title: 'Research B',
				group: 'research',
			},
			{
				title: 'Loose',
				group: null,
			},
		]);
	});
	it('places URL drops before/inside/after vertical group headers using collapse semantics', () => {
		const model = createModel();
		model.selectTab('loose');
		model.updateGroup('research', {
			collapsed: true,
		});
		const onUrlDrop = vi.fn<() => void>();
		const { rerender } = render(
			<TabStrip model={model} orientation={TabStripOrientation.Vertical} onUrlDrop={onUrlDrop} />,
		);
		const strip = screen.getByRole('tablist', {
			name: 'Browser tabs',
		});
		setRect(strip, 0, 240, 0, 500);
		let group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*collapsed$/,
		});
		setRect(group, 12, 216, 100, 30);
		fireUrlDrag('dragover', group, urlDataTransfer('https://example.com/before'), 100, 102);
		expect(document.querySelector<HTMLElement>('.ct-drop-arrow')?.style.top).toBe('90px');
		fireUrlDrag('drop', group, urlDataTransfer('https://example.com/before'), 100, 102);
		expect(onUrlDrop).toHaveBeenLastCalledWith(
			'https://example.com/before',
			expect.objectContaining({
				index: 1,
			}),
		);
		fireUrlDrag('dragover', group, urlDataTransfer('https://example.com/after'), 100, 128);
		expect(document.querySelector<HTMLElement>('.ct-drop-arrow')?.style.top).toBe('120px');
		fireUrlDrag('drop', group, urlDataTransfer('https://example.com/after'), 100, 128);
		expect(onUrlDrop).toHaveBeenLastCalledWith(
			'https://example.com/after',
			expect.objectContaining({
				index: 3,
			}),
		);
		model.updateGroup('research', {
			collapsed: false,
		});
		rerender(
			<TabStrip model={model} orientation={TabStripOrientation.Vertical} onUrlDrop={onUrlDrop} />,
		);
		group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		setRect(group, 12, 216, 100, 30);
		fireUrlDrag('dragover', group, urlDataTransfer('https://example.com/inside'), 100, 128);
		expect(document.querySelector<HTMLElement>('.ct-drop-arrow')?.style.top).toBe('120px');
		fireUrlDrag('drop', group, urlDataTransfer('https://example.com/inside'), 100, 128);
		expect(onUrlDrop).toHaveBeenLastCalledWith(
			'https://example.com/inside',
			expect.objectContaining({
				index: 1,
				groupInclusion: 'include-in-group',
			}),
		);
	});
	it('mirrors horizontal group-header URL placement in RTL', () => {
		const model = createModel();
		model.selectTab('loose');
		model.updateGroup('research', {
			collapsed: true,
		});
		const onUrlDrop = vi.fn<() => void>();
		const { rerender } = render(
			<div dir='rtl'>
				<TabStrip model={model} onUrlDrop={onUrlDrop} />
			</div>,
		);
		const strip = screen.getByRole('tablist', {
			name: 'Browser tabs',
		});
		setRect(strip, 0, 600);
		let group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*collapsed$/,
		});
		setRect(group, 100, 60);
		fireUrlDrag('dragover', group, urlDataTransfer('https://example.com/before'), 158, 20);
		expect(document.querySelector<HTMLElement>('.ct-drop-arrow')?.style.left).toBe('150px');
		fireUrlDrag('drop', group, urlDataTransfer('https://example.com/before'), 158, 20);
		expect(onUrlDrop).toHaveBeenLastCalledWith(
			'https://example.com/before',
			expect.objectContaining({
				index: 1,
			}),
		);
		fireUrlDrag('dragover', group, urlDataTransfer('https://example.com/after'), 102, 20);
		expect(document.querySelector<HTMLElement>('.ct-drop-arrow')?.style.left).toBe('90px');
		fireUrlDrag('drop', group, urlDataTransfer('https://example.com/after'), 102, 20);
		expect(onUrlDrop).toHaveBeenLastCalledWith(
			'https://example.com/after',
			expect.objectContaining({
				index: 1,
				groupInclusion: 'include-in-group',
			}),
		);
		model.updateGroup('research', {
			collapsed: false,
		});
		rerender(
			<div dir='rtl'>
				<TabStrip model={model} onUrlDrop={onUrlDrop} />
			</div>,
		);
		group = screen.getByRole('tablist', {
			name: /^Research, 2 tabs.*expanded$/,
		});
		setRect(group, 100, 60);
		fireUrlDrag('dragover', group, urlDataTransfer('https://example.com/inside'), 102, 20);
		fireUrlDrag('drop', group, urlDataTransfer('https://example.com/inside'), 102, 20);
		expect(onUrlDrop).toHaveBeenLastCalledWith(
			'https://example.com/inside',
			expect.objectContaining({
				index: 1,
				groupInclusion: 'include-in-group',
			}),
		);
	});
	it('keeps a collapsed vertical strip expanded for the lifetime of an external URL drag', () => {
		const model = createModel();
		render(<TabStrip model={model} orientation={TabStripOrientation.Vertical} verticalCollapsed />);
		const strip = screen.getByRole('tablist', {
			name: 'Browser tabs',
		});
		const loose = screen.getByRole('tab', {
			name: /^Loose/,
		});
		setRect(strip, 0, 56, 0, 500);
		setRect(loose, 12, 32, 100, 30);
		fireUrlDrag('dragover', loose, urlDataTransfer('https://example.com/'), 28, 115);
		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(true);
		fireEvent.dragLeave(strip, {
			relatedTarget: null,
		});
		expect(strip.hasAttribute('data-expanded-on-hover')).toBe(false);
	});
	it('uses the x-axis for URL insertion in an expanded vertical pinned grid', () => {
		const model = new TabModel({
			tabs: [
				{
					id: 'p1',
					title: 'P1',
					pinned: true,
					group: null,
					split: null,
				},
				{
					id: 'p2',
					title: 'P2',
					pinned: true,
					group: null,
					split: null,
				},
				{
					id: 'loose',
					title: 'Loose',
					pinned: false,
					group: null,
					split: null,
				},
			],
		});
		const onUrlDrop = vi.fn<() => void>();
		render(
			<TabStrip model={model} orientation={TabStripOrientation.Vertical} onUrlDrop={onUrlDrop} />,
		);
		const strip = screen.getByRole('tablist', {
			name: 'Browser tabs',
		});
		const p1 = screen.getByRole('tab', {
			name: /^P1/,
		});
		setRect(strip, 0, 240, 0, 500);
		setRect(p1, 12, 100, 10, 30);
		const transfer = urlDataTransfer('https://example.com/pinned');
		fireUrlDrag('dragover', p1, transfer, 110, 25);
		fireUrlDrag('drop', p1, transfer, 110, 25);
		expect(onUrlDrop).toHaveBeenCalledWith(
			'https://example.com/pinned',
			expect.objectContaining({
				index: 1,
			}),
		);
	});
});
