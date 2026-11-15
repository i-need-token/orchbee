import { useOrchbeeIntl, websiteDesktopMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { Kicker } from '../../../kicker/Kicker';
import { DetailDuo } from '../../../detail-duo/DetailDuo';
import { OptionSwitch } from '../option-switch/OptionSwitch';
import styles from './DesktopRendering.module.css';

// Code whose operators JetBrains Mono joins into ligatures.
const SAMPLE = 'if (a !== b && count >= 10) {\n  items.map((x) => x * 2);   // -> done\n}';

/**
 * Rendering: a sample in the terminal font with its ligatures, which the visitor turns off and on,
 * beside what the renderer offers. Without script the ligatures stay on and the switch hidden.
 */
export function DesktopRendering(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<DetailDuo
			flip
			text={
				<>
					<Kicker>{t(messages.renderingKicker)}</Kicker>
					<h2 className={styles['title']}>{t(messages.renderingTitle)}</h2>
					<p className={styles['paragraph']}>{t(messages.renderingBody)}</p>
					<ul className={styles['facts']}>
						<li>WebGPU</li>
						<li>{t(messages.factCanvas)}</li>
						<li>{t(messages.factColor)}</li>
						<li>{t(messages.factKitty)}</li>
						<li>{t(messages.factHistory)}</li>
					</ul>
				</>
			}
			figure={
				<div data-desktop-rendering=''>
					<div className={styles['sample']} data-ligature-sample=''>
						{SAMPLE}
					</div>
					<OptionSwitch
						name='ligatures'
						label={t(messages.ligatures)}
						options={[
							{ id: 'on', label: t(messages.ligaturesOn) },
							{ id: 'off', label: t(messages.ligaturesOff) },
						]}
						pressed='on'
					/>
				</div>
			}
		/>
	);
}
