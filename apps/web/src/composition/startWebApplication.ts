import { ActionRuntime } from '@orchbee/actions';
import {
	createOrchbeeIntl,
	getOrchbeeTextDirection,
	OrchbeeI18nProvider,
	resolveOrchbeeLocale,
	workbenchTabMessages,
} from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import {
	createWorkbenchPreviewTerminalPresentationComposition,
	terminalPresentationId,
	WorkbenchProjectWorkspaceRuntime,
	type WorkbenchLayoutStorage,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import { WebApplication } from '../web-application';

interface WebApplicationSession {
	dispose(): void;
}

export function startWebApplication(
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	layoutStorage: WorkbenchLayoutStorage,
): WebApplicationSession {
	if (rootElement === null) throw new Error('The Orchbee web root element is missing.');
	const locale = resolveOrchbeeLocale(preferredLocales);
	const intl = createOrchbeeIntl(locale);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);
	const actionRuntime = new ActionRuntime();
	const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/workspace',
		standaloneTerminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle, {
			number: 1,
		}),
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('web:standalone:initial'),
		),
	});
	let reactRoot: Root | undefined = createRoot(rootElement);

	reactRoot.render(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(
				TooltipProvider,
				undefined,
				createElement(WebApplication, {
					actionRuntime,
					layoutStorage,
					projectWorkspaceRuntime,
					shortcutPlatform,
				}),
			),
		),
	);
	return {
		dispose: () => {
			reactRoot?.unmount();
			reactRoot = undefined;
			actionRuntime.dispose();
		},
	};
}
