import { useOrchbeeIntl, websiteHomeMessages as messages } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import type { Release } from '../../../../release';
import { GetOrchbee } from '../../../get-orchbee/GetOrchbee';
import { StateGlyph } from '../../../state-glyph/StateGlyph';
import styles from './Hero.module.css';

// The top of the home page: the status line, the headline, the lead and the way to get Orchbee.
// The status line shows the state the drawn Session settles in. `children` follow the head in the
// same column.
export function Hero({
	release,
	children,
}: {
	release: Release;
	children?: ReactNode;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<div className={styles['column']}>
			<div className={styles['head']}>
				<div>
					<p className={styles['status']} data-live=''>
						<StateGlyph state='done' />
						<span>
							{[t(messages.statusDone, { count: 2 }), t(messages.statusReady, { count: 1 })].join(
								' · ',
							)}
						</span>
					</p>
					<h1 className={styles['title']}>{t(messages.title)}</h1>
				</div>
				<div>
					<p className={styles['lead']}>{t(messages.lead)}</p>
					<GetOrchbee release={release} offer='both' />
					<p className={styles['beta']}>{t(messages.beta)}</p>
				</div>
			</div>
			{children}
		</div>
	);
}
