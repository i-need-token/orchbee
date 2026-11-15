import { useOrchbeeIntl, websiteProductMessages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { initialPanelLayout } from '../panel-tree/panel-tree';
import { PanelTree, type PanelLeafProps } from '../panel-tree/PanelTree';
import { TuiPanel } from '../tui-panel/TuiPanel';
import { TuiTabStrip } from '../tui-tab-strip/TuiTabStrip';
import { TuiWindow } from '../tui-window/TuiWindow';
import styles from './PanelPlayTerminalInterface.module.css';

function TerminalPanel({ id, focus, rows }: PanelLeafProps): ReactElement {
	return (
		<TuiPanel title='PowerShell' focus={focus} rows={rows} bind={{ rows: 'panel' }} leaf={id} />
	);
}

/**
 * The terminal interface of a PanelPlay: one PowerShell Tab whose Panels come from a PanelTree,
 * drawn in the starting layout. Place it as the PanelPlay's `window`.
 */
export function PanelPlayTerminalInterface(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<TuiWindow label={t(websiteProductMessages.terminalInterface)} size='play'>
			<TuiTabStrip title='PowerShell' />
			<div className={styles['panels']}>
				<PanelTree layout={initialPanelLayout()} Leaf={TerminalPanel} />
			</div>
		</TuiWindow>
	);
}
