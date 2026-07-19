import { getOrchbeeTextDirection, resolveOrchbeeLocale } from '@orchbee/i18n';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import { createInMemorySessionService } from '../sessions';
import { WebApplication } from '../web-application';

export function startWebApplication(
	rootElement: Element,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
): void {
	const locale = resolveOrchbeeLocale(preferredLocales);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);

	const sessions = createInMemorySessionService(locale);
	createRoot(rootElement).render(createElement(WebApplication, { locale, sessions }));
}
