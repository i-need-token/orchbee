import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import { useWorkbenchShortcuts } from '../useWorkbenchShortcuts';

interface WorkbenchShortcutsHarnessProps {
	readonly onCloseTab: () => void;
	readonly onNewTerminal: () => void;
	readonly onSearch: () => void;
	readonly onSettings: () => void;
	readonly onToggleSidebar: () => void;
}

function WorkbenchShortcutsHarness({
	onCloseTab,
	onNewTerminal,
	onSearch,
	onSettings,
	onToggleSidebar,
}: WorkbenchShortcutsHarnessProps) {
	useWorkbenchShortcuts('apple', onCloseTab, onNewTerminal, onSearch, onSettings, onToggleSidebar);
	return null;
}

describe('useWorkbenchShortcuts', () => {
	it('can mount during server rendering without invoking an action', () => {
		const onCloseTab = vi.fn<() => void>();
		const onNewTerminal = vi.fn<() => void>();
		const onSearch = vi.fn<() => void>();
		const onSettings = vi.fn<() => void>();
		const onToggleSidebar = vi.fn<() => void>();

		expect(
			renderToStaticMarkup(
				createElement(WorkbenchShortcutsHarness, {
					onCloseTab,
					onNewTerminal,
					onSearch,
					onSettings,
					onToggleSidebar,
				}),
			),
		).toBe('');
		expect(onCloseTab).not.toHaveBeenCalled();
		expect(onNewTerminal).not.toHaveBeenCalled();
		expect(onSearch).not.toHaveBeenCalled();
		expect(onSettings).not.toHaveBeenCalled();
		expect(onToggleSidebar).not.toHaveBeenCalled();
	});
});
