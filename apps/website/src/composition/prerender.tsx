import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { renderToStaticMarkup } from 'react-dom/server';

import { getPageMetadata, release, Website, type Release } from '../website';

export function renderPage(
	locale: OrchbeeLocale,
	currentRelease: Release = release,
): ReturnType<typeof getPageMetadata> & { markup: string } {
	return {
		...getPageMetadata(locale),
		markup: renderToStaticMarkup(
			<OrchbeeI18nProvider locale={locale}>
				<Website locale={locale} release={currentRelease} />
			</OrchbeeI18nProvider>,
		),
	};
}

export function renderNotFound(
	locale: OrchbeeLocale,
): ReturnType<typeof getPageMetadata> & { markup: string } {
	return {
		...getPageMetadata(locale, 'not-found'),
		markup: renderToStaticMarkup(
			<OrchbeeI18nProvider locale={locale}>
				<Website locale={locale} release={release} page='not-found' />
			</OrchbeeI18nProvider>,
		),
	};
}
