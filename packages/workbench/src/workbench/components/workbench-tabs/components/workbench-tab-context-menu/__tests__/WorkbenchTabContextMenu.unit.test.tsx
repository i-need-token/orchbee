import { createElement, type ReactElement, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

function Container({ children }: { readonly children?: ReactNode }) {
	return createElement('div', undefined, children);
}

function Action({
	children,
	closeOnClick,
}: {
	readonly children?: ReactNode;
	readonly closeOnClick?: boolean;
}) {
	return createElement(
		'div',
		{ 'data-workbench-test-close-on-click': closeOnClick ? true : undefined },
		children,
	);
}

function Trigger({ render }: { readonly render: ReactElement }) {
	return render;
}

vi.mock('@orchbee/ui/components', () => {
	return {
		ContextMenu: Container,
		ContextMenuCheckboxItem: Action,
		ContextMenuContent: Container,
		ContextMenuGroup: Container,
		ContextMenuItem: Action,
		ContextMenuLabel: Container,
		ContextMenuRadioGroup: Container,
		ContextMenuRadioItem: Action,
		ContextMenuSeparator: Container,
		ContextMenuShortcut: Container,
		ContextMenuSub: Container,
		ContextMenuSubContent: Container,
		ContextMenuSubTrigger: Container,
		ContextMenuTrigger: Trigger,
	};
});

import { WorkbenchTabGroupContextMenu } from '../../workbench-tab-group-context-menu/WorkbenchTabGroupContextMenu';
import { WorkbenchTabContextMenu } from '../WorkbenchTabContextMenu';

const CLOSE_ON_CLICK_MARKER = 'data-workbench-test-close-on-click="true"';

describe('Workbench context menus', () => {
	it('closes the tab menu after every final action', () => {
		const markup = renderToStaticMarkup(
			createElement(WorkbenchTabContextMenu, {
				addToGroupLabel: 'Add to group',
				addToNewGroupLabel: 'Add to new group',
				closeAllLabel: 'Close all',
				closeLabel: 'Close',
				closeOthersLabel: 'Close others',
				closeRightLabel: 'Close right',
				closeShortcutLabel: 'Command W',
				color: 'default',
				colorChoices: [{ label: 'Default', value: 'default' }],
				currentGroupId: 'current-group',
				groupChoices: [
					{ color: 'blue', id: 'current-group', label: 'Current group' },
					{ color: 'purple', id: 'other-group', label: 'Other group' },
				],
				isPinned: false,
				onClose: () => undefined,
				onCloseAll: () => undefined,
				onCloseOthers: () => undefined,
				onCloseRight: () => undefined,
				onColorChange: () => undefined,
				onCreateGroup: () => undefined,
				onGroupAssign: () => undefined,
				onGroupRemove: () => undefined,
				onPinToggle: () => undefined,
				onRename: () => undefined,
				pinLabel: 'Pin',
				renameLabel: 'Rename',
				removeFromGroupLabel: 'Remove from group',
				tabColorLabel: 'Tab color',
				trigger: createElement('button'),
			}),
		);

		expect(markup.match(new RegExp(CLOSE_ON_CLICK_MARKER, 'g'))).toHaveLength(10);
	});

	it('closes the group menu after every final action', () => {
		const markup = renderToStaticMarkup(
			createElement(WorkbenchTabGroupContextMenu, {
				closeLabel: 'Close group',
				color: 'blue',
				colorChoices: [{ label: 'Blue', value: 'blue' }],
				colorLabel: 'Group color',
				deleteLabel: 'Delete group',
				newTabLabel: 'New tab',
				onClose: () => undefined,
				onColorChange: () => undefined,
				onDelete: () => undefined,
				onNewTab: () => undefined,
				onRename: () => undefined,
				onUngroup: () => undefined,
				renameLabel: 'Rename group',
				trigger: createElement('button'),
				ungroupLabel: 'Ungroup',
			}),
		);

		expect(markup.match(new RegExp(CLOSE_ON_CLICK_MARKER, 'g'))).toHaveLength(6);
	});
});
