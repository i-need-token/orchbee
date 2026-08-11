import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import {
	createWorkbenchPreviewTerminalPresentationComposition,
	terminalPresentationId,
	WorkbenchProjectWorkspaceRuntime,
} from '@orchbee/workbench';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { WebApplication } from '../WebApplication';

describe('WebApplication', () => {
	it('renders the Workbench with the initial terminal tabs', () => {
		const actionRuntime = new ActionRuntime();
		let presentationSequence = 0;
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/workspace',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
				terminalPresentationId('web:standalone:initial'),
				() => terminalPresentationId(`web:test:dynamic:${(presentationSequence += 1)}`),
			),
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
						layoutStorage: {
							getItem: () => '{"width":320}',
							setItem: () => undefined,
						},
						projectWorkspaceRuntime,
						shortcutPlatform: 'apple',
					}),
				),
			),
		);

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('flex-basis:320px');
		expect(markup).toContain('role="tablist"');
		expect(markup).toContain('Terminal 1');
		expect(markup).not.toContain('Terminal 4');
		expect(markup).toContain('aria-label="Create Project"');
		expect(markup).not.toContain('data-slot="workbench-sidebar-project"');
		expect(markup).not.toContain('data-slot="workbench-sidebar-workspace"');
		expect(markup).not.toContain('Local shell');
		expect(markup).not.toContain('本地 Shell');
		actionRuntime.dispose();
	});
});
