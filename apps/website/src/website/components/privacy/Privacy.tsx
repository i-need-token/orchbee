import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { SectionHeading } from '../section-heading/SectionHeading';
import styles from './Privacy.module.css';

// What Orchbee does not do: accounts, telemetry, network.
export function Privacy(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const facts = [
		[t(messages.noAccountTitle), t(messages.noAccountBody)],
		[t(messages.noTelemetryTitle), t(messages.noTelemetryBody)],
		[t(messages.offlineTitle), t(messages.offlineBody)],
	] as const;
	return (
		<section className={styles['scene']} id='private' data-scene='4'>
			<div className={styles['inner']}>
				<div className={styles['heading']}>
					<SectionHeading
						number='04'
						label={t(messages.scenePrivate)}
						lead={t(messages.privateTitleLead)}
						accent={t(messages.privateTitleAccent)}
					/>
				</div>
				<ul className={styles['facts']}>
					{facts.map(([title, body]) => (
						<li key={title}>
							<b>{title}</b>
							<span>{body}</span>
						</li>
					))}
				</ul>
			</div>
		</section>
	);
}
