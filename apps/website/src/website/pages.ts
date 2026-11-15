import { ORCHBEE_LOCALES, type OrchbeeLocale } from '@orchbee/i18n';

// The site's pages. Each one is generated in every locale: English at its path, Simplified
// Chinese under /zh-CN/ followed by the same path.
export const WEBSITE_PAGES = ['home', 'terminal', 'desktop', 'cli'] as const;

export type WebsitePage = (typeof WEBSITE_PAGES)[number];

/** The page the build renders: one of the site's pages, or the page served for a missing address. */
export type RenderedPage = WebsitePage | 'not-found';

const PAGE_PATHS: Readonly<Record<WebsitePage, string>> = {
	home: '/',
	terminal: '/terminal/',
	desktop: '/desktop/',
	cli: '/cli/',
};

const LOCALE_PREFIXES: Readonly<Record<OrchbeeLocale, string>> = {
	'en-US': '',
	'zh-CN': '/zh-CN',
};

/** The address of `page` in `locale`, from the site root, ending in a slash. */
export function pagePath(page: WebsitePage, locale: OrchbeeLocale): string {
	return `${LOCALE_PREFIXES[locale]}${PAGE_PATHS[page]}`;
}

/** The page and locale an address names, or undefined when the site has no such page. */
export function pageAtPath(path: string): { page: WebsitePage; locale: OrchbeeLocale } | undefined {
	const normalized = path.replace(/index\.html$/u, '').replace(/\/?$/u, '/');
	for (const locale of ORCHBEE_LOCALES)
		for (const page of WEBSITE_PAGES)
			if (pagePath(page, locale) === normalized) return { page, locale };
	return undefined;
}

export function otherLocale(locale: OrchbeeLocale): OrchbeeLocale {
	return locale === 'zh-CN' ? 'en-US' : 'zh-CN';
}
