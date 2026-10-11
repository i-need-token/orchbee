import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { renderToStaticMarkup } from 'react-dom/server';

import { getPageMetadata, Website } from '../website';

export function renderPage(
	locale: OrchbeeLocale,
): ReturnType<typeof getPageMetadata> & { markup: string } {
	return {
		...getPageMetadata(locale),
		markup: renderToStaticMarkup(
			<OrchbeeI18nProvider locale={locale}>
				<Website locale={locale} />
			</OrchbeeI18nProvider>,
		),
	};
}
