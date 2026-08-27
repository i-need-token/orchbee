import {
	browserAuthenticationMessages,
	createOrchbeeIntl,
	getOrchbeeTextDirection,
	OrchbeeI18nProvider,
	resolveOrchbeeLocale,
} from '@orchbee/i18n';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import {
	BrowserAuthenticationClient,
	BrowserAuthenticationPage,
	BrowserAuthenticationController,
	resolveBrowserAuthenticationReturnTarget,
} from '../browser-authentication-page';

export interface BrowserAuthenticationApplicationSession {
	dispose(): void;
}

export function startBrowserAuthenticationApplication(
	targetWindow: Window,
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
): BrowserAuthenticationApplicationSession {
	if (rootElement === null) throw new Error('The Orchbee authentication root element is missing.');
	const locale = resolveOrchbeeLocale(preferredLocales);
	const intl = createOrchbeeIntl(locale);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);
	targetWindow.document.title = intl.formatMessage(browserAuthenticationMessages.documentTitle);
	const controller = new BrowserAuthenticationController({
		client: new BrowserAuthenticationClient(targetWindow.fetch.bind(targetWindow)),
		navigate: (target) => targetWindow.location.assign(target),
		returnTarget: resolveBrowserAuthenticationReturnTarget(targetWindow.location.search),
	});
	let reactRoot: Root | undefined = createRoot(rootElement);
	reactRoot.render(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(BrowserAuthenticationPage, { controller }),
		),
	);
	return {
		dispose: () => {
			reactRoot?.unmount();
			reactRoot = undefined;
			controller.dispose();
		},
	};
}
