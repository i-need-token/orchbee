import { useOrchbeeIntl, websiteCliMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { DetailSection } from '../../../detail-section/DetailSection';
import { MarkScope, NumberedMark } from '../../../numbered-marks/NumberedMarks';
import styles from './CliGrammar.module.css';

// One command line taken apart: each part, in order, with the note that names it. The command is
// the one the Session runs; terminal text stays English in every language.
const PARTS = [
	{ text: 'agent', title: messages.grammarFamilyTitle, body: messages.grammarFamilyBody },
	{ text: 'prompt', title: messages.grammarActionTitle, body: messages.grammarActionBody },
	{ text: '@panel:0x36', title: messages.grammarTargetTitle, body: messages.grammarTargetBody },
	{
		text: '"Review the current changes"',
		title: messages.grammarArgumentTitle,
		body: messages.grammarArgumentBody,
	},
	{ text: '--wait', title: messages.grammarOptionTitle, body: messages.grammarOptionBody },
] as const;

export function CliGrammar(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<DetailSection
			kicker={t(messages.grammarKicker)}
			title={t(messages.grammarTitle)}
			lead={t(messages.grammarLead)}
		>
			<MarkScope>
				<div className={styles['figure']} data-reveal=''>
					<div className={styles['line']}>
						<span className={styles['program']}>orchbee</span>
						{PARTS.map((part, index) => (
							<span key={part.text} className={styles['part']} data-mark={index + 1}>
								{part.text}
								<span className={styles['partMark']}>
									<NumberedMark number={index + 1} />
								</span>
							</span>
						))}
					</div>
				</div>
				<ol className={styles['notes']} data-reveal=''>
					{PARTS.map((part, index) => (
						<li key={part.text} className={styles['note']} data-mark={index + 1} data-mark-note=''>
							<NumberedMark number={index + 1} />
							<span>
								<b className={styles['noteTitle']}>{t(part.title)}</b>
								{t(part.body, richText)}
							</span>
						</li>
					))}
				</ol>
			</MarkScope>
		</DetailSection>
	);
}
