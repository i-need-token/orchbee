import {
	createOrchbeeIntl,
	ORCHBEE_LOCALES,
	websiteCliMessages,
	websiteDesktopMessages,
	websiteHomeMessages,
	websiteShellMessages,
	websiteTerminalMessages,
	type MessageDescriptor,
	type OrchbeeLocale,
} from '@orchbee/i18n';

import { pagePath, type RenderedPage, type WebsitePage } from './pages';

const SITE_ORIGIN = 'https://orchbee.com';

const META: Readonly<
	Record<WebsitePage, { title: MessageDescriptor; description: MessageDescriptor }>
> = {
	home: { title: websiteHomeMessages.metaTitle, description: websiteHomeMessages.metaDescription },
	terminal: {
		title: websiteTerminalMessages.metaTitle,
		description: websiteTerminalMessages.metaDescription,
	},
	desktop: {
		title: websiteDesktopMessages.metaTitle,
		description: websiteDesktopMessages.metaDescription,
	},
	cli: { title: websiteCliMessages.metaTitle, description: websiteCliMessages.metaDescription },
};

export interface PageMetadata {
	readonly title: string;
	readonly description: string;
	readonly language: OrchbeeLocale;
	readonly origin: string;
	/** The page's absolute address; absent on the 404 page, which has no address of its own. */
	readonly canonical: string | undefined;
	/** The page's absolute address in every language, absent on the 404 page. */
	readonly alternates: readonly { readonly language: OrchbeeLocale; readonly href: string }[];
}

// The document title, description and discovery links of one page in one language.
export function getPageMetadata(locale: OrchbeeLocale, page: RenderedPage): PageMetadata {
	const intl = createOrchbeeIntl(locale);
	if (page === 'not-found')
		return {
			title: intl.formatMessage(websiteShellMessages.notFoundMetaTitle),
			description: intl.formatMessage(websiteHomeMessages.metaDescription),
			language: locale,
			origin: SITE_ORIGIN,
			canonical: undefined,
			alternates: [],
		};
	return {
		title: intl.formatMessage(META[page].title),
		description: intl.formatMessage(META[page].description),
		language: locale,
		origin: SITE_ORIGIN,
		canonical: `${SITE_ORIGIN}${pagePath(page, locale)}`,
		alternates: ORCHBEE_LOCALES.map((language) => ({
			language,
			href: `${SITE_ORIGIN}${pagePath(page, language)}`,
		})),
	};
}
