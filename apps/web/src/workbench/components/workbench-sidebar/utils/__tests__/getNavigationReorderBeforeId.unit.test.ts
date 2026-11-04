// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';

import { getNavigationReorderBeforeId } from '../getNavigationReorderBeforeId';

describe('getNavigationReorderBeforeId', () => {
	it('uses only non-source direct-row midpoints and treats the exact midpoint as after', () => {
		const list = document.createElement('ul');
		list.append(item('item-1', 0), item('item-2', 40), item('item-3', 80));

		expect(getNavigationReorderBeforeId(list, 'item-2', 19)).toBe('item-1');
		expect(getNavigationReorderBeforeId(list, 'item-2', 20)).toBe('item-3');
		expect(getNavigationReorderBeforeId(list, 'item-2', 100)).toBeNull();
	});

	it('ignores nested descendants and malformed children', () => {
		const list = document.createElement('ul');
		const first = item('item-1', 0);
		const nestedItem = document.createElement('div');
		nestedItem.dataset['navigationReorderId'] = 'nested-item';
		nestedItem.getBoundingClientRect = () => rectangle(10, 400);
		first.append(nestedItem);
		list.append(document.createElement('li'), first, item('item-2', 40));

		expect(getNavigationReorderBeforeId(list, 'item-1', 21)).toBe('item-2');
	});

	it('uses layout positions instead of squeeze transforms for stable midpoint crossings', () => {
		const list = document.createElement('ul');
		list.getBoundingClientRect = () => rectangle(100, 120);
		const first = item('item-1', 0);
		const second = item('item-2', 40);
		const secondRow = second.querySelector<HTMLElement>('[data-navigation-reorder-row]');
		expect(secondRow).not.toBeNull();
		if (secondRow === null) throw new Error('Expected a navigation reorder row.');
		secondRow.getBoundingClientRect = () => rectangle(220, 40);
		list.append(first, second);

		expect(getNavigationReorderBeforeId(list, 'item-1', 159)).toBe('item-2');
		expect(getNavigationReorderBeforeId(list, 'item-1', 160)).toBeNull();
	});

	it('projects row offsets correctly when the positioned offset parent is outside the list', () => {
		const project = document.createElement('div');
		const list = document.createElement('ul');
		const first = item('item-1', 42);
		const second = item('item-2', 82);
		Object.defineProperties(project, {
			offsetTop: { configurable: true, value: 100 },
		});
		Object.defineProperties(list, {
			offsetParent: { configurable: true, value: project },
			offsetTop: { configurable: true, value: 42 },
		});
		Object.defineProperties(first, {
			offsetParent: { configurable: true, value: project },
		});
		Object.defineProperties(second, {
			offsetParent: { configurable: true, value: project },
		});
		list.getBoundingClientRect = () => rectangle(142, 80);
		list.append(first, second);

		expect(getNavigationReorderBeforeId(list, 'item-1', 201)).toBe('item-2');
		expect(getNavigationReorderBeforeId(list, 'item-1', 202)).toBeNull();
	});
});

function item(itemId: string, top: number): HTMLLIElement {
	const element = document.createElement('li');
	element.dataset['navigationReorderId'] = itemId;
	Object.defineProperty(element, 'offsetTop', { configurable: true, value: top });
	const row = document.createElement('div');
	row.dataset['navigationReorderRow'] = 'true';
	Object.defineProperties(row, {
		offsetHeight: { configurable: true, value: 40 },
		offsetParent: { configurable: true, value: element },
		offsetTop: { configurable: true, value: 0 },
	});
	row.getBoundingClientRect = () => rectangle(top, 40);
	element.append(row);
	return element;
}

function rectangle(top: number, height: number): DOMRect {
	return {
		bottom: top + height,
		height,
		left: 0,
		right: 200,
		top,
		width: 200,
		x: 0,
		y: top,
		toJSON: () => ({}),
	};
}
