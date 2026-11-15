import { useOrchbeeIntl, websiteHomeMessages as messages, type OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { ClientIndex } from '../../../client-index/ClientIndex';
import { SectionHead } from '../../../section-head/SectionHead';
import styles from './Clients.module.css';

// The index of the three client pages.
export function Clients({ locale }: { locale: OrchbeeLocale }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['section']} id='clients'>
			<SectionHead
				kicker={t(messages.clientsKicker)}
				title={t(messages.clientsTitle)}
				lead={t(messages.clientsLead)}
			/>
			<ClientIndex locale={locale} />
		</section>
	);
}
