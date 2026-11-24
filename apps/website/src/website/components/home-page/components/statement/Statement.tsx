import { useOrchbeeIntl, websiteHomeMessages as messages } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import { offersMacos, REPOSITORY_URL, type Release } from '../../../../release';
import styles from './Statement.module.css';

const LICENSE = 'MIT';

// What Orchbee is, in one large paragraph, beside a short data sheet of the facts. The platform row
// names macOS only while the release offers it.
export function Statement({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const rows: readonly (readonly [string, ReactNode])[] = [
		[t(messages.sheetClients), t(messages.sheetClientsValue)],
		[t(messages.sheetAgents), t(messages.sheetAgentsValue)],
		[t(messages.sheetTerminal), t(messages.sheetTerminalValue)],
		[
			t(messages.sheetPlatform),
			t(offersMacos(release) ? messages.sheetPlatformValueMacos : messages.sheetPlatformValue),
		],
		[t(messages.sheetPrivacy), t(messages.sheetPrivacyValue)],
		[t(messages.sheetLicense), LICENSE],
		[
			t(messages.sheetSource),
			<a
				key='source'
				className={styles['link']}
				href={REPOSITORY_URL}
				target='_blank'
				rel='noreferrer'
			>
				{REPOSITORY_URL.replace('https://', '')}
			</a>,
		],
	];
	return (
		<div className={styles['statement']} data-reveal=''>
			<p className={styles['text']}>
				{t(messages.statement, {
					quiet: (chunks: readonly ReactNode[]) => (
						<span className={styles['quiet']}>{chunks}</span>
					),
				})}
			</p>
			<dl className={styles['sheet']}>
				{rows.map(([term, value]) => (
					<div key={term} className={styles['row']}>
						<dt className={styles['term']}>{term}</dt>
						<dd className={styles['value']}>{value}</dd>
					</div>
				))}
			</dl>
		</div>
	);
}
