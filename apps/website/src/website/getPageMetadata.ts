import { createOrchbeeIntl, websiteMessages, type OrchbeeLocale } from '@orchbee/i18n';

const SITE_ORIGIN = 'https://orchbee.com';

export function getPageMetadata(
	locale: OrchbeeLocale,
	page: 'home' | 'not-found' = 'home',
): {
	title: string;
	description: string;
	origin: string;
	canonical: string;
	language: OrchbeeLocale;
	alternate: string;
} {
	const intl = createOrchbeeIntl(locale);
	return {
		title: intl.formatMessage(
			page === 'home' ? websiteMessages.metaTitle : websiteMessages.notFoundMetaTitle,
		),
		description: intl.formatMessage(websiteMessages.metaDescription),
		origin: SITE_ORIGIN,
		canonical: `${SITE_ORIGIN}${locale === 'zh-CN' ? '/zh-CN/' : '/'}`,
		language: locale,
		alternate: locale === 'zh-CN' ? '/' : '/zh-CN/',
	};
}
