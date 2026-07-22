import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { WorkbenchTabGroupHeader } from '../WorkbenchTabGroupHeader';

const COMMON_PROPERTIES = {
	closeLabel: 'Close Group',
	collapseLabel: 'Collapse Group A',
	colorChoices: [{ label: 'Blue', value: 'blue' }] as const,
	colorLabel: 'Group Color',
	deleteLabel: 'Delete Group',
	dragProperties: {
		'data-dragging': undefined,
		onClickCapture: () => undefined,
		onLostPointerCapture: () => undefined,
		onPointerCancel: () => undefined,
		onPointerDown: () => undefined,
		onPointerMove: () => undefined,
		onPointerUp: () => undefined,
		reorderable: true,
	},
	newTabLabel: 'New Tab in Group',
	onClose: undefined,
	onCollapseToggle: undefined,
	onColorChange: undefined,
	onDelete: undefined,
	onNewTab: undefined,
	onRename: undefined,
	onUngroup: undefined,
	renameLabel: 'Rename Group',
	ungroupLabel: 'Ungroup Tabs',
} as const;

describe('WorkbenchTabGroupHeader', () => {
	it('marks an expanded group', () => {
		const markup = renderToStaticMarkup(
			createElement(WorkbenchTabGroupHeader, {
				...COMMON_PROPERTIES,
				group: { color: 'blue', id: 'group-a', label: 'Group A' },
			}),
		);

		expect(markup).toContain('data-workbench-tab-group-expanded="true"');
		expect(markup).toContain('data-workbench-tab-group-reorderable="true"');
		expect(markup).toContain('aria-expanded="true"');
	});

	it('removes the expanded marker when the group is collapsed', () => {
		const markup = renderToStaticMarkup(
			createElement(WorkbenchTabGroupHeader, {
				...COMMON_PROPERTIES,
				collapseLabel: 'Expand Group A',
				group: { color: 'blue', id: 'group-a', isCollapsed: true, label: 'Group A' },
			}),
		);

		expect(markup).not.toContain('data-workbench-tab-group-expanded');
		expect(markup).toContain('aria-expanded="false"');
	});
});
