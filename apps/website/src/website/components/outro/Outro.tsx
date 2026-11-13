import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { REPOSITORY_URL, type Release } from '../../release';
import { BeeMark } from '../bee-mark/BeeMark';
import { PillButton } from '../pill-button/PillButton';
import { SectionHeading } from '../section-heading/SectionHeading';
import styles from './Outro.module.css';

// The close: the last heading, the bee the swarm gathers around, and the two ways in.
export function Outro({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['outro']} data-scene='7'>
			<div className={styles['inner']}>
				<SectionHeading
					lead={t(messages.outroTitleLead)}
					accent={t(messages.outroTitleAccent)}
					variant='outro'
				/>
				<div className={styles['middle']}>
					<span className={styles['bee']} data-outro-bee=''>
						<BeeMark />
					</span>
				</div>
				<div className={styles['actions']}>
					{release.published ? (
						<PillButton variant='primary' icon='dl' href={release.installerUrl}>
							{t(messages.downloadForWindows)}
						</PillButton>
					) : (
						<PillButton variant='primary' icon='dl' href='#install'>
							{t(messages.getSource)}
						</PillButton>
					)}
					<PillButton variant='glass' icon='gh' href={REPOSITORY_URL}>
						GitHub
					</PillButton>
				</div>
			</div>
		</section>
	);
}
