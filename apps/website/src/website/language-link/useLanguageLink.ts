import {
	useOrchbeeIntl,
	websiteShellMessages as messages,
	type OrchbeeLocale,
} from '@orchbee/i18n';

import { otherLocale, pagePath, type RenderedPage } from '../pages';

// The attributes and text of a link to the same page in the other language, named in that
// language; the 404 page links to the other language's home page. Its owner renders the <a> with
// `href`, `hrefLang` and `lang` set to these values and a data-language-link attribute, which
// `activateLanguageLinks` uses to keep the visitor's section on the way.
export function useLanguageLink(
	locale: OrchbeeLocale,
	page: RenderedPage,
): { href: string; locale: OrchbeeLocale; label: string } {
	const { formatMessage: t } = useOrchbeeIntl();
	const target = otherLocale(locale);
	return {
		href: pagePath(page === 'not-found' ? 'home' : page, target),
		locale: target,
		label: t(messages.otherLanguage),
	};
}
