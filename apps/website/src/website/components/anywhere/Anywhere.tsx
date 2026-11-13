import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import {
	MULTIPLEX_DESKTOP_RECORDING,
	MULTIPLEX_RECORDING,
	MULTIPLEX_TUI_RECORDING,
} from '../../pictures';
import { ProductRecording } from '../product-recording/ProductRecording';
import { SectionHeading } from '../section-heading/SectionHeading';
import styles from './Anywhere.module.css';

// One session seen from the terminal interface and the desktop app at once: a recording of the
// same commands appearing in both, side by side on wide screens and one half above the other on
// narrow ones.
export function Anywhere(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const terminal = (
		<>
			<b>{t(messages.captionTerminal)}</b>TUI
		</>
	);
	const desktop = <b>{t(messages.captionDesktop)}</b>;
	return (
		<section className={styles['scene']} id='anywhere' data-scene='1'>
			<div className={styles['inner']}>
				<div className={styles['heading']}>
					<SectionHeading
						number='01'
						label={t(messages.sceneAnywhere)}
						lead={t(messages.anywhereTitleLead)}
						accent={t(messages.anywhereTitleAccent)}
					>
						{t(messages.anywhereBody)}
					</SectionHeading>
				</div>
				<figure className={styles['wide']}>
					<div className={styles['frame']}>
						<ProductRecording
							src={MULTIPLEX_RECORDING.src}
							poster={MULTIPLEX_RECORDING.poster}
							width={MULTIPLEX_RECORDING.width}
							height={MULTIPLEX_RECORDING.height}
							label={t(messages.multiplexAlt)}
						/>
					</div>
					<figcaption className={styles['captions']}>
						<span>{terminal}</span>
						<span>{desktop}</span>
					</figcaption>
				</figure>
				<div className={styles['narrow']}>
					<figure className={styles['half']}>
						<div className={styles['frame']}>
							<ProductRecording
								src={MULTIPLEX_TUI_RECORDING.src}
								poster={MULTIPLEX_TUI_RECORDING.poster}
								width={MULTIPLEX_TUI_RECORDING.width}
								height={MULTIPLEX_TUI_RECORDING.height}
								label={t(messages.multiplexTuiAlt)}
							/>
						</div>
						<figcaption className={styles['caption']}>{terminal}</figcaption>
					</figure>
					<figure className={styles['half']}>
						<div className={styles['frame']}>
							<ProductRecording
								src={MULTIPLEX_DESKTOP_RECORDING.src}
								poster={MULTIPLEX_DESKTOP_RECORDING.poster}
								width={MULTIPLEX_DESKTOP_RECORDING.width}
								height={MULTIPLEX_DESKTOP_RECORDING.height}
								label={t(messages.multiplexDesktopAlt)}
							/>
						</div>
						<figcaption className={styles['caption']}>{desktop}</figcaption>
					</figure>
				</div>
			</div>
		</section>
	);
}
