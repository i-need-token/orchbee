import { useOrchbeeIntl, websiteProductMessages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { DesktopPanel } from '../desktop-panel/DesktopPanel';
import { DesktopTabStrip } from '../desktop-tab-strip/DesktopTabStrip';
import { DesktopWindow } from '../desktop-window/DesktopWindow';
import { initialPanelLayout } from '../panel-tree/panel-tree';
import { PanelTree, type PanelLeafProps } from '../panel-tree/PanelTree';
import styles from './PanelPlayDesktopApplication.module.css';

function ApplicationPanel({ id, focus, alone, rows }: PanelLeafProps): ReactElement {
	return (
		<DesktopPanel
			title='PowerShell'
			focus={focus}
			rows={rows}
			bind={{ rows: 'panel' }}
			leaf={id}
			alone={alone}
		/>
	);
}

/**
 * The desktop application of a PanelPlay: one PowerShell Tab whose Panels come from a PanelTree,
 * drawn in the starting layout. Place it as the PanelPlay's `window`.
 */
export function PanelPlayDesktopApplication(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<DesktopWindow label={t(websiteProductMessages.desktopApplication)} size='play'>
			<DesktopTabStrip title='PowerShell' />
			<div className={styles['panels']}>
				<PanelTree layout={initialPanelLayout()} Leaf={ApplicationPanel} />
			</div>
		</DesktopWindow>
	);
}
