import type { ReactNode } from 'react';

import styles from './WorkbenchPanelBroadcastTarget.module.css';

export function WorkbenchPanelBroadcastTarget(): ReactNode {
	return (
		<div
			aria-hidden='true'
			className={styles['root']}
			data-panel-broadcast-target='true'
			data-slot='workbench-panel-broadcast-target'
		/>
	);
}
