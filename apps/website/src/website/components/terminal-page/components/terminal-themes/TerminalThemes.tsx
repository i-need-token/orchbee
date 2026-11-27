import { useOrchbeeIntl, websiteTerminalMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { DetailSection } from '../../../detail-section/DetailSection';
import { ThemeList } from './components/theme-list/ThemeList';
import { ThemeWindow } from './components/theme-window/ThemeWindow';
import styles from './TerminalThemes.module.css';

/**
 * The product's terminal themes beside a window that shows the chosen one. While the list is on
 * screen the themes take turns in the window until the visitor picks one
 * (`activateTerminalThemes`).
 */
export function TerminalThemes(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<DetailSection
			kicker={t(messages.themesKicker)}
			title={t(messages.themesTitle)}
			lead={t(messages.themesLead)}
		>
			<div className={styles['themes']} data-reveal='' data-terminal-themes=''>
				<ThemeList />
				<ThemeWindow />
			</div>
		</DetailSection>
	);
}
