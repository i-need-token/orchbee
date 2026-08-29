import { browserAuthenticationMessages, createOrchbeeIntl } from '@orchbee/i18n';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import {
	BrowserAuthenticationClient,
	BrowserAuthenticationPage,
	BrowserAuthenticationController,
	resolveBrowserAuthenticationReturnTarget,
} from '../browser-authentication-page';
import { ApplicationLocaleController, type ApplicationLocaleStorage } from '../application-locale';
import { ApplicationLocaleProvider } from '../application-locale-provider';

export interface BrowserAuthenticationApplicationSession {
	dispose(): void;
}

export function startBrowserAuthenticationApplication(
	targetWindow: Window,
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	storage: ApplicationLocaleStorage,
): BrowserAuthenticationApplicationSession {
	if (rootElement === null) throw new Error('The Orchbee authentication root element is missing.');
	const applicationLocaleController = new ApplicationLocaleController({
		documentElement,
		preferredLocales,
		storage,
	});
	const updateDocumentTitle = (): void => {
		targetWindow.document.title = createOrchbeeIntl(
			applicationLocaleController.current(),
		).formatMessage(browserAuthenticationMessages.documentTitle);
	};
	updateDocumentTitle();
	const unsubscribeLocale = applicationLocaleController.subscribe(updateDocumentTitle);
	const controller = new BrowserAuthenticationController({
		client: new BrowserAuthenticationClient(targetWindow.fetch.bind(targetWindow)),
		navigate: (target) => targetWindow.location.assign(target),
		returnTarget: resolveBrowserAuthenticationReturnTarget(targetWindow.location.search),
	});
	let reactRoot: Root | undefined = createRoot(rootElement);
	reactRoot.render(
		createElement(
			ApplicationLocaleProvider,
			{ source: applicationLocaleController },
			createElement(BrowserAuthenticationPage, { controller }),
		),
	);
	return {
		dispose: () => {
			reactRoot?.unmount();
			reactRoot = undefined;
			controller.dispose();
			unsubscribeLocale();
			applicationLocaleController.dispose();
		},
	};
}
