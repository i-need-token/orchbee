import { createOrchbeeIntl, websiteMessages, type OrchbeeLocale } from '@orchbee/i18n';

const SITE_ORIGIN = 'https://orchbee.com';

export function getPageMetadata(locale: OrchbeeLocale): {
	title: string;
	description: string;
	canonical: string;
	language: OrchbeeLocale;
	alternate: string;
} {
	const intl = createOrchbeeIntl(locale);
	return {
		title: intl.formatMessage(websiteMessages.metaTitle),
		description: intl.formatMessage(websiteMessages.metaDescription),
		canonical: `${SITE_ORIGIN}${locale === 'zh-CN' ? '/zh-CN/' : '/'}`,
		language: locale,
		alternate: locale === 'zh-CN' ? '/' : '/zh-CN/',
	};
}
