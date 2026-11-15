import type { ReactElement } from 'react';

import { Codicon } from '../codicon/Codicon';
import { StateGlyph, type AgentState } from '../state-glyph/StateGlyph';
import { WindowCaption } from '../window-caption/WindowCaption';
import styles from './DesktopTabStrip.module.css';

/**
 * The Tab strip of the desktop application with its one selected Tab, the new-Tab button, and the
 * window's caption glyphs at its end. The Tab shows the state of its most urgent Agent and an
 * unread mark. `bind` names the scene values script writes into it.
 */
export function DesktopTabStrip({
	title,
	state,
	unread,
	bind,
}: {
	title: string;
	state: AgentState;
	unread: boolean;
	bind?: { readonly state?: string; readonly unread?: string };
}): ReactElement {
	return (
		<div className={styles['strip']}>
			<div className={styles['tab']}>
				<span className={styles['icon']}>
					<StateGlyph state={state} bind={bind?.state} product='small' />
				</span>
				<span className={styles['title']}>{title}</span>
				<span className={styles['unread']} hidden={!unread} data-scene-show={bind?.unread}>
					●
				</span>
				<span className={styles['button']}>
					<Codicon name='close' />
				</span>
			</div>
			<span className={styles['newTab']}>
				<Codicon name='add' />
			</span>
			<WindowCaption placement='end' />
		</div>
	);
}
