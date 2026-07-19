import { getOrchbeeTextDirection, resolveOrchbeeLocale } from '@orchbee/i18n';
import type { WorkbenchLayoutStorage } from '@orchbee/workbench';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import { WebApplication } from '../web-application';

export function startWebApplication(
	rootElement: Element,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	layoutStorage: WorkbenchLayoutStorage,
): void {
	const locale = resolveOrchbeeLocale(preferredLocales);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);

	createRoot(rootElement).render(createElement(WebApplication, { layoutStorage, locale }));
}
