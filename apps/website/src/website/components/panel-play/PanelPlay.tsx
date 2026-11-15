import { useOrchbeeIntl, websiteShellMessages as messages } from '@orchbee/i18n';
import { useId, type ReactElement, type ReactNode } from 'react';

import { richText } from '../../rich-text';
import { DetailDuo } from '../detail-duo/DetailDuo';
import { KeyCaps } from '../key-caps/KeyCaps';
import { Kicker } from '../kicker/Kicker';
import { WindowFit } from '../window-fit/WindowFit';
import { PANEL_PLAY_KEYS, type PanelPlayClient } from './panel-play-clients';
import type { PanelAction, PanelRefusal } from '../panel-tree/panel-tree';
import styles from './PanelPlay.module.css';

const ACTION_LABELS = {
	right: messages.panelPlaySplitRight,
	down: messages.panelPlaySplitDown,
	left: messages.panelPlaySplitLeft,
	up: messages.panelPlaySplitUp,
	zoom: messages.panelPlayMaximize,
	equal: messages.panelPlayEqualize,
	close: messages.panelPlayClose,
} as const satisfies Record<PanelAction, unknown>;

const REFUSALS = {
	full: messages.panelPlayFull,
	small: messages.panelPlaySmall,
	last: messages.panelPlayLast,
	one: messages.panelPlayOne,
	none: messages.panelPlayNone,
} as const satisfies Record<PanelRefusal, unknown>;

/**
 * A section where the visitor works the Panels of a drawn window: the label, heading and paragraph
 * with the action buttons and the notice line, beside the window (`window`, which draws its Panels
 * with a PanelTree). `client` picks the actions and keys; `flip` puts the window first. Without
 * script the window shows its starting layout as a picture and the buttons and notices stay
 * hidden; activatePanelPlay makes the window a focusable control.
 */
export function PanelPlay({
	client,
	kicker,
	title,
	lead,
	window,
	flip = false,
}: {
	client: PanelPlayClient;
	kicker: ReactNode;
	title: ReactNode;
	lead: ReactNode;
	window: ReactNode;
	flip?: boolean;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const helpId = useId();
	const { actions, keys, resize, prefix } = PANEL_PLAY_KEYS[client];
	const notice = (key: string | undefined, label: ReactNode): ReactNode =>
		key === undefined ? (
			label
		) : (
			<>
				<b>{key}</b> {label}
			</>
		);
	return (
		<div data-panel-play={client}>
			<DetailDuo
				flip={flip}
				text={
					<>
						<Kicker>{kicker}</Kicker>
						<h2 className={styles['title']}>{title}</h2>
						<p className={styles['lead']}>{lead}</p>
						<div className={styles['chips']} data-panel-chips='' hidden>
							{actions.map((act) => {
								const key = keys[act];
								return (
									<button key={act} className={styles['chip']} type='button' data-panel-act={act}>
										{key === undefined ? null : <KeyCaps keys={key} size='small' />}
										<span>{t(ACTION_LABELS[act])}</span>
									</button>
								);
							})}
						</div>
						<p className={styles['hint']} data-panel-hint='' aria-live='polite' hidden />
						<template data-panel-strings=''>
							<span data-key='label'>{t(messages.panelPlayLabel)}</span>
							<span data-key='idle'>{t(messages.panelPlayIdle)}</span>
							{prefix ? (
								<span data-key='prefix'>{t(messages.panelPlayPrefix, richText)}</span>
							) : null}
							{Object.entries(REFUSALS).map(([reason, message]) => (
								<span key={reason} data-key={reason}>
									{t(message)}
								</span>
							))}
							{actions.map((act) => (
								<span key={act} data-key={`act:${act}`}>
									{notice(keys[act], t(ACTION_LABELS[act]))}
								</span>
							))}
							<span data-key='act:resize'>{notice(resize, t(messages.panelPlayResize))}</span>
						</template>
					</>
				}
				figure={
					<div className={styles['host']} data-panel-host='' aria-describedby={helpId}>
						<WindowFit>{window}</WindowFit>
						<span id={helpId} hidden>
							{t(messages.panelPlayHelp)}
						</span>
					</div>
				}
			/>
		</div>
	);
}
