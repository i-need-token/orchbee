import { useOrchbeeIntl, websiteProductMessages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { TuiPanel } from '../../../../../tui-panel/TuiPanel';
import { TuiSidebar } from '../../../../../tui-sidebar/TuiSidebar';
import { TuiTabStrip } from '../../../../../tui-tab-strip/TuiTabStrip';
import { TuiWindow } from '../../../../../tui-window/TuiWindow';
import { MULTIPLEXING_WORKSPACES, type MultiplexingFrame } from '../../multiplexing-scene';
import styles from './MultiplexingTerminalInterface.module.css';

/** The shared shell in the terminal interface, at the mini size: one Panel beside the sidebar. */
export function MultiplexingTerminalInterface({
	frame,
}: {
	frame: MultiplexingFrame;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<TuiWindow
			label={t(websiteProductMessages.terminalInterface)}
			size='mini'
			sidebar={<TuiSidebar project='shop' workspaces={MULTIPLEXING_WORKSPACES} />}
		>
			<TuiTabStrip title='PowerShell' state='working' />
			<div className={styles['panels']}>
				<TuiPanel title='PowerShell' focus rows={frame.rows.shell} bind={{ rows: 'shell' }} />
			</div>
		</TuiWindow>
	);
}
