import { applicationRoutingMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ReactNode } from 'react';

import styles from './RouteUnavailable.module.css';

export function RouteUnavailable(): ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<main className={styles['root']} data-slot='route-unavailable'>
			<div className={styles['content']}>
				<h1 className={styles['title']}>
					{intl.formatMessage(applicationRoutingMessages.unavailableTitle)}
				</h1>
				<p className={styles['description']}>
					{intl.formatMessage(applicationRoutingMessages.unavailableDescription)}
				</p>
			</div>
		</main>
	);
}
