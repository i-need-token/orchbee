import { useOrchbeeIntl, websiteHomeMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import styles from './RemoteMachines.module.css';

/** The band on remote machines, a feature still in development: its tag, heading and paragraph. */
export function RemoteMachines(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['section']} data-reveal=''>
			<div className={styles['band']}>
				<div>
					<span className={styles['tag']}>
						<span className={styles['dot']} />
						<span>{t(messages.remoteTag)}</span>
					</span>
					<h2 className={styles['title']}>{t(messages.remoteTitle)}</h2>
				</div>
				<p className={styles['lead']}>{t(messages.remoteLead)}</p>
			</div>
		</section>
	);
}
