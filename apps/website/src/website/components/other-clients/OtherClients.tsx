import {
	useOrchbeeIntl,
	websiteShellMessages as messages,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import type { WebsitePage } from '../../pages';
import type { Release } from '../../release';
import { ClientIndex } from '../client-index/ClientIndex';
import { GetOrchbee } from '../get-orchbee/GetOrchbee';
import { Kicker } from '../kicker/Kicker';
import styles from './OtherClients.module.css';

// The closing section of a client page: the other two clients, then the call to action.
export function OtherClients({
	locale,
	current,
	release,
}: {
	locale: OrchbeeLocale;
	current: Exclude<WebsitePage, 'home'>;
	release: Release;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['section']}>
			<div className={styles['label']}>
				<Kicker reveal heading>
					{t(messages.otherClients)}
				</Kicker>
			</div>
			<ClientIndex locale={locale} current={current} />
			<div className={styles['action']}>
				<GetOrchbee locale={locale} release={release} offer='both' />
			</div>
		</section>
	);
}
