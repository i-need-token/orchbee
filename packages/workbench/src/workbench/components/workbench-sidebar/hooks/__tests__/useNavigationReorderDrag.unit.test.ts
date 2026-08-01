// @vitest-environment jsdom

import { act, cleanup, fireEvent, renderHook } from '@testing-library/react';
import type { PointerEvent as ReactPointerEvent } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { useNavigationReorderDrag } from '../useNavigationReorderDrag';

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe('useNavigationReorderDrag', () => {
	it('keeps a singleton navigation list inert', () => {
		const reorderItem = vi.fn<(itemId: string, beforeItemId: string | null) => void>();
		const items = [item('item-1')];
		const rendered = renderHook(() => useNavigationReorderDrag(items, reorderItem));
		const row = document.createElement('div');

		act(() => rendered.result.current.onItemPointerDown('item-1', pointerEvent(row, row)));
		fireEvent.pointerMove(document, { clientX: 20, clientY: 20, pointerId: 1 });
		fireEvent.pointerUp(document, { pointerId: 1 });

		expect(rendered.result.current.draggingItemId).toBeNull();
		expect(reorderItem).not.toHaveBeenCalled();
	});

	it('ignores row controls and commits a mouse reorder only after drag activation', () => {
		const reorderItem = vi.fn<(itemId: string, beforeItemId: string | null) => void>();
		const items = [item('item-1'), item('item-2')];
		const rendered = renderHook(() => useNavigationReorderDrag(items, reorderItem));
		const list = navigationList();
		const secondRow = list.querySelector<HTMLDivElement>(
			'[data-navigation-reorder-id="item-2"] [data-navigation-reorder-row]',
		);
		expect(secondRow).not.toBeNull();
		if (secondRow === null) throw new Error('Expected the second navigation row.');
		const setPointerCapture = vi.fn<(pointerId: number) => void>(() => {
			throw new Error('pointer capture unavailable');
		});
		secondRow.setPointerCapture = setPointerCapture;
		rendered.result.current.listRef.current = list;
		const excludedControl = document.createElement('button');
		excludedControl.dataset['navigationReorderExcluded'] = 'true';
		secondRow.append(excludedControl);

		act(() =>
			rendered.result.current.onItemPointerDown('item-2', pointerEvent(secondRow, excludedControl)),
		);
		fireEvent.pointerMove(document, { clientX: 20, clientY: 1, pointerId: 1 });
		const outside = document.createElement('button');
		const outsideClick = vi.fn<() => void>();
		outside.addEventListener('click', outsideClick);
		document.body.append(outside);
		fireEvent.pointerUp(document, { clientX: 20, clientY: 1, pointerId: 1 });
		expect(rendered.result.current.draggingItemId).toBeNull();
		expect(reorderItem).not.toHaveBeenCalled();

		act(() =>
			rendered.result.current.onItemPointerDown('item-2', pointerEvent(secondRow, secondRow)),
		);
		expect(setPointerCapture).not.toHaveBeenCalled();
		fireEvent.pointerMove(document, { clientX: 20, clientY: 1, pointerId: 1 });

		expect(setPointerCapture).toHaveBeenCalledOnce();
		expect(setPointerCapture).toHaveBeenCalledWith(1);
		expect(rendered.result.current.draggingItemId).toBe('item-2');
		expect(rendered.result.current.displacementDirection).toBe('down');
		expect(rendered.result.current.displacedItemIds).toEqual(['item-1']);
		expect(
			secondRow.style.getPropertyValue('--ob-comp-workbench-navigation-reorder-translate-y'),
		).toBe('-59px');

		fireEvent.pointerUp(document, { pointerId: 1 });
		expect(rendered.result.current.draggingItemId).toBeNull();
		expect(
			secondRow.style.getPropertyValue('--ob-comp-workbench-navigation-reorder-translate-y'),
		).toBe('');
		expect(reorderItem).toHaveBeenCalledWith('item-2', 'item-1');
		fireEvent.click(outside);
		expect(outsideClick).not.toHaveBeenCalled();
		outside.remove();
	});

	it('keeps the dragged row under the pointer while scrolling near a sidebar edge', () => {
		let pendingFrame: FrameRequestCallback | null = null;
		vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
			pendingFrame = callback;
			return 1;
		});
		const reorderItem = vi.fn<(itemId: string, beforeItemId: string | null) => void>();
		const items = [item('item-1'), item('item-2')];
		const rendered = renderHook(() => useNavigationReorderDrag(items, reorderItem));
		const scrollElement = document.createElement('div');
		scrollElement.dataset['navigationReorderScroll'] = 'true';
		Object.defineProperties(scrollElement, {
			clientHeight: { configurable: true, value: 100 },
			scrollHeight: { configurable: true, value: 300 },
		});
		scrollElement.getBoundingClientRect = () => rectangle(0, 100);
		const list = navigationList();
		scrollElement.append(list);
		document.body.append(scrollElement);
		rendered.result.current.listRef.current = list;
		const firstRow = list.querySelector<HTMLDivElement>(
			'[data-navigation-reorder-id="item-1"] [data-navigation-reorder-row]',
		);
		expect(firstRow).not.toBeNull();
		if (firstRow === null) throw new Error('Expected the first navigation row.');

		act(() =>
			rendered.result.current.onItemPointerDown(
				'item-1',
				pointerEvent(firstRow, firstRow, { clientY: 20 }),
			),
		);
		fireEvent.pointerMove(document, { clientX: 10, clientY: 98, pointerId: 1 });
		expect(pendingFrame).not.toBeNull();
		if (pendingFrame === null) throw new Error('Expected an automatic-scroll frame.');
		act(() => pendingFrame?.(0));

		expect(scrollElement.scrollTop).toBeGreaterThan(0);
		expect(rendered.result.current.displacementDirection).toBe('up');
		expect(rendered.result.current.displacedItemIds).toEqual(['item-2']);
		expect(
			firstRow.style.getPropertyValue('--ob-comp-workbench-navigation-reorder-translate-y'),
		).toBe(`${98 - 20}px`);
	});

	it('squeezes the final sibling when the dragged row midpoint crosses its midpoint', () => {
		const reorderItem = vi.fn<(itemId: string, beforeItemId: string | null) => void>();
		const items = [item('item-1'), item('item-2')];
		const rendered = renderHook(() => useNavigationReorderDrag(items, reorderItem));
		const list = navigationList();
		rendered.result.current.listRef.current = list;
		const firstRow = list.querySelector<HTMLDivElement>(
			'[data-navigation-reorder-id="item-1"] [data-navigation-reorder-row]',
		);
		expect(firstRow).not.toBeNull();
		if (firstRow === null) throw new Error('Expected the first navigation row.');

		act(() =>
			rendered.result.current.onItemPointerDown(
				'item-1',
				pointerEvent(firstRow, firstRow, { clientY: 1 }),
			),
		);
		fireEvent.pointerMove(document, { clientX: 10, clientY: 40, pointerId: 1 });
		expect(rendered.result.current.displacedItemIds).toEqual([]);

		fireEvent.pointerMove(document, { clientX: 10, clientY: 41, pointerId: 1 });
		expect(rendered.result.current.displacementDirection).toBe('up');
		expect(rendered.result.current.displacedItemIds).toEqual(['item-2']);

		fireEvent.pointerUp(document, { clientX: 10, clientY: 41, pointerId: 1 });
		expect(reorderItem).toHaveBeenCalledWith('item-1', null);
	});

	it('commits the visible squeeze when pointerup jitters back across a midpoint', () => {
		const reorderItem = vi.fn<(itemId: string, beforeItemId: string | null) => void>();
		const items = [item('item-1'), item('item-2')];
		const rendered = renderHook(() => useNavigationReorderDrag(items, reorderItem));
		const list = navigationList();
		rendered.result.current.listRef.current = list;
		const secondRow = list.querySelector<HTMLDivElement>(
			'[data-navigation-reorder-id="item-2"] [data-navigation-reorder-row]',
		);
		expect(secondRow).not.toBeNull();
		if (secondRow === null) throw new Error('Expected the second navigation row.');

		act(() =>
			rendered.result.current.onItemPointerDown(
				'item-2',
				pointerEvent(secondRow, secondRow, { clientY: 60 }),
			),
		);
		fireEvent.pointerMove(document, { clientX: 10, clientY: 19, pointerId: 1 });
		expect(rendered.result.current.displacementDirection).toBe('down');
		expect(rendered.result.current.displacedItemIds).toEqual(['item-1']);

		fireEvent.pointerUp(document, { clientX: 10, clientY: 21, pointerId: 1 });
		expect(reorderItem).toHaveBeenCalledWith('item-2', 'item-1');
	});

	it('cancels an invalidated drag and suppresses its document-level trailing click', () => {
		const reorderItem = vi.fn<(itemId: string, beforeItemId: string | null) => void>();
		const rendered = renderHook(
			({ currentItems }) => useNavigationReorderDrag(currentItems, reorderItem),
			{ initialProps: { currentItems: [item('item-1'), item('item-2')] } },
		);
		const list = navigationList();
		rendered.result.current.listRef.current = list;
		const secondRow = list.querySelector<HTMLDivElement>(
			'[data-navigation-reorder-id="item-2"] [data-navigation-reorder-row]',
		);
		expect(secondRow).not.toBeNull();
		if (secondRow === null) throw new Error('Expected the second navigation row.');

		act(() =>
			rendered.result.current.onItemPointerDown('item-2', pointerEvent(secondRow, secondRow)),
		);
		fireEvent.pointerMove(document, { clientX: 10, clientY: 1, pointerId: 1 });
		expect(rendered.result.current.draggingItemId).toBe('item-2');

		rendered.rerender({ currentItems: [item('item-2')] });
		expect(rendered.result.current.draggingItemId).toBeNull();
		expect(reorderItem).not.toHaveBeenCalled();
		const outside = document.createElement('button');
		const outsideClick = vi.fn<() => void>();
		outside.addEventListener('click', outsideClick);
		document.body.append(outside);
		fireEvent.click(outside);
		expect(outsideClick).not.toHaveBeenCalled();
		outside.remove();
	});

	it('suppresses a trailing click when an active drag unmounts', () => {
		const reorderItem = vi.fn<(itemId: string, beforeItemId: string | null) => void>();
		const rendered = renderHook(() =>
			useNavigationReorderDrag([item('item-1'), item('item-2')], reorderItem),
		);
		const list = navigationList();
		rendered.result.current.listRef.current = list;
		const secondRow = list.querySelector<HTMLDivElement>(
			'[data-navigation-reorder-id="item-2"] [data-navigation-reorder-row]',
		);
		expect(secondRow).not.toBeNull();
		if (secondRow === null) throw new Error('Expected the second navigation row.');

		act(() =>
			rendered.result.current.onItemPointerDown('item-2', pointerEvent(secondRow, secondRow)),
		);
		fireEvent.pointerMove(document, { clientX: 10, clientY: 1, pointerId: 1 });
		expect(rendered.result.current.draggingItemId).toBe('item-2');
		rendered.unmount();

		const outside = document.createElement('button');
		const outsideClick = vi.fn<() => void>();
		outside.addEventListener('click', outsideClick);
		document.body.append(outside);
		fireEvent.click(outside);
		expect(outsideClick).not.toHaveBeenCalled();
		expect(reorderItem).not.toHaveBeenCalled();
		outside.remove();
	});
});

function pointerEvent(
	currentTarget: HTMLDivElement,
	target: EventTarget,
	position: { readonly clientY?: number } = {},
): ReactPointerEvent<HTMLDivElement> {
	return {
		button: 0,
		clientX: 10,
		clientY: position.clientY ?? 60,
		currentTarget,
		pointerId: 1,
		pointerType: 'mouse',
		target,
	} as ReactPointerEvent<HTMLDivElement>;
}

function item(itemId: string): { readonly id: string } {
	return { id: itemId };
}

function navigationList(): HTMLUListElement {
	const list = document.createElement('ul');
	for (const [index, itemId] of ['item-1', 'item-2'].entries()) {
		const listItem = document.createElement('li');
		listItem.dataset['navigationReorderId'] = itemId;
		const row = document.createElement('div');
		row.dataset['navigationReorderRow'] = 'true';
		row.getBoundingClientRect = () => rectangle(index * 40);
		listItem.append(row);
		list.append(listItem);
	}
	return list;
}

function rectangle(top: number, height = 40): DOMRect {
	return {
		bottom: top + height,
		height,
		left: 0,
		right: 240,
		top,
		width: 240,
		x: 0,
		y: top,
		toJSON: () => ({}),
	};
}
