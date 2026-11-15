import { useOrchbeeIntl, websiteHomeMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { SectionHead } from '../../../section-head/SectionHead';
import { ThemeList } from './components/theme-list/ThemeList';
import { ThemeWindow } from './components/theme-window/ThemeWindow';
import styles from './TerminalCore.module.css';

// What the terminal does, each a fact of the product.
const FACTS = [
	messages.terminalFact1,
	messages.terminalFact2,
	messages.terminalFact3,
	messages.terminalFact4,
	messages.terminalFact5,
	messages.terminalFact6,
] as const;

/**
 * The terminal: the product's terminal themes beside a window that shows the chosen one, and the
 * terminal's facts under it. While the section is on screen the themes take turns in the window
 * until the visitor picks one (`activateTerminalThemes`).
 */
export function TerminalCore(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['section']} id='terminal-core'>
			<SectionHead
				kicker={t(messages.terminalKicker)}
				title={t(messages.terminalTitle)}
				lead={t(messages.terminalLead)}
			/>
			<div className={styles['themes']} data-reveal='' data-terminal-themes=''>
				<ThemeList />
				<div>
					<ThemeWindow />
					<ul className={styles['facts']}>
						{FACTS.map((fact) => (
							<li key={fact.id} className={styles['fact']}>
								{t(fact)}
							</li>
						))}
					</ul>
				</div>
			</div>
		</section>
	);
}
