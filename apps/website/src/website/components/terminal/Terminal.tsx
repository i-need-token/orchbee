import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import { SectionHeading } from '../section-heading/SectionHeading';
import styles from './Terminal.module.css';

// The terminal underneath: the protocols programs rely on, and three points about using it.
export function Terminal(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const kbd = { kbd: (chunks: ReactNode[]) => <kbd key='kbd'>{chunks}</kbd> };
	const chips = [
		messages.chipGraphics,
		messages.chipKeyboard,
		messages.chipClipboard,
		messages.chipMouse,
		messages.chipPaste,
		messages.chipFocus,
		messages.chipColor,
		messages.chipGpu,
	];
	return (
		<section className={styles['scene']} id='terminal' data-scene='5'>
			<div className={styles['inner']}>
				<div className={styles['heading']}>
					<SectionHeading
						number='05'
						label={t(messages.sceneTerminal)}
						lead={t(messages.terminalTitleLead)}
						accent={t(messages.terminalTitleAccent)}
					>
						{t(messages.terminalBody)}
					</SectionHeading>
				</div>
				<div className={styles['chips']}>
					{chips.map((chip) => (
						<span key={chip.id}>{t(chip)}</span>
					))}
				</div>
				<div className={styles['points']}>
					<div>
						<h3>{t(messages.mouseTitle)}</h3>
						<p>{t(messages.mouseBody, kbd)}</p>
					</div>
					<div>
						<h3>{t(messages.learnTitle)}</h3>
						<p>{t(messages.learnBody)}</p>
					</div>
					<div>
						<h3>{t(messages.whereTitle)}</h3>
						<p>{t(messages.whereBody)}</p>
					</div>
				</div>
			</div>
		</section>
	);
}
