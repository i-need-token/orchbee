// @vitest-environment jsdom

import {
	ContextMenu,
	ContextMenuTrigger,
	DropdownMenu,
	DropdownMenuTrigger,
} from '@orchbee/ui/components';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchActionMenuEntry } from '../../../types/workbench-action-menu';
import { WorkbenchActionDropdownMenuContent } from '../../workbench-action-dropdown-menu-content/WorkbenchActionDropdownMenuContent';
import { WorkbenchActionContextMenuContent } from '../WorkbenchActionContextMenuContent';

const entries: readonly WorkbenchActionMenuEntry[] = [
	{
		icon: 'tabs',
		id: 'bound',
		kind: 'command',
		label: 'New tab',
		onSelect: vi.fn<() => void>(),
		shortcut: '⌘T',
	},
	{
		icon: 'terminal',
		id: 'unbound',
		kind: 'command',
		label: 'Split left',
		onSelect: vi.fn<() => void>(),
	},
];

afterEach(cleanup);

describe('Workbench Action menu shortcut projection', () => {
	it('renders only the effective shortcut in a context menu trailing slot', () => {
		const { container } = render(
			<ContextMenu>
				<ContextMenuTrigger render={<button type='button'>Open context menu</button>} />
				<WorkbenchActionContextMenuContent entries={entries} />
			</ContextMenu>,
		);

		fireEvent.contextMenu(screen.getByRole('button', { name: 'Open context menu' }), {
			clientX: 20,
			clientY: 20,
		});

		const shortcuts = container.ownerDocument.querySelectorAll(
			'[data-slot="context-menu-shortcut"]',
		);
		expect(shortcuts).toHaveLength(1);
		expect(shortcuts[0]?.textContent).toBe('⌘T');
	});

	it('renders only the effective shortcut in a dropdown trailing slot', () => {
		const { container } = render(
			<DropdownMenu>
				<DropdownMenuTrigger render={<button type='button'>Open dropdown</button>} />
				<WorkbenchActionDropdownMenuContent entries={entries} />
			</DropdownMenu>,
		);

		fireEvent.click(screen.getByRole('button', { name: 'Open dropdown' }));

		const shortcuts = container.ownerDocument.querySelectorAll(
			'[data-slot="dropdown-menu-shortcut"]',
		);
		expect(shortcuts).toHaveLength(1);
		expect(shortcuts[0]?.textContent).toBe('⌘T');
	});
});
