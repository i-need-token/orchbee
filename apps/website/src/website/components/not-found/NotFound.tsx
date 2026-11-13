import { useOrchbeeIntl, websiteMessages as messages, type OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { AccentText } from '../accent-text/AccentText';
import { LinkButton } from '../link-button/LinkButton';
import styles from './NotFound.module.css';

// The page Cloudflare serves, with status 404, for any address the site does not have.
export function NotFound({ locale }: { locale: OrchbeeLocale }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['missing']} aria-labelledby='missing-title'>
			<p className={styles['code']}>404</p>
			<h1 className={styles['title']} id='missing-title'>
				<AccentText>{t(messages.notFoundTitle)}</AccentText>
			</h1>
			<p className={styles['body']}>{t(messages.notFoundBody)}</p>
			<div className={styles['actions']}>
				<LinkButton href={locale === 'zh-CN' ? '/zh-CN/' : '/'}>
					{t(messages.notFoundHome)}
				</LinkButton>
			</div>
		</section>
	);
}
