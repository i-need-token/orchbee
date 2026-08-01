import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { ActionRuntime } from '@orchbee/actions';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import type { WorkbenchLayoutStorage } from '../../../workbench-layout-storage';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { useWorkbench } from '../useWorkbench';

interface WorkbenchHarnessProps {
	readonly actionRuntime: ActionRuntime;
	readonly isUserInteraction: boolean;
	readonly storage: WorkbenchLayoutStorage;
}

function WorkbenchHarness({ actionRuntime, isUserInteraction, storage }: WorkbenchHarnessProps) {
	const workbench = useWorkbench({
		actionRuntime,
		onSettings: undefined,
		projectWorkspaceRuntime: new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
		}),
		shortcutPlatform: 'apple',
		storage,
	});
	workbench.onLeftPanelResize({ inPixels: 318.4 });
	workbench.onLayoutChanged({}, { isUserInteraction });
	return `${workbench.leftPanelSize} ${workbench.tabStripOrientation} ${workbench.sidebarToggleLabel} ${workbench.sidebarShortcutLabel} ${workbench.newTerminalShortcutLabel} ${workbench.searchShortcutLabel} ${workbench.settingsShortcutLabel}`;
}

function renderHarness(storage: WorkbenchLayoutStorage, isUserInteraction: boolean): string {
	return renderToStaticMarkup(
		createElement(
			OrchbeeI18nProvider,
			{ locale: 'en-US' },
			createElement(WorkbenchHarness, {
				actionRuntime: new ActionRuntime(),
				isUserInteraction,
				storage,
			}),
		),
	);
}

describe('useWorkbench', () => {
	it('prepares the restored layout, localized action, and platform shortcut', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();
		const markup = renderHarness(
			{
				getItem: (key) =>
					key === 'orchbee:workbench:left-panel-width:v1' ? '{"width":320}' : null,
				setItem,
			},
			false,
		);

		expect(markup).toContain('320px horizontal Hide Sidebar ⌘B ⌘N ⌘K ⌘,');
		expect(setItem).not.toHaveBeenCalled();
	});

	it('writes the latest width after a completed user interaction', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();

		renderHarness({ getItem: () => null, setItem }, true);

		expect(setItem).toHaveBeenCalledWith('orchbee:workbench:left-panel-width:v1', '{"width":318}');
	});
});
