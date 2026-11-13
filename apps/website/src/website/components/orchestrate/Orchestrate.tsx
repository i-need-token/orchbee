import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { AGENTS_CLI_RECORDING, AGENTS_CLI_SHELL_RECORDING } from '../../pictures';
import { ProductRecording } from '../product-recording/ProductRecording';
import { SectionHeading } from '../section-heading/SectionHeading';
import { Tag } from '../tag/Tag';
import { WindowFrame } from '../window-frame/WindowFrame';
import styles from './Orchestrate.module.css';

// A recording of a shell Panel driving another Agent with orchbee commands, cropped to that Panel on
// narrow screens, then the remote machines feature still in development.
export function Orchestrate(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['scene']} id='orchestrate' data-scene='3'>
			<div className={styles['inner']}>
				<div className={styles['heading']}>
					<SectionHeading
						number='03'
						label={t(messages.sceneOrchestrate)}
						lead={t(messages.orchestrateTitleLead)}
						accent={t(messages.orchestrateTitleAccent)}
					>
						{t(messages.orchestrateBody)}
					</SectionHeading>
				</div>
				<div className={styles['window']}>
					<WindowFrame title='orchbee'>
						<div className={styles['wide']}>
							<ProductRecording
								src={AGENTS_CLI_RECORDING.src}
								poster={AGENTS_CLI_RECORDING.poster}
								width={AGENTS_CLI_RECORDING.width}
								height={AGENTS_CLI_RECORDING.height}
								label={t(messages.agentsCliAlt)}
							/>
						</div>
						<div className={styles['narrow']}>
							<ProductRecording
								src={AGENTS_CLI_SHELL_RECORDING.src}
								poster={AGENTS_CLI_SHELL_RECORDING.poster}
								width={AGENTS_CLI_SHELL_RECORDING.width}
								height={AGENTS_CLI_SHELL_RECORDING.height}
								label={t(messages.agentsCliAlt)}
							/>
						</div>
					</WindowFrame>
				</div>
				<p className={styles['next']}>
					<Tag>{t(messages.tagInDevelopment)}</Tag>
					<span>{t(messages.orchestrateNext)}</span>
				</p>
			</div>
		</section>
	);
}
