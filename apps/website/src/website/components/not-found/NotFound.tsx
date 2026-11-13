import { useOrchbeeIntl, websiteMessages as messages, type OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { PillButton } from '../pill-button/PillButton';
import styles from './NotFound.module.css';

// The page Cloudflare serves, with status 404, for any address the site does not have.
export function NotFound({ locale }: { locale: OrchbeeLocale }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['missing']} aria-labelledby='missing-title'>
			<div className={styles['inner']}>
				<p className={styles['code']}>404</p>
				<h1 className={styles['title']} id='missing-title'>
					<span className={styles['accent']}>{t(messages.notFoundTitle)}</span>
				</h1>
				<p className={styles['body']}>{t(messages.notFoundBody)}</p>
				<div className={styles['actions']}>
					<PillButton variant='primary' href={locale === 'zh-CN' ? '/zh-CN/' : '/'}>
						{t(messages.notFoundHome)}
					</PillButton>
				</div>
			</div>
		</section>
	);
}
