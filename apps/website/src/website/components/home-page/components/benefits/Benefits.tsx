import { useOrchbeeIntl, websiteHomeMessages as messages } from '@orchbee/i18n';
import { terminalColorThemes } from '@orchbee/terminal';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { Highlights } from '../../../highlights/Highlights';
import { SectionHead } from '../../../section-head/SectionHead';
import styles from './Benefits.module.css';

const BENEFITS = [
	{ id: 'status', title: messages.benefitsStatusTitle, body: messages.benefitsStatusBody },
	{ id: 'persist', title: messages.benefitsPersistTitle, body: messages.benefitsPersistBody },
	{ id: 'delegate', title: messages.benefitsDelegateTitle, body: messages.benefitsDelegateBody },
	{ id: 'terminal', title: messages.benefitsTerminalTitle, body: messages.benefitsTerminalBody },
] as const;

// Why to use Orchbee, in four cards under the demo. The terminal card counts the product's own
// terminal themes, read at build time.
export function Benefits(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['section']} id='benefits'>
			<SectionHead
				kicker={t(messages.benefitsKicker)}
				title={t(messages.benefitsTitle)}
				lead={t(messages.benefitsLead, richText)}
			/>
			<Highlights
				columns={4}
				items={BENEFITS.map(({ id, title, body }) => ({
					id,
					title: t(title),
					body: t(body, { ...richText, count: terminalColorThemes.length }),
				}))}
			/>
		</section>
	);
}
