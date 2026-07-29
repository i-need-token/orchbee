import { getOrchbeeTextDirection, resolveOrchbeeLocale } from '@orchbee/i18n';
import type { WorkbenchLayoutStorage, WorkbenchShortcutPlatform } from '@orchbee/workbench';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import { WebApplication } from '../web-application';

export function startWebApplication(
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	layoutStorage: WorkbenchLayoutStorage,
): void {
	if (rootElement === null) throw new Error('The Orchbee web root element is missing.');
	const locale = resolveOrchbeeLocale(preferredLocales);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);

	createRoot(rootElement).render(
		createElement(WebApplication, { layoutStorage, locale, shortcutPlatform }),
	);
}
