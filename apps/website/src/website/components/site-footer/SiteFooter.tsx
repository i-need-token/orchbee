import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { REPOSITORY_URL } from '../../release';
import styles from './SiteFooter.module.css';

export function SiteFooter({
	home,
	installHref,
}: {
	home: string;
	installHref: string;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<footer className={styles['footer']}>
			<div className={styles['inner']}>
				<div className={styles['row']}>
					<div className={styles['identity']}>
						<a className={styles['brand']} href={home}>
							<img src='/icon.svg' width='24' height='24' alt='' />
							Orchbee
						</a>
						<p className={styles['tagline']}>{t(messages.footerTagline)}</p>
					</div>
					<nav className={styles['links']} aria-label={t(messages.footerNavigation)}>
						<a href={installHref}>{t(messages.installLabel)}</a>
						<a href={REPOSITORY_URL}>GitHub</a>
						<a href={`${REPOSITORY_URL}/issues`}>{t(messages.footerIssues)}</a>
						<a href={`${REPOSITORY_URL}/blob/main/LICENSE`}>{t(messages.footerLicense)}</a>
					</nav>
				</div>
				<div className={styles['note']}>
					<p>{t(messages.footerPrivacy)}</p>
					<p>© 2026 Orchbee</p>
				</div>
			</div>
		</footer>
	);
}
