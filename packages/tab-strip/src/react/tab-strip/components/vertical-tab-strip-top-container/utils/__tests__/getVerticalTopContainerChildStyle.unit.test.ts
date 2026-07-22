import { describe, expect, it } from 'vitest';
import { calculateVerticalTopContainerLayout } from '../../../../../../vertical';
import { getVerticalTopContainerChildStyle } from '../getVerticalTopContainerChildStyle';

describe('getVerticalTopContainerChildStyle', () => {
	it('projects a child bound and hides a missing child', () => {
		const layout = calculateVerticalTopContainerLayout({
			availableWidth: 240,
			collapseVisible: true,
			comboEndVisible: true,
		});
		const collapse = layout.children.find((child) => child.id === 'collapse');

		expect(getVerticalTopContainerChildStyle(layout, 'collapse')).toEqual({
			left: collapse?.bounds.x,
			top: collapse?.bounds.y,
			width: collapse?.bounds.width,
			height: collapse?.bounds.height,
		});
		expect(getVerticalTopContainerChildStyle(layout, 'unfocus')).toEqual({ display: 'none' });
	});
});
