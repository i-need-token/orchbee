import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { TerminalSettingsController } from '../../terminal-settings';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { ApplicationLocaleController } from '../../application-locale';
import {
	createSnapshotRuntime,
	workbenchSnapshot,
} from '../../__tests__/server-workbench-snapshot';
import { WebApplication } from '../WebApplication';
import { inertWorkbenchAuthority } from '../../__tests__/inertWorkbenchAuthority';
import { memoryUserPreferences } from '../../__tests__/memoryUserPreferences';

describe('WebApplication', () => {
	it('renders the Workbench before the Server holds any Workspace', () => {
		const actionRuntime = new ActionRuntime();
		const projectWorkspaceRuntime = createSnapshotRuntime(
			workbenchSnapshot(1, { workspaces: [] }, []),
		);
		const terminalSettingsController = new TerminalSettingsController({
			storage: { getItem: () => null, setItem: () => undefined },
			themeTarget: { dataset: {} },
			userPreferences: memoryUserPreferences(),
		});
		const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage: { getItem: () => null, setItem: () => undefined },
		});
		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(
					TooltipProvider,
					undefined,
					createElement(WebApplication, {
						actionRuntime,
						applicationLocaleController: createApplicationLocaleController(),
						authority: inertAuthority,
						keyboardShortcutSettingsController,
						layoutStorage: {
							getItem: () => '{"width":320}',
							setItem: () => undefined,
						},
						projectWorkspaceRuntime,
						shortcutPlatform: 'apple',
						terminalSettingsController,
					}),
				),
			),
		);

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('flex-basis:320px');
		// As in the TUI, the main area stays blank: no Tab Strip and no statement.
		expect(markup).not.toContain('role="tablist"');
		expect(markup).not.toContain('data-slot="workbench-empty-workspace"');
		expect(markup).toContain('aria-label="Create Project"');
		expect(markup).not.toContain('data-slot="workbench-sidebar-project"');
		expect(markup).not.toContain('data-slot="workbench-sidebar-workspace"');
		actionRuntime.dispose();
		keyboardShortcutSettingsController.dispose();
		terminalSettingsController.dispose();
	});
});

const inertAuthority = inertWorkbenchAuthority();

function createApplicationLocaleController(): ApplicationLocaleController {
	return new ApplicationLocaleController({
		documentElement: { dir: '', lang: '' },
		preferredLocales: ['en-US'],
		userPreferences: memoryUserPreferences(),
	});
}
