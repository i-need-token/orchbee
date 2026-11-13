import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import { SectionHeading } from '../section-heading/SectionHeading';
import { WindowFrame } from '../window-frame/WindowFrame';
import styles from './Privacy.module.css';

const TRAFFIC = [
	{ process: 'claude', host: 'api.anthropic.com', requests: '1,284', bar: 'bar100' },
	{ process: 'codex', host: 'api.openai.com', requests: '906', bar: 'bar71' },
	{ process: 'pnpm', host: 'registry.npmjs.org', requests: '212', bar: 'bar17' },
	{ process: 'git', host: 'github.com', requests: '14', bar: 'bar2' },
] as const;

// What Orchbee does not do: accounts, telemetry, network. A day of network traffic shows the
// Agents talking to their providers and Orchbee itself silent.
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
				<div className={styles['grid']}>
					<ul className={styles['facts']}>
						{facts.map(([title, body]) => (
							<li key={title}>
								<b>{title}</b>
								<span>{body}</span>
							</li>
						))}
					</ul>
					<WindowFrame title={t(messages.networkTitle)}>
						<div className={styles['head']}>
							<span>{t(messages.networkProcess)}</span>
							<span>{t(messages.networkTalksTo)}</span>
							<span className={styles['count']}>{t(messages.networkRequests)}</span>
						</div>
						{TRAFFIC.map((row) => (
							<div className={styles['row']} key={row.process}>
								<span className={styles['process']}>{row.process}</span>
								<span className={styles['host']}>
									<span>{row.host}</span>
									<i className={styles[row.bar]} />
								</span>
								<span className={styles['count']}>{row.requests}</span>
							</div>
						))}
						<div className={styles['self']}>
							<span className={styles['process']}>orchbee</span>
							<span className={styles['host']}>
								<span>—</span>
							</span>
							<span className={styles['count']}>0</span>
						</div>
						<p className={styles['foot']}>
							{t(messages.networkFooter, {
								code: (chunks: ReactNode[]) => <code key='code'>{chunks}</code>,
							})}
						</p>
					</WindowFrame>
				</div>
			</div>
		</section>
	);
}
