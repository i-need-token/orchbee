import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import type { WorkbenchLayoutStorage } from '../../../workbench-layout-storage';
import { useWorkbenchShellLayout } from '../useWorkbenchShellLayout';

interface WorkbenchShellLayoutHarnessProps {
	readonly isUserInteraction: boolean;
	readonly storage: WorkbenchLayoutStorage;
}

function WorkbenchShellLayoutHarness({
	isUserInteraction,
	storage,
}: WorkbenchShellLayoutHarnessProps) {
	const layout = useWorkbenchShellLayout(storage);
	layout.onLeftPanelResize({ inPixels: 318.4 });
	layout.onLayoutChanged({}, { isUserInteraction });
	return layout.defaultLeftPanelSize;
}

describe('useWorkbenchShellLayout', () => {
	it('restores the saved pixel width without writing during initialization', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();
		const markup = renderToStaticMarkup(
			createElement(WorkbenchShellLayoutHarness, {
				isUserInteraction: false,
				storage: { getItem: () => '{"width":320}', setItem },
			}),
		);

		expect(markup).toContain('320px');
		expect(setItem).not.toHaveBeenCalled();
	});

	it('writes the latest width after a completed user interaction', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();
		renderToStaticMarkup(
			createElement(WorkbenchShellLayoutHarness, {
				isUserInteraction: true,
				storage: { getItem: () => null, setItem },
			}),
		);

		expect(setItem).toHaveBeenCalledWith('orchbee:workbench:left-panel-width:v1', '{"width":318}');
	});
});
