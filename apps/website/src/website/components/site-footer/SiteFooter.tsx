import {
	useOrchbeeIntl,
	websiteShellMessages as messages,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import type { RenderedPage } from '../../pages';
import { useLanguageLink } from '../../language-link/useLanguageLink';
import { REPOSITORY_URL } from '../../release';
import { richText } from '../../rich-text';
import styles from './SiteFooter.module.css';

// Where the name comes from, and the project links.
export function SiteFooter({
	locale,
	page,
}: {
	locale: OrchbeeLocale;
	page: RenderedPage;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const language = useLanguageLink(locale, page);
	return (
		<footer className={styles['footer']}>
			<div className={styles['foot']}>
				<p className={styles['name']}>{t(messages.footerName, richText)}</p>
				<nav className={styles['links']} aria-label={t(messages.footerNavigation)}>
					<a className={styles['link']} href={REPOSITORY_URL} target='_blank' rel='noreferrer'>
						GitHub ↗
					</a>
					<a
						className={styles['link']}
						href={`${REPOSITORY_URL}/releases`}
						target='_blank'
						rel='noreferrer'
					>
						{t(messages.footerReleases)}
					</a>
					<a
						className={styles['link']}
						href={`${REPOSITORY_URL}/blob/main/LICENSE`}
						target='_blank'
						rel='noreferrer'
					>
						{t(messages.footerLicense)}
					</a>
					<a
						className={styles['link']}
						href={language.href}
						hrefLang={language.locale}
						lang={language.locale}
						data-language-link=''
					>
						{language.label}
					</a>
				</nav>
			</div>
		</footer>
	);
}
