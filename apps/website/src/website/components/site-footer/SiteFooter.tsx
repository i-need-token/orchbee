import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { README_URL, REPOSITORY_URL } from '../../release';
import { BrandMark } from '../brand-mark/BrandMark';
import styles from './SiteFooter.module.css';

// The footer: the mark and tagline, the links, and the site's own privacy note.
export function SiteFooter({ home }: { home: string }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<footer className={styles['foot']}>
			<div className={styles['inner']}>
				<div className={styles['row']}>
					<div className={styles['brand']}>
						<a className={styles['mark']} href={`${home}#top`} aria-label='Orchbee'>
							<span className={styles['logo']}>
								<BrandMark />
							</span>
							<b>Orchbee</b>
						</a>
						<span className={styles['tagline']}>{t(messages.tagline)}</span>
					</div>
					<nav className={styles['links']} aria-label={t(messages.footerNavigation)}>
						<a href={`${home}#install`}>{t(messages.sceneInstall)}</a>
						<a href={README_URL}>{t(messages.docs)}</a>
						<a href={REPOSITORY_URL}>GitHub</a>
					</nav>
				</div>
				<div className={styles['note']}>
					<span>{t(messages.footerPrivacy)}</span>
					<span>© 2026 Orchbee</span>
				</div>
			</div>
		</footer>
	);
}
