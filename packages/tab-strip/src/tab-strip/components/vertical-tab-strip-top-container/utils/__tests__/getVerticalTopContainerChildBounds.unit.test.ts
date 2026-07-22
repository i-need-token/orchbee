import { describe, expect, it } from 'vitest';
import { calculateVerticalTopContainerLayout } from '../../../../../vertical';
import { getVerticalTopContainerChildBounds } from '../getVerticalTopContainerChildBounds';

describe('getVerticalTopContainerChildBounds', () => {
	it('projects a child bound and hides a missing child', () => {
		const layout = calculateVerticalTopContainerLayout({
			availableWidth: 240,
			collapseVisible: true,
			comboEndVisible: true,
		});
		const collapse = layout.children.find((child) => child.id === 'collapse');

		expect(getVerticalTopContainerChildBounds(layout, 'collapse')).toEqual(collapse?.bounds);
		expect(getVerticalTopContainerChildBounds(layout, 'unfocus')).toBeNull();
	});
});
