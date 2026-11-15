import { useOrchbeeIntl, websiteHomeMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { SectionHead } from '../../../section-head/SectionHead';
import { StateLegend } from '../../../state-legend/StateLegend';
import { AgentRing } from './components/agent-ring/AgentRing';
import { SidebarAnatomy } from './components/sidebar-anatomy/SidebarAnatomy';
import { RECOGNIZED_AGENTS } from './recognized-agents';
import styles from './Agents.module.css';

/**
 * Agent status and organization: the ring of Agents beside the five states and the Agent CLIs
 * the product recognizes, then the sidebar of both clients taken apart.
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
					<div className={styles['states']}>
						<StateLegend
							states={['working', 'attention', 'done', 'failed', 'idle']}
							layout='list'
						/>
					</div>
					<p className={styles['recognized']}>{t(messages.agentsRecognized)}</p>
					<ul className={styles['names']}>
						{RECOGNIZED_AGENTS.map((name) => (
							<li key={name.id} className={styles['name']}>
								{t(name)}
							</li>
						))}
					</ul>
				</div>
			</div>
			<SidebarAnatomy />
		</section>
	);
}
