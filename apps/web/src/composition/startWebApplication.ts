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
import { RouterProvider } from '@tanstack/react-router';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import { createWebApplicationRouting } from '../application-routing';
import { createBrowserRandomIdentifier } from '../browser-random-identifier';
import { KeyboardShortcutSettingsController } from '../keyboard-shortcuts';
import { TerminalInteractionSettingsController } from '../terminal-interaction-settings';
import { TerminalSettingsController } from '../terminal-settings';
import {
	createBrowserTerminalNotificationCapability,
	TerminalFeedbackSettingsController,
	UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY,
} from '../terminal-feedback-settings';

interface WebApplicationSession {
	dispose(): void;
}

export function startWebApplication(
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dataset' | 'dir' | 'lang'>,
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
	const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
		platform: shortcutPlatform,
		storage: layoutStorage,
	});
	const terminalSettingsController = new TerminalSettingsController({
		storage: layoutStorage,
		themeTarget: documentElement,
	});
	const terminalInteractionSettingsController = new TerminalInteractionSettingsController(
		layoutStorage,
	);
	const targetWindow = rootElement.ownerDocument?.defaultView;
	const terminalFeedbackSettingsController = new TerminalFeedbackSettingsController({
		notificationCapability:
			targetWindow === null || targetWindow === undefined
				? UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY
				: createBrowserTerminalNotificationCapability(targetWindow),
		storage: layoutStorage,
	});
	const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/workspace',
		standaloneTerminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle),
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('web:standalone:initial'),
			() => terminalPresentationId(`web:preview:${createBrowserRandomIdentifier()}`),
		),
	});
	const routing = createWebApplicationRouting({
		actionRuntime,
		keyboardShortcutSettingsController,
		layoutStorage,
		projectWorkspaceRuntime,
		shortcutPlatform,
		terminalSettingsController,
		terminalInteractionSettingsController,
		terminalFeedbackSettingsController,
	});
	let reactRoot: Root | undefined = createRoot(rootElement);

	reactRoot.render(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(
				TooltipProvider,
				undefined,
				createElement(RouterProvider, { router: routing.router }),
			),
		),
	);
	return {
		dispose: () => {
			reactRoot?.unmount();
			reactRoot = undefined;
			routing.dispose();
			actionRuntime.dispose();
			keyboardShortcutSettingsController.dispose();
			terminalSettingsController.dispose();
			terminalInteractionSettingsController.dispose();
			terminalFeedbackSettingsController.dispose();
		},
	};
}
