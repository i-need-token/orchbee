import { useOrchbeeIntl, websiteProductMessages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { DesktopPanel } from '../../../../../desktop-panel/DesktopPanel';
import { DesktopSidebar } from '../../../../../desktop-sidebar/DesktopSidebar';
import { DesktopTabStrip } from '../../../../../desktop-tab-strip/DesktopTabStrip';
import { DesktopWindow } from '../../../../../desktop-window/DesktopWindow';
import { MULTIPLEXING_WORKSPACES, type MultiplexingFrame } from '../../multiplexing-scene';
import styles from './MultiplexingDesktopApplication.module.css';

/**
 * The shared shell in the desktop application, at the mini size: the Tab's only Panel, which
 * shows no header, beside the sidebar.
 */
export function MultiplexingDesktopApplication({
	frame,
}: {
	frame: MultiplexingFrame;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<DesktopWindow
			label={t(websiteProductMessages.desktopApplication)}
			size='mini'
			sidebar={<DesktopSidebar project='shop' workspaces={MULTIPLEXING_WORKSPACES} />}
		>
			<DesktopTabStrip title='PowerShell' state='working' unread={false} />
			<div className={styles['column']} data-panel-column=''>
				<DesktopPanel title='PowerShell' focus rows={frame.rows.shell} bind={{ rows: 'shell' }} />
			</div>
		</DesktopWindow>
	);
}
