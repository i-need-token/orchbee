import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import { useWorkbenchSidebarShortcut } from '../useWorkbenchSidebarShortcut';

interface WorkbenchSidebarShortcutHarnessProps {
	readonly onToggle: () => void;
}

function WorkbenchSidebarShortcutHarness({ onToggle }: WorkbenchSidebarShortcutHarnessProps) {
	useWorkbenchSidebarShortcut('apple', onToggle);
	return null;
}

describe('useWorkbenchSidebarShortcut', () => {
	it('can mount during server rendering without invoking the action', () => {
		const onToggle = vi.fn<() => void>();

		expect(renderToStaticMarkup(createElement(WorkbenchSidebarShortcutHarness, { onToggle }))).toBe(
			'',
		);
		expect(onToggle).not.toHaveBeenCalled();
	});
});
