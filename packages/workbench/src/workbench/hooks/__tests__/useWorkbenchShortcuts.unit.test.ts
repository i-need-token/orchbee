import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import { useWorkbenchShortcuts } from '../useWorkbenchShortcuts';

interface WorkbenchShortcutsHarnessProps {
	readonly onNewTerminal: () => void;
	readonly onSearch: () => void;
	readonly onSettings: () => void;
	readonly onToggleSidebar: () => void;
}

function WorkbenchShortcutsHarness({
	onNewTerminal,
	onSearch,
	onSettings,
	onToggleSidebar,
}: WorkbenchShortcutsHarnessProps) {
	useWorkbenchShortcuts('apple', onNewTerminal, onSearch, onSettings, onToggleSidebar);
	return null;
}

describe('useWorkbenchShortcuts', () => {
	it('can mount during server rendering without invoking an action', () => {
		const onNewTerminal = vi.fn<() => void>();
		const onSearch = vi.fn<() => void>();
		const onSettings = vi.fn<() => void>();
		const onToggleSidebar = vi.fn<() => void>();

		expect(
			renderToStaticMarkup(
				createElement(WorkbenchShortcutsHarness, {
					onNewTerminal,
					onSearch,
					onSettings,
					onToggleSidebar,
				}),
			),
		).toBe('');
		expect(onNewTerminal).not.toHaveBeenCalled();
		expect(onSearch).not.toHaveBeenCalled();
		expect(onSettings).not.toHaveBeenCalled();
		expect(onToggleSidebar).not.toHaveBeenCalled();
	});
});
