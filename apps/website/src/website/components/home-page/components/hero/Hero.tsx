import { useOrchbeeIntl, websiteHomeMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { offersMacos, type Release } from '../../../../release';
import { GetOrchbee } from '../../../get-orchbee/GetOrchbee';
import { SessionScene } from '../../../session-scene/SessionScene';
import { useSessionFrame } from '../../../session-scene/useSessionFrame';
import { StateGlyph } from '../../../state-glyph/StateGlyph';
import { HeroStage } from './components/hero-stage/HeroStage';
import styles from './Hero.module.css';

// The top of the home page: the status line, the headline, the lead, the way to get Orchbee, and
// the drawn Session. The status line follows the Session as it plays.
export function Hero({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const frame = useSessionFrame();
	return (
		<div className={styles['column']}>
			<SessionScene>
				<div className={styles['head']}>
					<div>
						<p className={styles['status']} data-live=''>
							<StateGlyph state={frame.states.summary} bind='summary' />
							<span data-scene-text='summary'>{frame.texts.summary}</span>
						</p>
						<h1 className={styles['title']}>{t(messages.title)}</h1>
					</div>
					<div>
						<p className={styles['lead']}>{t(messages.lead)}</p>
						<GetOrchbee release={release} offer='both' />
						<p className={styles['beta']}>
							{t(offersMacos(release) ? messages.betaMacos : messages.beta)}
						</p>
					</div>
				</div>
				<HeroStage />
			</SessionScene>
		</div>
	);
}
