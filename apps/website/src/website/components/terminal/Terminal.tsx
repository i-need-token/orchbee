import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import { BeeMark } from '../bee-mark/BeeMark';
import { SectionHeading } from '../section-heading/SectionHeading';
import { WindowFrame } from '../window-frame/WindowFrame';
import { DEMO_KEYS } from './demo-keys';
import styles from './Terminal.module.css';

// The terminal underneath: a split the visitor can drag, an image drawn with the Kitty
// graphics protocol, and a pane that shows the Kitty keyboard sequence of each key pressed.
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
				<div className={styles['window']}>
					<WindowFrame title='orchbee — ~/code/website'>
						<div className={styles['split']} data-split=''>
							<div className={styles['left']} data-split-left=''>
								<div>
									<span className={styles['prompt']}>$ </span>kitten icat bee.png
								</div>
								<span className={styles['image']} role='img' aria-label={t(messages.kittyAlt)}>
									<BeeMark />
								</span>
								<div>
									<span className={styles['prompt']}>$ </span>
									<i className={styles['caret']} />
								</div>
							</div>
							<div
								className={styles['gutter']}
								role='separator'
								tabIndex={0}
								aria-orientation='vertical'
								aria-valuemin={25}
								aria-valuemax={75}
								aria-valuenow={46}
								aria-label={t(messages.splitLabel)}
								data-split-gutter=''
							/>
							<div className={styles['right']}>
								<div
									className={styles['keys']}
									tabIndex={0}
									role='log'
									aria-label={t(messages.keysLabel)}
									data-keys=''
								>
									<div className={styles['keysHead']}>
										<span>{t(messages.keysHeading)}</span>
									</div>
									<div data-key-rows=''>
										{DEMO_KEYS.map((key) => (
											<div className={styles['keyOld']} key={key.name}>
												<b>{key.name}</b>
												<code>{key.sequence}</code>
											</div>
										))}
									</div>
								</div>
							</div>
						</div>
						<div className={styles['foot']}>
							<span>Ghostty VT</span>
							<span data-split-size=''>55×32 · 65×32</span>
						</div>
					</WindowFrame>
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
