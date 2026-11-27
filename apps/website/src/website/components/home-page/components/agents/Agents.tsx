import { useOrchbeeIntl, websiteHomeMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { SectionHead } from '../../../section-head/SectionHead';
import { StateLegend } from '../../../state-legend/StateLegend';
import { AgentRing } from './components/agent-ring/AgentRing';
import { FEATURED_AGENTS, RECOGNIZED_AGENTS } from './recognized-agents';
import styles from './Agents.module.css';

/**
 * The Agents Orchbee works with: the ring of Agents beside the five states, the Agent CLIs most
 * visitors use, and every recognized one behind a toggle that works without script.
 */
export function Agents(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['section']} id='agents'>
			<SectionHead
				kicker={t(messages.agentsKicker)}
				title={t(messages.agentsTitle)}
				lead={t(messages.agentsLead)}
			/>
			<div className={styles['agents']} data-reveal=''>
				<AgentRing />
				<div>
					<ul className={styles['featured']}>
						{FEATURED_AGENTS.map((name) => (
							<li key={name.id} className={styles['feature']}>
								{t(name)}
							</li>
						))}
					</ul>
					<details className={styles['all']}>
						<summary className={styles['toggle']}>
							{t(messages.agentsAll, { count: RECOGNIZED_AGENTS.length })}
						</summary>
						<ul className={styles['names']}>
							{RECOGNIZED_AGENTS.map((name) => (
								<li key={name.id} className={styles['name']}>
									{t(name)}
								</li>
							))}
						</ul>
					</details>
					<div className={styles['states']}>
						<StateLegend
							states={['working', 'attention', 'done', 'failed', 'idle']}
							layout='list'
						/>
					</div>
				</div>
			</div>
		</section>
	);
}
