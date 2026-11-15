import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { renderToStaticMarkup } from 'react-dom/server';

import {
	getPageMetadata,
	release,
	Website,
	type PageMetadata,
	type Release,
	type RenderedPage,
} from '../website';

// Renders one page in one language to static markup, with the metadata its document head needs.
export function renderPage(
	locale: OrchbeeLocale,
	page: RenderedPage,
	currentRelease: Release = release,
): PageMetadata & { markup: string } {
	return {
		...getPageMetadata(locale, page),
		markup: renderToStaticMarkup(
			<OrchbeeI18nProvider locale={locale}>
				<Website locale={locale} page={page} release={currentRelease} />
			</OrchbeeI18nProvider>,
		),
	};
}
